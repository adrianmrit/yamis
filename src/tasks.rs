use std::{env, error, fmt, fs};
use std::collections::HashMap;
use std::fs::read;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

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


#[derive(Debug, PartialEq)]
pub enum ConfigError {
    EmptyTask(String),  // Nothing to run
    FileNotFound(String) // Config File not found
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

#[derive(Debug)]
#[derive(Deserialize)]
// Do not deny for now
// #[serde(deny_unknown_fields)]
// Minimal for now
/// Represents a Task. Should have only program, command or script at the same time
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
    /// Next config to check if the one doesn't contains the required task.
    pub next: Option<Box<ConfigFile>>,
    /// Tasks inside the config file
    pub tasks: Option<HashMap<String, Task>>,
}


/// Used to discover files.
pub struct ConfigFiles {
    /// First config file to check
    entry: ConfigFile,
}


impl Task {
    /// Runs the task with the given arguments
    pub fn run(&self, args: &HashMap<String, String>) -> Result<ExitStatus, Box<dyn error::Error>> {

        let command = self.prepare_command(args)?;
        self.run_and_print_output(command)
    }

    /// Prepares the task command to run
    fn prepare_command(&self, args: &HashMap<String, String>) -> Result<Command, Box<dyn error::Error>> {
        // TODO: Validate only one of program, command line or script is given
        let task_command = if let Some(command) = &self.command {
            // Get parsed params
            let params = self.get_parsed_params(args)?;

            // Prepare string with expected capacity
            let lengths_vec: Vec<usize> = params.iter().map(|s| s.len()).collect();
            let total_length = command.len()
                + params.len()
                + lengths_vec.iter().fold(0, |t, v| t + v)
                + 1; // space between command and params
            let mut script = String::with_capacity(total_length);

            // Joins everything as a single argument since we are passing it to a program
            script.push_str(command);
            for param in params {
                if param.is_empty() {
                    continue
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
        } else if let Some(script) = &self.script{
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
            return Err(Box::new(ConfigError::EmptyTask(String::from("nothing found"))));
        };
        Ok(task_command)
    }

    /// Runs the task, with stdout, stderr and stdin inherited.
    fn run_and_print_output(&self, mut command: Command) -> Result<ExitStatus, Box<dyn error::Error>> {
        command.stdout(Stdio::inherit());
        command.stderr(Stdio::inherit());
        command.stdin(Stdio::inherit());
        let mut child = command.spawn()?;
        Ok(child.wait()?)
    }

    /// Given a map of args, returns a vector of parsed parameters for the task.
    fn get_parsed_params(&self, args: &HashMap<String, String>) -> Result<Vec<String>, Box<dyn error::Error>> {
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
    pub fn load(path: &Path) -> Result<ConfigFile, Box<dyn error::Error>> {
        let contents = fs::read_to_string(&path)?;
        let mut conf: ConfigFile = toml::from_str(&*contents)?;
        conf.filepath = path.to_str().unwrap().to_string();
        conf.set_next()?;
        Ok(conf)
    }

    // TODO: Consider lazily loading next
    /// Sets the next config file.
    fn set_next<'b>(&'b mut self) -> Result<(), Box<dyn error::Error>>{
        let path = Path::new(&self.filepath);
        if path.ends_with(ROOT_PROJECT_CONF_NAME){
            return Ok(());
        }

        if let Some(parent) = path.parent() {
            for path in parent.ancestors() {
                for name in CONFIG_FILES_PRIO {
                    let config_file_path = path.join(name);
                    if config_file_path.is_file() {
                        let mut config = ConfigFile::load(&config_file_path)?;
                        self.next = Some(Box::new(config));
                    }
                }
            }
        }
        return Ok(());
    }

    /// Finds a task by name on this config file or the next
    fn get_task(&self, task_name: &str) -> Option<&Task> {
        if let Some(tasks) = &self.tasks {
            if let Some(task) = tasks.get(task_name) {
                return Some(task);
            }
        } else if let Some(next) = &self.next {
            return next.get_task(task_name);
        }
        return None;
    }
}

impl ConfigFiles {
    /// Discovers the config files.
    pub fn discover() -> Result<ConfigFiles, Box<dyn error::Error>>{
        let working_dir = env::current_dir()?;
        for dir in working_dir.ancestors() {
            for conf_name in CONFIG_FILES_PRIO {
                let config_path = dir.join(conf_name);
                if config_path.is_file() {
                    let config = ConfigFile::load(config_path.as_path())?;
                    return Ok(ConfigFiles {entry: config});
                }
            }
        }
        Err(Box::new(ConfigError::FileNotFound(String::from("No File Found"))))
    }

    /// Returns a task for the given name
    pub fn get_task(&self, task_name: &str) -> Option<&Task> {
        return self.entry.get_task(task_name);
    }
}

#[test]
fn test_format_string_unclosed_tag(){
    let config = ConfigFile::load(Path::new("src/sample.toml"));
    assert!(config.unwrap().tasks.unwrap().contains_key("echo_base"));
}

#[test]
fn test_exec(){
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