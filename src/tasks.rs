use std::collections::HashMap;
use std::env::temp_dir;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{stderr, stdin, stdout, Error, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::{env, error, fmt, fs, result};

use serde_derive::Deserialize;
use uuid::Uuid;

use crate::args::{format_string, ArgsMap};

/// Config file names by order of priority. The first one refers to local config and
/// should not be committed to the repository. The program should discover config files
/// by looping on the parent folders and current directory until reaching the root path
/// or a the project config (last one on the list) is found.
const CONFIG_FILES_PRIO: &[&str] = &["local.yamis.toml", "yamis.toml", "project.yamis.toml"];

cfg_if::cfg_if! {
    if #[cfg(target_os = "windows")] {
        const SHELL_PROGRAM: &str = "cmd";
        const SHELL_PROGRAM_ARG: &str = "/C";
        const SCRIPT_EXTENSION: &str = "bat";
    } else if #[cfg(target_os = "linux")] {
        const SHELL_PROGRAM: &str = "bash";
        const SHELL_PROGRAM_ARG: &str = "-c";
        const SCRIPT_EXTENSION: &str = "sh";
    } else if #[cfg(target_os = "macos")] {
        const SHELL_PROGRAM: &str = "bash";
        const SHELL_PROGRAM_ARG: &str = "-c";
        const SCRIPT_EXTENSION: &str = "sh";
    }else {
        compile_error!("Unsupported platform.");
    }
}

/// Alias the result type for convenience. We simply return a dynamic error as these should
/// be displayed to the user as they are.
type Result<T> = result::Result<T, Box<dyn error::Error>>;

/// Errors related to config files and tasks.
#[derive(Debug, PartialEq)]
pub enum ConfigError {
    /// Raised when trying to run an empty task.
    EmptyTask(String), // Nothing to run
    /// Raised when a config file is not found for a given path.
    FileNotFound(String), // Given config file not found
    /// Raised when no config file is found during auto-discovery.
    NoConfigFile, // No config file was discovered
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ConfigError::EmptyTask(ref s) => write!(f, "Task {} is empty.", s),
            ConfigError::FileNotFound(ref s) => write!(f, "File {} not found.", s),
            ConfigError::NoConfigFile => write!(f, "No config file found."),
        }
    }
}

impl error::Error for ConfigError {
    fn description(&self) -> &str {
        match *self {
            ConfigError::EmptyTask(_) => "nothing to run",
            ConfigError::FileNotFound(_) => "file not found",
            ConfigError::NoConfigFile => "no config discovered",
        }
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        None
    }
}

#[derive(Debug, Deserialize)]
// TODO: Deny invalid fields
// #[serde(deny_unknown_fields)]
/// Represents a Task.
pub struct Task {
    /// Whether to automatically quote argument with spaces
    quote: Option<bool>,
    /// Script to run.
    script: Option<String>,
    /// Env variables for the task.
    env: Option<HashMap<String, String>>,
    /// Working dir.
    wd: Option<String>,
    /// Task to run instead if the OS is linux.
    linux: Option<Box<Task>>,
    /// Task to run instead if the OS is windows.
    windows: Option<Box<Task>>,
    /// Task to run instead if the OS is macos.
    macos: Option<Box<Task>>,
}

fn default_quote() -> bool {
    true
}

#[derive(Deserialize)]
// TODO: Deny invalid fields
// #[serde(deny_unknown_fields)]
/// Represents a config file.
pub struct ConfigFile {
    /// Path of the file.
    #[serde(skip)]
    filepath: PathBuf,
    /// Whether to automatically quote argument with spaces unless task specified
    #[serde(default = "default_quote")]
    quote: bool,
    /// Tasks inside the config file.
    tasks: Option<HashMap<String, Task>>,
    /// Env variables for all the tasks.
    env: Option<HashMap<String, String>>,
}

/// Used to discover files.
pub struct ConfigFiles {
    /// First config file to check.
    configs: Vec<ConfigFile>,
}

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
        return None;
    }
}

impl ConfigFilePaths {
    /// Returns a new iterator that starts at the given path.
    fn new(path: PathBuf) -> ConfigFilePaths {
        ConfigFilePaths {
            index: 0,
            finished: false,
            current: path,
        }
    }
}

/// Creates a temporal script returns the path to it.
/// The OS should take care of cleaning the file.
///
/// # Arguments
///
/// * `content` - Content of the script file
fn get_temp_script(content: String) -> Result<PathBuf> {
    let mut dir = temp_dir();

    // Alternatives to uuid are timestamp and random number, or those together,
    // so this might change in the future.
    let file_name = format!("{}.yamis.{}", Uuid::new_v4(), SCRIPT_EXTENSION);
    dir.push(file_name);

    let mut file = File::create(&dir)?;
    file.write_all(content.as_bytes())?;
    Ok(dir)
}

impl Task {
    /// Runs the task. Stdout, stdin and stderr are inherited. Also, adds a handler to
    /// the ctrl-c signal that basically does nothing, such that the child process is the
    /// one handling the signal.
    ///
    /// # Arguments
    ///  
    /// * `args` - Arguments to return the script with
    /// * `config_file` - Config file the task belongs to
    pub fn run(&self, args: &ArgsMap, config_file: &ConfigFile) -> Result<ExitStatus> {
        return if let Some(script) = &self.script {
            let mut command = Command::new(SHELL_PROGRAM);
            command.arg(SHELL_PROGRAM_ARG);
            command.stdout(Stdio::inherit());
            command.stderr(Stdio::inherit());
            command.stdin(Stdio::inherit());

            match &self.wd {
                None => {}
                Some(wd) => {
                    let mut wd = PathBuf::from(wd);
                    if !wd.is_absolute() {
                        let config_file_path = &config_file.filepath;
                        let base_path = config_file_path.parent().unwrap();
                        wd = base_path.join(wd);
                    }
                    command.current_dir(wd);
                }
            };

            match &config_file.env {
                None => {}
                Some(env) => {
                    command.envs(env);
                }
            }

            match &self.env {
                None => {}
                Some(env) => {
                    command.envs(env);
                }
            }

            let quote = match self.quote {
                None => config_file.quote,
                Some(quote) => quote,
            };

            let script = format_string(script, args, quote)?;
            let script_file = get_temp_script(script)?;
            command.arg(script_file.to_str().unwrap());

            let mut child = command.spawn()?;

            // let child handle ctrl-c to prevent dropping the parent and leaving the child running
            ctrlc::set_handler(move || {})?;

            Ok(child.wait()?)
        } else {
            Err(ConfigError::EmptyTask(String::from("nothing found")))?
        };
    }
}

impl ConfigFile {
    /// Loads a config file from the TOML representation.
    ///
    /// # Arguments
    ///
    /// * path - path of the toml file to load
    pub fn load(path: &Path) -> Result<ConfigFile> {
        let contents = match fs::read_to_string(&path) {
            Ok(file_contents) => file_contents,
            Err(e) => Err(format!("There was an error reading the file:\n{}", e))?,
        };
        let mut conf: ConfigFile = match toml::from_str(&*contents) {
            Ok(conf) => conf,
            Err(e) => {
                let err_msg = e.to_string();
                Err(format!(
                    "There was an error parsing the toml file:\n{}{}",
                    &err_msg[..1].to_uppercase(),
                    &err_msg[1..]
                ))?
            }
        };
        conf.filepath = path.to_path_buf();
        Ok(conf)
    }

    /// Finds a task by name on this config file if it exists.
    ///
    /// # Arguments
    ///
    /// * task_name - Name of the task to search for
    fn get_task(&self, task_name: &str) -> Option<&Task> {
        if let Some(tasks) = &self.tasks {
            if let Some(task) = tasks.get(task_name) {
                return Some(task);
            }
        }
        return None;
    }
}

impl ConfigFiles {
    /// Discovers the config files.
    pub fn discover() -> Result<ConfigFiles> {
        let mut confs: Vec<ConfigFile> = Vec::new();
        let working_dir = env::current_dir()?;
        for config_path in ConfigFilePaths::new(working_dir) {
            let config = ConfigFile::load(config_path.as_path())?;
            confs.push(config);
        }
        if confs.is_empty() {
            Err(ConfigError::NoConfigFile)?
        }
        Ok(ConfigFiles { configs: confs })
    }

    /// Only loads the config file for the given path.
    ///
    /// # Arguments
    ///
    /// * path - Config file to load
    pub fn for_path<S: AsRef<OsStr> + ?Sized>(path: &S) -> Result<ConfigFiles> {
        let config = ConfigFile::load(Path::new(path))?;
        return Ok(ConfigFiles {
            configs: vec![config],
        });
    }

    /// Returns a task for the given name and the config file that contains it.
    ///
    /// # Arguments
    ///
    /// * task_name - Name of the task to search for
    pub fn get_task(&self, task_name: &str) -> Option<(&Task, &ConfigFile)> {
        for conf in &self.configs {
            if let Some(task) = conf.get_task(task_name) {
                if env::consts::OS == "linux" {
                    if let Some(linux_task) = &task.linux {
                        return Some((&*linux_task, conf));
                    }
                } else if env::consts::OS == "windows" {
                    if let Some(windows_task) = &task.windows {
                        return Some((&*windows_task, conf));
                    }
                } else if env::consts::OS == "macos" {
                    if let Some(macos_task) = &task.macos {
                        return Some((&*macos_task, conf));
                    }
                }
                return Some((task, conf));
            }
        }
        return None;
    }
}

#[test]
fn test_discovery() {
    let config = ConfigFiles::discover().unwrap();
    assert_eq!(config.configs.len(), 1);

    match config.get_task("non_existent") {
        None => {}
        Some((_, _)) => {
            assert!(false, "task non_existent should not exist");
        }
    }

    match config.get_task("hello_world") {
        None => {
            assert!(false, "task hello_world should exist");
        }
        Some((_, _)) => {}
    }

    let config = ConfigFiles::for_path("project.yamis.toml").unwrap();
    assert_eq!(config.configs.len(), 1);
}

#[test]
fn test_task_by_platform() {
    let config = ConfigFiles::discover().unwrap();
    assert_eq!(config.configs.len(), 1);

    match config.get_task("os_sample") {
        None => {}
        Some((task, config)) => {
            if cfg!(target_os = "windows") {
                assert_eq!(
                    task.script.clone().unwrap(),
                    String::from("echo hello windows")
                );
            } else if cfg!(target_os = "linux") {
                assert_eq!(
                    task.script.clone().unwrap(),
                    String::from("echo hello linux")
                );
            } else {
                assert_eq!(
                    task.script.clone().unwrap(),
                    String::from("echo hello linux")
                );
            }
        }
    }
}
