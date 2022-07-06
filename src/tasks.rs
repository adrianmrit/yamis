use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::read;
use std::path::{Ancestors, Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::{env, error, fmt, fs, result};

use serde_derive::Deserialize;

use crate::args::format_string;

const ROOT_PROJECT_CONF_NAME: &str = "project.yamis.toml";
const CONF_NAME: &str = "yamis.toml";
const PRIVATE_CONF_NAME: &str = "local.yamis.toml";
const CONFIG_FILES_PRIO: &[&str] = &["local.yamis.toml", "yamis.toml", "project.yamis.toml"];

cfg_if::cfg_if! {
    if #[cfg(target_os = "windows")] {
        const SHELL_PROGRAM: &str = "cmd";
        const SHELL_PROGRAM_ARG: &str = "/C";
    } else if #[cfg(target_os = "linux")] {
        const SHELL_PROGRAM: &str = "bash";
        const SHELL_PROGRAM_ARG: &str = "-c";
    } else if #[cfg(target_os = "macos")] {
        const SHELL_PROGRAM: &str = "bash";
        const SHELL_PROGRAM_ARG: &str = "-c";
    }else {
        compile_error!("Unsupported platform.");
    }
}

type Result<T> = result::Result<T, Box<dyn error::Error>>;

#[derive(Debug, PartialEq)]
pub enum ConfigError {
    EmptyTask(String),    // Nothing to run
    FileNotFound(String), // Config File not found
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ConfigError::EmptyTask(ref s) => write!(f, "Task {} is empty.", s),
            ConfigError::FileNotFound(ref s) => write!(f, "File {} not found.", s),
        }
    }
}

impl error::Error for ConfigError {
    fn description(&self) -> &str {
        match *self {
            ConfigError::EmptyTask(_) => "nothing to run",
            ConfigError::FileNotFound(_) => "file not found",
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
    /// Name of the program to execute with command line arguments.
    /// Note that this will not work with built in command-line commands.
    program: Option<String>,
    /// To be used to call command line commands. It will launch the command line
    /// with the command and arg as command line arguments.
    command: Option<String>,
    /// Same as joining command and params in a single string.
    script: Option<String>,
    /// Params to pass to the program or command. Not accepted by script.
    params: Option<Vec<String>>,
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
            dbg!(&path);
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

impl Task {
    /// Runs the task with the given arguments.
    pub fn run(&self, args: &HashMap<String, String>) -> Result<ExitStatus> {
        let command = self.prepare_command(args)?;
        self.run_and_print_output(command)
    }

    /// Prepares the task command to run.
    fn prepare_command(&self, args: &HashMap<String, String>) -> Result<Command> {
        // TODO: Validate only one of program, command line or script is given
        let task_command = if let Some(command) = &self.command {
            // Get parsed params
            let params = self.get_parsed_params(args)?;

            // Prepare string with expected capacity
            let lengths_vec: Vec<usize> = params.iter().map(|s| s.len()).collect();
            let total_length =
                command.len() + params.len() + lengths_vec.iter().fold(0, |t, v| t + v) + 1; // space between command and params
            let mut script = String::with_capacity(total_length);

            // Joins everything as a single argument since we are passing it to a program
            script.push_str(command);
            for param in params {
                if param.is_empty() {
                    continue;
                }
                script.push_str(" ");
                script.push_str(&*param);
            }
            let mut command = Command::new(SHELL_PROGRAM);
            command.arg(SHELL_PROGRAM_ARG);
            if !script.is_empty() {
                command.arg(script);
            }
            command
        } else if let Some(script) = &self.script {
            let script = format_string(script, args);
            let mut command = Command::new(SHELL_PROGRAM);
            // TODO: Handle empty script
            command.arg(SHELL_PROGRAM_ARG).arg(script?);
            command
        } else if let Some(program) = &self.program {
            let params: Vec<String> = self.get_parsed_params(args)?;
            let mut non_empty_params: Vec<String> = Vec::with_capacity(params.len());
            for param in params {
                if !param.is_empty() {
                    non_empty_params.push(param);
                }
            }
            let mut command = Command::new(program);
            if non_empty_params.len() > 0 {
                command.args(non_empty_params);
            }
            command
        } else {
            return Err(ConfigError::EmptyTask(String::from("nothing found")))?;
        };
        Ok(task_command)
    }

    /// Runs the task, with stdout, stderr and stdin inherited.
    fn run_and_print_output(&self, mut command: Command) -> Result<ExitStatus> {
        command.stdout(Stdio::inherit());
        command.stderr(Stdio::inherit());
        command.stdin(Stdio::inherit());
        let mut child = command.spawn()?;
        Ok(child.wait()?)
    }

    /// Given a map of args, returns a vector of parsed parameters for the task.
    fn get_parsed_params(&self, args: &HashMap<String, String>) -> Result<Vec<String>> {
        let mut v: Vec<String> = Vec::new();
        if let Some(params) = &self.params {
            for param in params {
                let result = format_string(param, args)?;
                v.push(result);
            }
        }
        Ok(v)
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
            Err(ConfigError::FileNotFound(String::from("No File Found")))?
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
    pub fn get_task(&self, task_name: &str) -> Option<&Task> {
        for conf in &self.configs {
            if let Some(task) = conf.get_task(task_name) {
                return Some(task);
            }
        }
        return None;
    }
}

#[test]
fn test_format_string_unclosed_tag() {
    let config = ConfigFile::load(Path::new("src/sample.toml"));
    assert!(config.unwrap().tasks.unwrap().contains_key("echo_base"));
}

#[test]
fn test_exec() {
    // TODO: Write actual test
    let config = ConfigFile::load(Path::new("src/sample.toml"));
    let mut args: HashMap<String, String> = HashMap::new();
    args.insert(String::from("-m"), String::from("hi from python"));
    let task = &config.unwrap().tasks.unwrap()["command"];
    task.run(&args).unwrap();

    let config = ConfigFile::load(Path::new("src/sample.toml"));
    let task = &config.unwrap().tasks.unwrap()["script"];
    task.run(&args).unwrap();

    let config = ConfigFile::load(Path::new("src/sample.toml"));
    let task = &config.unwrap().tasks.unwrap()["program"];
    task.run(&args).unwrap();
}
