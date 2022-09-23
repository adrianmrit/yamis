use crate::defaults::default_quote;
use crate::parser::EscapeMode;
use crate::tasks::Task;
use crate::types::DynErrResult;
use crate::utils::{
    get_path_relative_to_base, get_task_dependency_graph, read_env_file, to_os_task_name,
};
use indexmap::IndexMap;
use petgraph::algo::toposort;
use serde_derive::Deserialize;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::{env, error, fmt, fs};

pub type ConfigFileSharedPtr = Arc<Mutex<ConfigFile>>;

/// Config file names by order of priority. The first one refers to local config and
/// should not be committed to the repository. The program should discover config files
/// by looping on the parent folders and current directory until reaching the root path
/// or a the project config (last one on the list) is found.
const CONFIG_FILES_PRIO: &[&str] = &["local.yamis", "yamis", "project.yamis"];

const GLOBAL_CONFIG_FILE: &str = "user.yamis";
/// Name the global config file, without extension.

#[cfg(not(test))]
const GLOBAL_CONFIG_FILE_PATH: &str = "~/.yamis";

/// Allowed extensions for config files.
const ALLOWED_EXTENSIONS: &[&str] = &["yml", "yaml", "toml"];

/// Errors related to config files and tasks
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ConfigError {
    // /// Raised when a config file is not found for a given path
    // FileNotFound(String), // Given config file not found
    // /// Raised when no config file is found during auto-discovery
    // NoConfigFile, // No config file was discovered
    /// Bad Config error
    BadConfigFile(PathBuf, String),
    /// Found a config file multiple times with different extensions
    DuplicateConfigFile(String),
}

impl Display for ConfigError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            // ConfigError::FileNotFound(ref s) => write!(f, "File {} not found.", s),
            // ConfigError::NoConfigFile => write!(f, "No config file found."),
            ConfigError::BadConfigFile(ref path, ref reason) => write!(f, "Bad config file `{}`:\n    {}", path.to_string_lossy(), reason),
            ConfigError::DuplicateConfigFile(ref s) => write!(f,
                                                              "Config file `{}` defined multiple times with different extensions in the same directory.", s),
        }
    }
}

impl error::Error for ConfigError {
    fn description(&self) -> &str {
        match *self {
            // ConfigError::FileNotFound(_) => "file not found",
            // ConfigError::NoConfigFile => "no config discovered",
            ConfigError::BadConfigFile(_, _) => "bad config file",
            ConfigError::DuplicateConfigFile(_) => "duplicate config file",
        }
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        None
    }
}

/// Represents a config file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    /// Path of the file.
    #[serde(skip)]
    pub(crate) filepath: PathBuf,
    #[serde(default)]
    /// Working directory. Defaults to the folder containing the config file.
    wd: String,
    /// Whether to automatically quote argument with spaces unless task specified
    #[serde(default = "default_quote")]
    pub(crate) quote: EscapeMode,
    /// Tasks inside the config file.
    #[serde(default)]
    pub(crate) tasks: HashMap<String, Task>,
    /// Env variables for all the tasks.
    pub(crate) env: Option<HashMap<String, String>>,
    /// Env file to read environment variables from
    pub(crate) env_file: Option<String>,
    #[serde(skip)]
    pub(crate) loaded_tasks: HashMap<String, Arc<Task>>,
}

#[derive(Debug)]
/// Iterates over existing config file paths, in order of priority.
pub struct ConfigFilePaths {
    /// Index of value to use from `CONFIG_FILES_PRIO`
    index: usize,
    /// Whether the iterator finished or not
    root_reached: bool,
    /// Whether the iterator finished or not
    ended: bool,
    /// Only loaded one file, which should already be in the cache
    single: bool,
    /// Current directory
    current_dir: PathBuf,
    /// Cached config files
    cached: Vec<PathBuf>,
}

pub struct ConfigFilesContainer {
    /// Cached config files
    cached: IndexMap<PathBuf, ConfigFileSharedPtr>,
}

impl Iterator for ConfigFilePaths {
    // Returning &Path would be more optimal but complicates more the code. There is no need
    // to optimize that much since it should not find that many config files.
    type Item = DynErrResult<PathBuf>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }

        if self.single {
            self.ended = true;
            return if self.cached.is_empty() {
                None
            } else {
                Some(Ok(PathBuf::from(self.cached.last().unwrap())))
            };
        }

        // Stores any error to return after breaking the loop
        let mut err: Option<Box<dyn error::Error>> = None;

        // Loops until a project config file is found or the root path is reached
        while !self.root_reached {
            let config_file_name = CONFIG_FILES_PRIO[self.index];

            // project file is the last one on the list
            let checking_for_project_config = CONFIG_FILES_PRIO.len() - 1 == self.index;
            self.index = (self.index + 1) % CONFIG_FILES_PRIO.len();

            let found_file =
                self.get_config_file_path(self.current_dir.as_path(), config_file_name);
            let found_file = match found_file {
                Ok(v) => v,
                Err(e) => {
                    err = Some(e.into());
                    break;
                }
            };

            if checking_for_project_config {
                // When checking for project config, we need to update the next dir to check
                let new_current = self.current_dir.parent();
                match new_current {
                    None => {
                        self.root_reached = true;
                    }
                    Some(new_current) => {
                        self.current_dir = new_current.to_path_buf();
                    }
                }
            }

            if let Some(found_file) = found_file {
                if checking_for_project_config {
                    self.root_reached = true;
                }
                self.cached.push(found_file.clone());
                return Some(Ok(found_file));
            }
        }

        self.root_reached = true;
        self.ended = true;

        if let Some(err) = err {
            return Some(Err(err));
        }

        let global_config_dir = Self::get_global_config_file_dir();
        let found_file = self.get_config_file_path(&global_config_dir, GLOBAL_CONFIG_FILE);
        let found_file = match found_file {
            Ok(v) => v,
            Err(e) => {
                return Some(Err(e.into()));
            }
        };

        if let Some(found_file) = found_file {
            self.cached.push(found_file.clone());
            return Some(Ok(found_file));
        }

        None
    }
}

impl ConfigFilePaths {
    /// Initializes ConfigFilePaths to start at the given path.
    ///
    /// # Arguments
    ///
    /// * `path`: Path to start searching for config files.
    ///
    /// returns: ConfigFilePaths
    pub fn new<S: AsRef<OsStr> + ?Sized>(path: &S) -> ConfigFilePaths {
        let current = PathBuf::from(path);
        ConfigFilePaths {
            index: 0,
            ended: false,
            root_reached: false,
            single: false,
            current_dir: current,
            cached: Vec::with_capacity(2),
        }
    }

    /// Initializes ConfigFilePaths such that it only loads the config file for the given path.
    ///
    /// # Arguments
    ///
    /// * `path`: Path of the config file to load
    ///
    /// returns:  Result<ConfigFilePaths, Box<dyn error::Error>>
    pub fn only<S: AsRef<OsStr> + ?Sized>(path: &S) -> DynErrResult<ConfigFilePaths> {
        let path = PathBuf::from(path);
        let config_files = ConfigFilePaths {
            index: 0,
            ended: true,
            root_reached: true,
            single: true,
            current_dir: path.clone(),
            cached: vec![path],
        };
        Ok(config_files)
    }

    /// Returns the path of the global config file directory.
    #[cfg(not(test))]
    pub(crate) fn get_global_config_file_dir() -> PathBuf {
        let global_config_dir = shellexpand::tilde(GLOBAL_CONFIG_FILE_PATH);
        PathBuf::from(global_config_dir.as_ref())
    }

    /// Returns the path of the global config file directory.
    #[cfg(test)]
    pub(crate) fn get_global_config_file_dir() -> PathBuf {
        use assert_fs::TempDir;
        use lazy_static::lazy_static;
        lazy_static! {
            static ref GLOBAL_CONFIG_DIR: TempDir = TempDir::new().unwrap();
            pub static ref TEST_GLOBAL_CONFIG_PATH: PathBuf =
                PathBuf::from(GLOBAL_CONFIG_DIR.path());
        }
        TEST_GLOBAL_CONFIG_PATH.clone()
    }

    /// Finds the appropriate filepath to load in the given dir.
    ///
    /// # Arguments
    ///
    /// * `dir`:
    /// * `config_file_name`:
    ///
    /// returns: Result<Option<PathBuf>, ConfigError>
    fn get_config_file_path(
        &self,
        dir: &Path,
        config_file_name: &str,
    ) -> Result<Option<PathBuf>, ConfigError> {
        let mut files_count: u8 = 0;
        let mut found_file: Option<PathBuf> = None;

        for file_extension in ALLOWED_EXTENSIONS {
            let file_name = format!("{}.{}", config_file_name, file_extension);
            let path = dir.join(file_name);
            if path.is_file() {
                files_count += 1;
                found_file = Some(path);
            }
        }

        if files_count > 1 {
            Err(ConfigError::DuplicateConfigFile(String::from(
                config_file_name,
            )))
        } else {
            Ok(found_file)
        }
    }

    /// Peeks at the file and returns the version of the config file.
    ///
    /// # Arguments
    ///
    /// * `path`: path to the file to extract the version from
    ///
    /// returns: Result<String, Box<dyn Error, Global>>
    pub(crate) fn get_version(path: &Path) -> DynErrResult<String> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        match lines.next() {
            Some(line) => {
                let line = line?;
                match line.strip_prefix("#!v:") {
                    None => Ok(String::from("1")),
                    Some(version) => Ok(String::from(version.trim())),
                }
            }
            None => Ok(String::from("1")),
        }
    }
}

impl ConfigFilesContainer {
    /// Initializes ConfigFilesContainer.
    pub fn new() -> ConfigFilesContainer {
        ConfigFilesContainer {
            cached: IndexMap::new(),
        }
    }

    /// Reads the config file from the given path.
    ///
    /// # Arguments
    ///
    /// * `path`: Path to read the config file from
    ///
    /// returns: Result<Arc<Mutex<ConfigFile>>, Box<dyn Error, Global>>
    pub fn read_config_file(&mut self, path: PathBuf) -> DynErrResult<ConfigFileSharedPtr> {
        let config_file = ConfigFile::load(path.clone());
        match config_file {
            Ok(config_file) => {
                let arc_config_file = Arc::new(Mutex::new(config_file));
                let result = Ok(Arc::clone(&arc_config_file));
                self.cached.insert(path, arc_config_file);
                result
            }
            Err(e) => Err(e),
        }
    }

    #[cfg(test)] // Used in tests only for now, but still leaving it here just in case
    /// Returns whether the given task exists in the config files.
    pub fn has_task<S: AsRef<str>>(&mut self, name: S) -> bool {
        for config_file in self.cached.values() {
            let config_file_ptr = config_file.as_ref();
            let handle = config_file_ptr.lock().unwrap();
            if handle.has_task(name.as_ref()) {
                return true;
            }
        }
        false
    }
}

impl ConfigFile {
    /// Reads the file from the path and constructs a config file
    fn extract(path: &Path) -> DynErrResult<ConfigFile> {
        let extension = path
            .extension()
            .unwrap_or_else(|| OsStr::new(""))
            .to_string_lossy()
            .to_string();

        let is_yaml = match extension.as_str() {
            "yaml" => true,
            "yml" => true,
            "toml" => false,
            _ => {
                return Err(ConfigError::BadConfigFile(
                    path.to_path_buf(),
                    String::from("Extension must be either `.toml`, `.yaml` or `.yml`"),
                )
                .into());
            }
        };
        let contents = match fs::read_to_string(&path) {
            Ok(file_contents) => file_contents,
            Err(e) => return Err(format!("There was an error reading the file:\n{}", e).into()),
        };
        if is_yaml {
            Ok(serde_yaml::from_str(&*contents)?)
        } else {
            Ok(toml::from_str(&*contents)?)
        }
    }

    /// Loads a config file
    ///
    /// # Arguments
    ///
    /// * path - path of the toml file to load
    pub fn load(path: PathBuf) -> DynErrResult<ConfigFile> {
        let mut conf: ConfigFile = ConfigFile::extract(path.as_path())?;
        conf.filepath = path;

        if let Some(env_file_path) = &conf.env_file {
            let env_file_path = get_path_relative_to_base(conf.directory(), &env_file_path);
            let env_from_file = read_env_file(&env_file_path)?;
            match conf.env.as_mut() {
                None => {
                    conf.env = Some(HashMap::from_iter(env_from_file.into_iter()));
                }
                Some(env) => {
                    for (key, val) in env_from_file.into_iter() {
                        // manually set env takes precedence over env_file
                        env.entry(key).or_insert(val);
                    }
                }
            }
        }

        let mut tasks = conf.get_flat_tasks()?;

        let dep_graph = get_task_dependency_graph(&tasks)?;
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

        for dependency_name in dependencies {
            // temp remove because of rules of references
            let mut task = tasks.remove(&dependency_name).unwrap();
            // task.bases should be empty for the first item in the iteration
            // we no longer need the bases
            let bases = std::mem::take(&mut task.bases);
            for base in bases {
                let os_task_name = format!("{}.{}", &base, env::consts::OS);
                if let Some(base_task) = conf.loaded_tasks.get(&os_task_name) {
                    task.extend_task(base_task);
                } else if let Some(base_task) = conf.loaded_tasks.get(&base) {
                    task.extend_task(base_task);
                } else {
                    panic!("found non existent task {}", base);
                }
            }
            // insert modified task back in
            conf.loaded_tasks.insert(dependency_name, Arc::new(task));
        }

        // Store the other tasks left
        for (task_name, task) in tasks {
            conf.loaded_tasks.insert(task_name, Arc::new(task));
        }
        Ok(conf)
    }

    /// Returns the directory where the config file
    pub fn directory(&self) -> &Path {
        self.filepath.parent().unwrap()
    }

    /// Returns the working directory for the config file
    pub fn working_directory(&self) -> PathBuf {
        // Some sort of cache would make it faster, but keeping it
        // simple until it is really needed
        get_path_relative_to_base(self.directory(), &self.wd)
    }

    /// Returns plain and OS specific tasks with normalized names. This consumes `self.tasks`
    fn get_flat_tasks(&mut self) -> DynErrResult<HashMap<String, Task>> {
        let mut flat_tasks = HashMap::new();
        let tasks = std::mem::take(&mut self.tasks);
        for (name, mut task) in tasks {
            // TODO: Use a macro
            if task.linux.is_some() {
                let os_task = std::mem::replace(&mut task.linux, None);
                let mut os_task = *os_task.unwrap();
                let os_task_name = format!("{}.linux", name);
                if flat_tasks.contains_key(&os_task_name) {
                    return Err(format!("Duplicate task `{}`", os_task_name).into());
                }
                os_task.setup(&os_task_name, self.directory())?;
                flat_tasks.insert(os_task_name, os_task);
            }

            if task.windows.is_some() {
                let os_task = std::mem::replace(&mut task.windows, None);
                let mut os_task = *os_task.unwrap();
                let os_task_name = format!("{}.windows", name);
                if flat_tasks.contains_key(&os_task_name) {
                    return Err(format!("Duplicate task `{}`", os_task_name).into());
                }
                os_task.setup(&os_task_name, self.directory())?;
                flat_tasks.insert(os_task_name, os_task);
            }

            if task.macos.is_some() {
                let os_task = std::mem::replace(&mut task.macos, None);
                let mut os_task = *os_task.unwrap();
                let os_task_name = format!("{}.macos", name);
                if flat_tasks.contains_key(&os_task_name) {
                    return Err(format!("Duplicate task `{}`", os_task_name).into());
                }
                os_task.setup(&os_task_name, self.directory())?;
                flat_tasks.insert(os_task_name, os_task);
            }
            task.setup(&name, self.directory())?;
            flat_tasks.insert(name, task);
        }
        Ok(flat_tasks)
    }

    /// Finds and task by name on this config file and returns it if it exists.
    /// It searches fist for the current OS version of the task, if None is found,
    /// it tries with the plain name.
    ///
    /// # Arguments
    ///
    /// * task_name - Name of the task to search for
    pub fn get_task(&self, task_name: &str) -> Option<Arc<Task>> {
        let os_task_name = to_os_task_name(task_name);

        if let Some(task) = self.loaded_tasks.get(&os_task_name) {
            return Some(Arc::clone(task));
        } else if let Some(task) = self.loaded_tasks.get(task_name) {
            return Some(Arc::clone(task));
        }
        None
    }

    /// Returns whether the config file has a task with the given name. This also
    /// checks for the OS specific version of the task.
    ///
    /// # Arguments
    ///
    /// * `task_name`: Name of the task to check for
    ///
    /// returns: bool
    #[cfg(test)]
    pub fn has_task(&self, task_name: &str) -> bool {
        let os_task_name = to_os_task_name(task_name);

        self.loaded_tasks.contains_key(&os_task_name) || self.loaded_tasks.contains_key(task_name)
    }
}

impl Display for ConfigFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.filepath.display())
    }
}

#[cfg(test)]
mod tests {
    use crate::config_files::{ConfigFilePaths, ConfigFilesContainer};
    use assert_fs::TempDir;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_discovery() {
        let tmp_dir = TempDir::new().unwrap();
        let project_config_path = tmp_dir.path().join("project.yamis.toml");
        let mut project_config_file = File::create(project_config_path.as_path()).unwrap();
        project_config_file
            .write_all(
                r#"
    [tasks.hello_project]
    script = "echo hello project"
    "#
                .as_bytes(),
            )
            .unwrap();

        let config_path = tmp_dir.path().join("yamis.yaml");
        let mut config_file = File::create(config_path.as_path()).unwrap();
        config_file
            .write_all(
                r#"
    tasks:
        hello:
            script: echo hello
    "#
                .as_bytes(),
            )
            .unwrap();

        let local_config_path = tmp_dir.path().join("local.yamis.yaml");
        let mut local_file = File::create(local_config_path.as_path()).unwrap();
        local_file
            .write_all(
                r#"
    tasks:
        hello_local:
            script: echo hello local
    "#
                .as_bytes(),
            )
            .unwrap();

        let global_config_path =
            ConfigFilePaths::get_global_config_file_dir().join("user.yamis.toml");
        let mut global_config_file = File::create(global_config_path.as_path()).unwrap();
        global_config_file
            .write_all(
                r#"
                [tasks.hello_global]
                script = "echo hello project"
                "#
                .as_bytes(),
            )
            .unwrap();

        let mut config_files = ConfigFilesContainer::new();
        let mut paths = ConfigFilePaths::new(&tmp_dir.path());
        let local_path = paths.next().unwrap().unwrap();
        let regular_path = paths.next().unwrap().unwrap();
        let project_path = paths.next().unwrap().unwrap();
        let global_path = paths.next().unwrap().unwrap();
        assert!(paths.next().is_none());
        config_files.read_config_file(local_path).unwrap();
        config_files.read_config_file(regular_path).unwrap();
        config_files.read_config_file(project_path).unwrap();
        config_files.read_config_file(global_path).unwrap();

        assert!(!config_files.has_task("non_existent"));
        assert!(config_files.has_task("hello_project"));
        assert!(config_files.has_task("hello"));
        assert!(config_files.has_task("hello_local"));
        assert!(config_files.has_task("hello_global"));

        let mut paths = ConfigFilePaths::only(project_config_path.as_path()).unwrap();

        assert!(paths.next().is_none());

        assert_eq!(paths.cached[0], project_config_path.as_path());
    }
}
