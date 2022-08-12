use crate::args_format::EscapeMode;
use crate::defaults::default_quote;
use crate::tasks::Task;
use crate::types::DynErrResult;
use crate::utils::get_task_dependency_graph;
use petgraph::algo::toposort;
use serde_derive::Deserialize;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::{env, error, fmt, fs};

/// Config file names by order of priority. The first one refers to local config and
/// should not be committed to the repository. The program should discover config files
/// by looping on the parent folders and current directory until reaching the root path
/// or a the project config (last one on the list) is found.
const CONFIG_FILES_PRIO: &[&str] = &["local.yamis.toml", "yamis.toml", "project.yamis.toml"];

/// Errors related to config files and tasks
#[derive(Debug, PartialEq)]
pub enum ConfigError {
    /// Raised when a config file is not found for a given path
    // FileNotFound(String), // Given config file not found
    /// Raised when no config file is found during auto-discovery
    NoConfigFile, // No config file was discovered
    /// Bad Config error
    BadConfigFile(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            // ConfigError::FileNotFound(ref s) => write!(f, "File {} not found.", s),
            ConfigError::NoConfigFile => write!(f, "No config file found."),
            ConfigError::BadConfigFile(ref s) => write!(f, "Bad config file. {}", s),
        }
    }
}

impl error::Error for ConfigError {
    fn description(&self) -> &str {
        match *self {
            // ConfigError::FileNotFound(_) => "file not found",
            ConfigError::NoConfigFile => "no config discovered",
            ConfigError::BadConfigFile(_) => "bad config file",
        }
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        None
    }
}

/// Used to discover files.
#[derive(Debug)]
pub struct ConfigFiles {
    /// First config file to check.
    pub configs: Vec<ConfigFile>,
}

/// Represents a config file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    /// Path of the file.
    #[serde(skip)]
    pub(crate) filepath: PathBuf,
    /// Whether to automatically quote argument with spaces unless task specified
    #[serde(default = "default_quote")]
    pub(crate) quote: EscapeMode,
    /// Tasks inside the config file.
    tasks: HashMap<String, Task>,
    /// Env variables for all the tasks.
    pub(crate) env: Option<HashMap<String, String>>,
    /// Env file to read environment variables from
    pub(crate) env_file: Option<String>,
}

#[derive(Debug)]
/// Iterates over existing config file paths, in order of priority.
pub struct ConfigFilePaths {
    index: usize,
    finished: bool,
    current: PathBuf,
}

impl Iterator for ConfigFilePaths {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        while !self.finished {
            let path = self.current.join(CONFIG_FILES_PRIO[self.index]);
            let is_last_index = CONFIG_FILES_PRIO.len() - 1 == self.index;
            let is_file = path.is_file();
            if is_last_index {
                if is_file {
                    // Break if the file found is the root file
                    // Index is updated on the previous match, therefore we compare against 0
                    self.finished = true;
                } else {
                    let new_current = path.parent().unwrap().parent();
                    match new_current {
                        None => {
                            self.finished = true;
                        }
                        Some(new_current) => {
                            self.current = new_current.to_path_buf();
                        }
                    }
                }
                self.index = 0;
            } else {
                self.index += 1;
            }
            if is_file {
                return Some(path);
            }
        }
        None
    }
}

impl ConfigFilePaths {
    /// Returns a new iterator that starts at the given path.
    fn new<S: AsRef<OsStr> + ?Sized>(path: &S) -> ConfigFilePaths {
        let current = PathBuf::from(&path);
        ConfigFilePaths {
            index: 0,
            finished: false,
            current,
        }
    }
}

impl ConfigFile {
    /// Loads a config file from the TOML representation.
    ///
    /// # Arguments
    ///
    /// * path - path of the toml file to load
    pub fn load(path: &Path) -> DynErrResult<ConfigFile> {
        let contents = match fs::read_to_string(&path) {
            Ok(file_contents) => file_contents,
            Err(e) => return Err(format!("There was an error reading the file:\n{}", e).into()),
        };
        let mut conf: ConfigFile = match toml::from_str(&*contents) {
            Ok(conf) => conf,
            Err(e) => {
                let err_msg = e.to_string();
                return Err(ConfigError::BadConfigFile(format!(
                    "There was an error parsing the toml file:\n{}{}",
                    &err_msg[..1].to_uppercase(),
                    &err_msg[1..]
                ))
                .into());
            }
        };
        conf.filepath = path.to_path_buf();
        conf.move_system_tasks_up_and_setup()?;

        let dep_graph = get_task_dependency_graph(&conf.tasks)?;
        let dependencies = toposort(&dep_graph, None);
        let dependencies = match dependencies {
            Ok(dependencies) => dependencies,
            Err(e) => {
                return Err(format!("Found a cyclic dependency for Task:\n{}", e.node_id()).into());
            }
        };
        let dependencies: Vec<String> = dependencies
            .iter()
            .rev()
            .map(|v| String::from(*v))
            .collect();
        for task_name in dependencies {
            // temp remove because of rules of references
            let mut task = conf.tasks.remove(&task_name).unwrap();
            // task.bases should be empty for the first item in the iteration
            // we no longer need the bases
            let bases = std::mem::take(&mut task.bases);
            for base in bases {
                let base_task = conf.tasks.get(&base).unwrap();
                task.extend_task(base_task);
            }
            // insert modified task back in
            conf.tasks.insert(task_name, task);
        }
        Ok(conf)
    }

    /// Moves OS specific tasks up and runs the task setup
    fn move_system_tasks_up_and_setup(&mut self) -> DynErrResult<()> {
        let mut os_tasks: HashMap<String, Task> = HashMap::new();
        let folder_reference = self.filepath.parent().unwrap();
        for (name, task) in self.tasks.iter_mut() {
            task.setup(name, folder_reference)?;

            if task.linux.is_some() {
                let os_task = std::mem::replace(&mut task.linux, None);
                let mut os_task = *os_task.unwrap();
                let os_task_name = format!("{}.linux", name);
                os_task.setup(&os_task_name, folder_reference)?;
                os_tasks.insert(os_task_name, os_task);
            }

            if task.windows.is_some() {
                let os_task = std::mem::replace(&mut task.windows, None);
                let mut os_task = *os_task.unwrap();
                let os_task_name = format!("{}.windows", name);
                os_task.setup(&os_task_name, folder_reference)?;
                os_tasks.insert(os_task_name, os_task);
            }

            if task.macos.is_some() {
                let os_task = std::mem::replace(&mut task.macos, None);
                let mut os_task = *os_task.unwrap();
                let os_task_name = format!("{}.macos", name);
                os_task.setup(&os_task_name, folder_reference)?;
                os_tasks.insert(os_task_name, os_task);
            }
        }
        for (name, task) in os_tasks {
            self.tasks.insert(name, task);
        }
        Ok(())
    }

    /// Finds a task by name on this config file if it exists.
    ///
    /// # Arguments
    ///
    /// * task_name - Name of the task to search for
    fn get_task(&self, task_name: &str) -> Option<&Task> {
        if let Some(task) = self.tasks.get(task_name) {
            return Some(task);
        }
        None
    }

    pub(crate) fn get_system_task(&self, task_name: &str) -> Option<&Task> {
        let os_task_name = format!("{}.{}", task_name, env::consts::OS);

        if let Some(task) = self.tasks.get(&os_task_name) {
            return Some(task);
        } else if let Some(task) = self.tasks.get(task_name) {
            return Some(task);
        }
        None
    }
}

impl ConfigFiles {
    /// Discovers the config files.
    pub fn discover<S: AsRef<OsStr> + ?Sized>(path: &S) -> DynErrResult<ConfigFiles> {
        let mut confs: Vec<ConfigFile> = Vec::new();
        for config_path in ConfigFilePaths::new(path) {
            let config = ConfigFile::load(config_path.as_path())?;
            confs.push(config);
        }
        if confs.is_empty() {
            return Err(ConfigError::NoConfigFile.into());
        }
        Ok(ConfigFiles { configs: confs })
    }

    /// Only loads the config file for the given path.
    ///
    /// # Arguments
    ///
    /// * path - Config file to load
    pub fn for_path<S: AsRef<OsStr> + ?Sized>(path: &S) -> DynErrResult<ConfigFiles> {
        let config = ConfigFile::load(Path::new(path))?;
        Ok(ConfigFiles {
            configs: vec![config],
        })
    }

    /// Returns a task for the given name and the config file that contains it.
    ///
    /// # Arguments
    ///
    /// * task_name - Name of the task to search for
    pub fn get_task(&self, task_name: &str) -> Option<(&Task, &ConfigFile)> {
        for conf in &self.configs {
            if let Some(task) = conf.get_task(task_name) {
                return Some((task, conf));
            }
        }
        None
    }

    /// Returns a task for the given name and the config file that contains it.
    ///
    /// # Arguments
    ///
    /// * task_name - Name of the task to search for
    pub fn get_system_task(&self, task_name: &str) -> Option<(&Task, &ConfigFile)> {
        for conf in &self.configs {
            if let Some(task) = conf.get_system_task(task_name) {
                return Some((task, conf));
            }
        }
        None
    }
}
