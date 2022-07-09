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

use crate::args::format_string;

const ROOT_PROJECT_CONF_NAME: &str = "project.yamis.toml";
const CONF_NAME: &str = "yamis.toml";
const PRIVATE_CONF_NAME: &str = "local.yamis.toml";
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

type Result<T> = result::Result<T, Box<dyn error::Error>>;

#[derive(Debug, PartialEq)]
pub enum ConfigError {
    Killed,               // Nothing to run
    EmptyTask(String),    // Nothing to run
    FileNotFound(String), // Given config file not found
    NoConfigFile,         // No config file was discovered
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ConfigError::Killed => write!(f, "Task was killed."),
            ConfigError::EmptyTask(ref s) => write!(f, "Task {} is empty.", s),
            ConfigError::FileNotFound(ref s) => write!(f, "File {} not found.", s),
            ConfigError::NoConfigFile => write!(f, "No config file found."),
        }
    }
}

impl error::Error for ConfigError {
    fn description(&self) -> &str {
        match *self {
            ConfigError::Killed => "task killed",
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
// Do not deny for now
// #[serde(deny_unknown_fields)]
// Minimal for now
/// Represents a Task. Should have only program, command or script at the same time.
pub struct Task {
    /// Name of the task.
    #[serde(skip)]
    name: String,
    /// Script to run.
    script: Option<String>,
    /// Env options for the task
    env: Option<HashMap<String, String>>,
    /// Working dir
    wd: Option<String>,
    linux: Option<Box<Task>>,
    windows: Option<Box<Task>>,
    macos: Option<Box<Task>>,
}

#[derive(Deserialize)]
// #[serde(deny_unknown_fields)]
/// Represents a config file.
pub struct ConfigFile {
    #[serde(skip)]
    filepath: String,
    /// Tasks inside the config file.
    pub tasks: Option<HashMap<String, Task>>,
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

fn get_temp_script(content: String) -> Result<PathBuf> {
    let mut dir = temp_dir();

    let file_name = format!("{}.yamis.{}", Uuid::new_v4(), SCRIPT_EXTENSION);
    dir.push(file_name);

    let mut file = File::create(&dir)?;
    file.write_all(content.as_bytes())?;
    Ok(dir)
}

impl Task {
    /// Runs the task with the given arguments.
    pub fn run(
        &self,
        args: &HashMap<String, String>,
        config_file: &ConfigFile,
    ) -> Result<ExitStatus> {
        return if let Some(script) = &self.script {
            let mut command = Command::new(SHELL_PROGRAM);
            command.arg(SHELL_PROGRAM_ARG);
            command.stdout(Stdio::inherit());
            command.stderr(Stdio::inherit());
            command.stdin(Stdio::inherit());

            match &self.wd {
                None => {}
                Some(wd) => {
                    let config_file_path = PathBuf::from(&config_file.filepath);
                    let base_path = config_file_path.parent().unwrap();
                    let wd = base_path.join(wd);
                    command.current_dir(wd);
                }
            };

            match &self.env {
                None => {}
                Some(env) => {
                    command.envs(env);
                }
            }

            let script = format_string(script, args)?;
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
        conf.filepath = path.to_str().unwrap().to_string();
        Ok(conf)
    }

    /// Finds a task by name on this config file or the next.
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
    pub fn for_path<S: AsRef<OsStr> + ?Sized>(path: &S) -> Result<ConfigFiles> {
        let config = ConfigFile::load(Path::new(path))?;
        return Ok(ConfigFiles {
            configs: vec![config],
        });
    }

    /// Returns a task for the given name.
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
