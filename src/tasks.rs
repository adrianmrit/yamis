use std::{error, fmt, fs};
use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

use serde_derive::Deserialize;

use crate::args::format_string;

const ROOT_PROJECT_CONF_NAME: &str = "yamis.project.toml";
const CONF_NAME: &str = "yamis.toml";
const PRIVATE_CONF_NAME: &str = "yamis.local.toml";
const CONFIG_FILES_PRIO: &[&str] = &["yamis.local.toml", "yamis.toml", "yamis.project.toml"];

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
pub enum TaskError {
    Empty(String),  // Nothing to run
}

impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TaskError::Empty(ref s) => write!(f, "Task {} is empty.", s),
        }
    }
}

impl error::Error for TaskError {
    fn description(&self) -> &str {
        match *self {
            TaskError::Empty(_) => "nothing to run",
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
/// Repressents a config file.
pub struct ConfigFile {
    #[serde(skip)]
    filepath: String,
    /// Tasks inside the conig file
    pub tasks: Option<HashMap<String, Task>>,
}


impl Task {
    /// Runs the task with the given arguments
    pub fn run(&self, args: &HashMap<String, String>) -> Result<ExitStatus, Box<dyn error::Error>> {

        let command = self.prepare_command(args)?;
        self.run_and_print_output(command)
    }

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
            return Err(Box::new(TaskError::Empty(String::from("nothing found"))));
        };
        Ok(task_command)
    }

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
    pub fn load<P: AsRef<Path>>(path: P) -> Result<ConfigFile, Box<dyn error::Error>> {
        let contents = fs::read_to_string(&path)?;
        let conf: ConfigFile = toml::from_str(&*contents)?;
        Ok(conf)
    }
}

#[test]
fn test_format_string_unclosed_tag(){
    let config = ConfigFile::load("src/sample.toml");
    assert!(config.unwrap().tasks.unwrap().contains_key("echo_base"));
}

#[test]
fn test_exec(){
    // TODO: Write actual test
    let config = ConfigFile::load("src/sample.toml");
    let mut args: HashMap<String, String> = HashMap::new();
    args.insert(String::from("-m"), String::from("hi from python"));
    let task = &config.unwrap().tasks.unwrap()["command"];
    task.run(&args).unwrap();

    let config = ConfigFile::load("src/sample.toml");
    let task = &config.unwrap().tasks.unwrap()["script"];
    task.run(&args).unwrap();

    let config = ConfigFile::load("src/sample.toml");
    let task = &config.unwrap().tasks.unwrap()["program"];
    task.run(&args).unwrap();
}