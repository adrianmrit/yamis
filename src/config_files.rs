use crate::tasks::Task;
use crate::types::DynErrResult;
use crate::utils::{
    get_path_relative_to_base, get_task_dependency_graph, read_env_file, to_os_task_name,
};
use directories::UserDirs;
use indexmap::IndexMap;
use petgraph::algo::toposort;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::{env, error, fmt, fs};

pub(crate) type ConfigFileSharedPtr = Arc<Mutex<ConfigFile>>;

/// Config file names by order of priority. The program should discover config files
/// by looping on the parent folders and current directory until reaching the root path
/// or a the project config (last one on the list) is found.
const CONFIG_FILES_PRIO: &[&str] = &[
    "yamis.private.yml",
    "yamis.private.yaml",
    "yamis.yml",
    "yamis.yaml",
    "yamis.root.yml",
    "yamis.root.yaml",
];

/// Global config file names by order of priority.
const GLOBAL_CONFIG_FILES_PRIO: &[&str] = &["yamis/yamis.global.yml", "yamis/yamis.global.yaml"];

pub(crate) type PathIteratorItem = PathBuf;
pub(crate) type PathIterator = Box<dyn Iterator<Item = PathIteratorItem>>;

/// Errors related to config files and tasks
#[derive(Debug)]
pub(crate) enum ConfigError {
    // /// Raised when a config file is not found for a given path
    // FileNotFound(String), // Given config file not found
    // /// Raised when no config file is found during auto-discovery
    // NoConfigFile, // No config file was discovered
    /// Bad Config error
    BadConfigFile(PathBuf, String),
}

impl Display for ConfigError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            // ConfigError::FileNotFound(ref s) => write!(f, "File {} not found.", s),
            // ConfigError::NoConfigFile => write!(f, "No config file found."),
            ConfigError::BadConfigFile(ref path, ref reason) => write!(
                f,
                "Bad config file `{}`:\n    {}",
                path.to_string_lossy(),
                reason
            ),
        }
    }
}

impl error::Error for ConfigError {}

/// Iterates over existing config file paths, in order of priority.
pub(crate) struct ConfigFilePaths {
    /// Index of value to use from `CONFIG_FILES_PRIO`
    index: usize,
    /// Whether the iterator finished or not
    ended: bool,
    /// Current directory
    current_dir: PathBuf,
}

impl Iterator for ConfigFilePaths {
    // Returning &Path would be more optimal but complicates more the code. There is no need
    // to optimize that much since it should not find that many config files.
    type Item = PathIteratorItem;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }

        while !self.ended {
            // Loops until a project config file is found or the root path is reached
            let config_file_name = CONFIG_FILES_PRIO[self.index];
            let config_file_path = self.current_dir.join(config_file_name);

            let config_file_path = if config_file_path.is_file() {
                if self.is_root_config_file(&config_file_path) {
                    self.ended = true;
                }
                Some(config_file_path)
            } else {
                None
            };

            self.index = (self.index + 1) % CONFIG_FILES_PRIO.len();

            // If we checked all the config files, we need to check in the parent directory
            if self.index == 0 {
                let new_current = self.current_dir.parent();
                match new_current {
                    None => {
                        self.ended = true;
                    }
                    Some(new_current) => {
                        self.current_dir = new_current.to_path_buf();
                    }
                }
            }
            if let Some(config_file_path) = config_file_path {
                return Some(config_file_path);
            }
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
    pub(crate) fn new<S: AsRef<OsStr> + ?Sized>(path: &S) -> Box<Self> {
        let current = PathBuf::from(path);
        Box::new(ConfigFilePaths {
            index: 0,
            ended: false,
            current_dir: current,
        })
    }

    fn is_root_config_file(&self, path: &Path) -> bool {
        path.file_name()
            .map(|s| s.to_string_lossy().starts_with("yamis.root."))
            .unwrap_or(false)
    }
}

/// Single config file path iterator. This iterator will only return the given path
/// if it exists and is a file, otherwise it will return None.

pub(crate) struct SingleConfigFilePath {
    path: PathBuf,
    ended: bool,
}

impl SingleConfigFilePath {
    /// Initializes SingleConfigFilePath to start at the given path.
    /// If the path does not exist or is not a file, the iterator will return None.
    /// # Arguments
    /// * `path`: Path to start searching for config files.
    /// returns: SingleConfigFilePath

    pub(crate) fn new<S: AsRef<OsStr> + ?Sized>(path: &S) -> Box<Self> {
        Box::new(SingleConfigFilePath {
            path: PathBuf::from(path),
            ended: false,
        })
    }
}

impl Iterator for SingleConfigFilePath {
    type Item = PathIteratorItem;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }
        self.ended = true;

        if self.path.is_file() {
            Some(self.path.clone())
        } else {
            None
        }
    }
}

/// Iterator that returns the first existing global config file path.
pub(crate) struct GlobalConfigFilePath {
    ended: bool,
}

impl GlobalConfigFilePath {
    /// Initializes GlobalConfigFilePath.

    pub(crate) fn new() -> Box<Self> {
        Box::new(GlobalConfigFilePath { ended: false })
    }
}

impl Iterator for GlobalConfigFilePath {
    type Item = PathIteratorItem;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }
        self.ended = true;
        if let Some(user_dirs) = UserDirs::new() {
            let home_dir = user_dirs.home_dir();
            for &path in GLOBAL_CONFIG_FILES_PRIO {
                let path = home_dir.join(path);
                if path.is_file() {
                    return Some(path);
                }
            }
        }
        None
    }
}

// At the moment we don't really take advantage of this, but might be useful in the future.
/// Caches config files to avoid reading them multiple times.
pub(crate) struct ConfigFilesContainer {
    /// Cached config files
    cached: IndexMap<PathBuf, ConfigFileSharedPtr>,
}

impl ConfigFilesContainer {
    /// Initializes ConfigFilesContainer.
    pub fn new() -> Self {
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

impl Default for ConfigFilesContainer {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a config file.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    /// Version of the config file.
    #[allow(dead_code)] // to avoid lint errors
    #[serde(default, skip_serializing)]
    version: serde::de::IgnoredAny,
    /// Path of the file.
    #[serde(skip_deserializing)]
    pub(crate) filepath: PathBuf,

    #[serde(default)]
    /// Working directory. Defaults to the folder where the script runs.
    wd: Option<String>,
    /// Tasks inside the config file.
    #[serde(default)]
    pub(crate) tasks: HashMap<String, Task>,
    /// Env variables for all the tasks.
    pub(crate) env: Option<HashMap<String, String>>,
    /// Env file to read environment variables from
    pub(crate) env_file: Option<String>,
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
        let contents = match fs::read_to_string(path) {
            Ok(file_contents) => file_contents,
            Err(e) => return Err(format!("There was an error reading the file:\n{}", e).into()),
        };
        if is_yaml {
            Ok(serde_yaml::from_str(&contents)?)
        } else {
            Ok(toml::from_str(&contents)?)
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
                if let Some(base_task) = conf.tasks.get(&os_task_name) {
                    task.extend_task(base_task);
                } else if let Some(base_task) = conf.tasks.get(&base) {
                    task.extend_task(base_task);
                } else {
                    panic!("found non existent task {}", base);
                }
            }
            // insert modified task back in
            conf.tasks.insert(dependency_name, task);
        }

        // Store the other tasks left
        for (task_name, task) in tasks {
            conf.tasks.insert(task_name, task);
        }
        Ok(conf)
    }

    /// Returns the directory where the config file
    pub fn directory(&self) -> &Path {
        self.filepath.parent().unwrap()
    }

    /// If set in the config file, returns the working directory as an absolute path.
    pub fn working_directory(&self) -> Option<PathBuf> {
        // Some sort of cache would make it faster, but keeping it
        // simple until it is really needed
        self.wd
            .as_ref()
            .map(|wd| get_path_relative_to_base(self.directory(), wd))
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
    pub fn get_task(&self, task_name: &str) -> Option<Task> {
        self.get_task_ref(task_name).cloned()
    }

    pub fn get_task_ref(&self, task_name: &str) -> Option<&Task> {
        let os_task_name = to_os_task_name(task_name);

        if let Some(task) = self.tasks.get(&os_task_name) {
            return Some(task);
        } else if let Some(task) = self.tasks.get(task_name) {
            return Some(task);
        }
        None
    }

    /// Finds an public task by name on this config file and returns it if it exists.
    /// It searches fist for the current OS version of the task, if None is found,
    /// it tries with the plain name.
    ///
    /// # Arguments
    ///
    /// * task_name - Name of the task to search for
    pub fn get_public_task(&self, task_name: &str) -> Option<Task> {
        let os_task_name = to_os_task_name(task_name);

        if let Some(task) = self.tasks.get(&os_task_name) {
            if task.is_private() {
                return None;
            }
            return Some(task.clone());
        } else if let Some(task) = self.tasks.get(task_name) {
            if task.is_private() {
                return None;
            }
            return Some(task.clone());
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

        self.tasks.contains_key(&os_task_name) || self.tasks.contains_key(task_name)
    }

    /// Returns the list of names of tasks in this config file
    pub fn get_task_names(&self) -> Vec<&String> {
        self.tasks.keys().collect()
    }

    /// Returns the list of names of tasks that are not private in this config file
    pub fn get_public_task_names(&self) -> Vec<&str> {
        self.tasks
            .values()
            .filter(|t| !t.is_private())
            .map(|t| t.get_name())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_discovery() {
        let tmp_dir = TempDir::new().unwrap();
        let project_config_path = tmp_dir.path().join("yamis.root.yml");
        let mut project_config_file = File::create(project_config_path.as_path()).unwrap();
        project_config_file
            .write_all(
                r#"
    tasks:
        hello_project:
            script: "echo hello project"
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

        let local_config_path = tmp_dir.path().join("yamis.private.yaml");
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

        let mut config_files = ConfigFilesContainer::new();
        let mut paths: Box<ConfigFilePaths> = ConfigFilePaths::new(&tmp_dir.path());
        let local_path = paths.next().unwrap();
        let regular_path = paths.next().unwrap();
        let project_path = paths.next().unwrap();

        assert!(paths.next().is_none());

        config_files.read_config_file(local_path).unwrap();
        config_files.read_config_file(regular_path).unwrap();
        config_files.read_config_file(project_path).unwrap();

        assert!(!config_files.has_task("non_existent"));
        assert!(config_files.has_task("hello_project"));
        assert!(config_files.has_task("hello"));
        assert!(config_files.has_task("hello_local"));
    }

    #[test]
    fn test_discovery_given_file() {
        let tmp_dir = TempDir::new().unwrap();
        let sample_config_file_path = tmp_dir.path().join("sample.yamis.yml");
        let mut sample_config_file = File::create(sample_config_file_path.as_path()).unwrap();
        sample_config_file
            .write_all(
                r#"
tasks:
    hello_project:
        script: echo hello project
    "#
                .as_bytes(),
            )
            .unwrap();

        let mut config_files = ConfigFilesContainer::new();
        let mut paths = SingleConfigFilePath::new(&sample_config_file_path);
        let sample_path = paths.next().unwrap();
        assert!(paths.next().is_none());

        config_files.read_config_file(sample_path).unwrap();

        assert!(config_files.has_task("hello_project"));
    }

    #[test]
    fn test_config_file_invalid_path() {
        let cnfg = ConfigFile::extract(Path::new("non_existent"));
        assert!(cnfg.is_err());

        let cnfg = ConfigFile::extract(Path::new("non_existent.ext"));
        assert!(cnfg.is_err());

        let cnfg = ConfigFile::extract(Path::new("non_existent.yml"));
        assert!(cnfg.is_err());
    }

    #[test]
    fn test_container_read_config_error() {
        let tmp_dir = TempDir::new().unwrap();
        let project_config_path = tmp_dir.path().join("yamis.root.yml");
        let mut project_config_file = File::create(project_config_path.as_path()).unwrap();
        project_config_file
            .write_all(
                r#"
    some invalid condig
    "#
                .as_bytes(),
            )
            .unwrap();

        let mut config_files = ConfigFilesContainer::default();
        let result = config_files.read_config_file(project_config_path);

        assert!(result.is_err());
    }

    #[test]
    fn test_config_file_read() {
        let tmp_dir = TempDir::new().unwrap();

        let dot_env_path = tmp_dir.path().join(".env");
        let mut dot_env_file = File::create(dot_env_path.as_path()).unwrap();
        dot_env_file
            .write_all(
                r#"VALUE_OVERRIDE=OLD_VALUE
OTHER_VALUE=HELLO
"#
                .as_bytes(),
            )
            .unwrap();

        let project_config_path = tmp_dir.path().join("yamis.root.yaml");
        let mut project_config_file = File::create(project_config_path.as_path()).unwrap();
        project_config_file
            .write_all(
                r#"
env_file: ".env"
env:
  VALUE_OVERRIDE: NEW_VALUE
tasks:
  hello_local:
    script: echo hello local
        "#
                .as_bytes(),
            )
            .unwrap();
        let config_file = ConfigFile::load(project_config_path).unwrap();
        assert!(config_file.has_task("hello_local"));
        let env = config_file.env.unwrap();
        assert_eq!(env.get("VALUE_OVERRIDE").unwrap(), "NEW_VALUE");
        assert_eq!(env.get("OTHER_VALUE").unwrap(), "HELLO");
    }

    #[test]
    fn test_config_file_get_task_names() {
        let tmp_dir = TempDir::new().unwrap();

        let project_config_path = tmp_dir.path().join("yamis.root.yaml");
        let mut project_config_file = File::create(project_config_path.as_path()).unwrap();
        project_config_file
            .write_all(
                r#"
tasks:
  task_1:
    script: echo hello

  task_2:
    script: echo hello again

  task_3:
    script: echo hello again
    private: true

        "#
                .as_bytes(),
            )
            .unwrap();
        let config_file = ConfigFile::load(project_config_path).unwrap();
        let mut task_names = config_file.get_task_names();
        task_names.sort();
        assert_eq!(task_names, vec!["task_1", "task_2", "task_3"]);
        let mut task_names = config_file.get_public_task_names();
        task_names.sort();
        assert_eq!(task_names, vec!["task_1", "task_2"]);
    }

    #[test]
    fn test_config_file_get_task() {
        let tmp_dir = TempDir::new().unwrap();

        let project_config_path = tmp_dir.path().join("yamis.root.yaml");
        let mut project_config_file = File::create(project_config_path.as_path()).unwrap();
        project_config_file
            .write_all(
                r#"
tasks:
  task_1:
    script: echo hello

  task_2:
    script: echo hello again

  task_3:
    script: echo hello again
    private: true

        "#
                .as_bytes(),
            )
            .unwrap();
        let config_file = ConfigFile::load(project_config_path).unwrap();

        let task_nam = config_file.get_task("task_1");
        assert!(task_nam.is_some());
        assert_eq!(task_nam.unwrap().get_name(), "task_1");

        let task_nam = config_file.get_task("task_2");
        assert!(task_nam.is_some());
        assert_eq!(task_nam.unwrap().get_name(), "task_2");

        let task_nam = config_file.get_task("task_3");
        assert!(task_nam.is_some());
        assert_eq!(task_nam.unwrap().get_name(), "task_3");
    }

    #[test]
    fn test_config_file_get_non_private_task() {
        let tmp_dir = TempDir::new().unwrap();

        let project_config_path = tmp_dir.path().join("yamis.root.yaml");
        let mut project_config_file = File::create(project_config_path.as_path()).unwrap();
        project_config_file
            .write_all(
                r#"
tasks:
  task_1:
    script: echo hello

  task_2:
    script: echo hello again

  task_3:
    script: echo hello again
    private: true

        "#
                .as_bytes(),
            )
            .unwrap();
        let config_file = ConfigFile::load(project_config_path).unwrap();

        let task_nam = config_file.get_public_task("task_1");
        assert!(task_nam.is_some());
        assert_eq!(task_nam.unwrap().get_name(), "task_1");

        let task_nam = config_file.get_public_task("task_2");
        assert!(task_nam.is_some());
        assert_eq!(task_nam.unwrap().get_name(), "task_2");

        let task_nam = config_file.get_public_task("task_3");
        assert!(task_nam.is_none());
    }

    #[test]
    fn test_wrong_config_file_extension() {
        let tmp_dir = TempDir::new().unwrap();

        let project_config_path = tmp_dir.path().join("yamis.root.wrong");
        File::create(project_config_path.as_path()).unwrap();
        let config_file = ConfigFile::load(project_config_path);
        assert!(config_file.is_err());
        assert!(config_file
            .unwrap_err()
            .to_string()
            .contains("Bad config file"));
    }
}
