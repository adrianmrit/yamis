use std::collections::HashMap;
use std::env::temp_dir;
use std::fs::File;
use std::io::Write;
use std::mem;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::args::ArgsMap;
use crate::args_format::{format_arg, format_script, EscapeMode};
use crate::config_files::{ConfigError, ConfigFile, ConfigFiles};
use crate::defaults::{default_false, default_true};
use serde_derive::Deserialize;
use uuid::Uuid;

use crate::types::DynErrResult;
use crate::utils::{get_path_relative_to_base, read_env_file};

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

/// Represents a Task
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    #[serde(skip)]
    name: String,
    /// Whether to automatically quote argument with spaces
    quote: Option<String>,
    /// Script to run
    script: Option<String>,
    /// A program to run
    program: Option<String>,
    /// Args to pass to a command
    args: Option<Vec<String>>,
    /// If given, runs all those tasks at once
    serial: Option<Vec<String>>,
    /// Env variables for the task
    #[serde(default)]
    pub(crate) env: HashMap<String, String>,
    /// Env file to read environment variables from
    env_file: Option<String>,
    /// Working dir
    wd: Option<String>,
    /// Task to run instead if the OS is linux
    pub(crate) linux: Option<Box<Task>>,
    /// Task to run instead if the OS is windows
    pub(crate) windows: Option<Box<Task>>,
    /// Task to run instead if the OS is macos
    pub(crate) macos: Option<Box<Task>>,
    /// Base task to inherit from
    #[serde(default)]
    pub(crate) bases: Vec<String>,
    /// If true, env is merged during inheritance
    #[serde(default = "default_true")]
    merge_env: bool,
    /// If private, it cannot be called
    #[serde(default = "default_false")]
    private: bool,
}

cfg_if::cfg_if! {
    if #[cfg(target_os = "windows")] {
        fn create_script_file<P: AsRef<Path>>(path: P) -> DynErrResult<File> {
            Ok(File::create(&path)?)
        }
    } else {
        use std::os::unix::fs::OpenOptionsExt;
        fn create_script_file<P: AsRef<Path>>(path: P) -> DynErrResult<File> {
            Ok(fs::OpenOptions::new()
            .create(true)
            .write(true)
            .mode(0o770)  // Create with appropriate permission
            .open(path)?)
        }
    }
}

/// Creates a temporal script returns the path to it.
/// The OS should take care of cleaning the file.
///
/// # Arguments
///
/// * `content` - Content of the script file
fn get_temp_script(content: String) -> DynErrResult<PathBuf> {
    let mut path = temp_dir();

    // Alternatives to uuid are timestamp and random number, or those together,
    // so this might change in the future.
    let file_name = format!("{}.yamis.{}", Uuid::new_v4(), SCRIPT_EXTENSION);
    path.push(file_name);

    let mut file = create_script_file(&path)?;
    file.write_all(content.as_bytes())?;
    Ok(path)
}

/// Shortcut to inherit values from the task
macro_rules! inherit_value {
    ( $task:expr, $base:expr ) => {
        if $task.is_none() && $base.is_some() {
            $task = $base.clone();
        }
    };
}

impl Task {
    /// Does extra setup on the task and does some validation.
    ///
    /// # Arguments
    ///
    /// * `name`: name of the task
    /// * `base_path`: path to use as a reference to resolve relative paths
    ///
    /// returns: Result<(), Box<dyn Error, Global>>
    ///
    /// # Examples
    ///
    pub(crate) fn setup(&mut self, name: &str, base_path: &Path) -> DynErrResult<()> {
        self.name = String::from(name);
        self.load_env_file(base_path)?;
        Ok(self.validate()?)
    }

    /// Extends from the given task.
    ///
    /// # Arguments
    ///
    /// * `base_task`: task to extend from
    ///
    /// returns: ()
    ///
    pub(crate) fn extend_task(&mut self, base_task: &Task) {
        inherit_value!(self.quote, base_task.quote);
        inherit_value!(self.script, base_task.script);
        inherit_value!(self.program, base_task.program);
        inherit_value!(self.args, base_task.args);
        inherit_value!(self.serial, base_task.serial);
        inherit_value!(self.env_file, base_task.env_file);

        if self.merge_env && !base_task.env.is_empty() {
            let old_env = mem::replace(&mut self.env, base_task.env.clone());

            for (key, val) in old_env {
                self.env.insert(key, val);
            }
        } else if self.env.is_empty() {
            self.env.extend(base_task.env.clone());
        }
    }

    /// Loads the environment file contained between this task
    ///
    /// # Arguments
    ///
    /// * `base_path`: path to use as a reference to resolve relative paths
    ///
    /// returns: Result<(), Box<dyn Error, Global>>
    fn load_env_file(&mut self, base_path: &Path) -> DynErrResult<()> {
        // removes the env_file as we won't need it again
        let env_file = mem::replace(&mut self.env_file, None);
        if let Some(env_file) = env_file {
            let env_file = get_path_relative_to_base(base_path, &env_file);
            let env_variables = read_env_file(env_file.as_path())?;
            for (key, val) in env_variables {
                self.env.entry(key).or_insert(val);
            }
        }
        Ok(())
    }

    /// Validates the task configuration.
    ///
    /// # Arguments
    ///  
    /// * `name` - Name of the task
    fn validate(&self) -> Result<(), ConfigError> {
        if self.script.is_some() && self.program.is_some() {
            return Err(ConfigError::BadTask(
                self.name.clone(),
                String::from("Task cannot specify `script` and `program` at the same time."),
            ));
        }

        if self.script.is_some() && self.serial.is_some() {
            return Err(ConfigError::BadTask(
                self.name.clone(),
                String::from("Cannot specify `script` and `serial` at the same time."),
            ));
        }

        if self.program.is_some() && self.serial.is_some() {
            return Err(ConfigError::BadTask(
                self.name.clone(),
                String::from("Cannot specify `program` and `serial` at the same time."),
            ));
        }

        if self.script.is_some() && self.args.is_some() {
            return Err(ConfigError::BadTask(
                self.name.clone(),
                String::from("Cannot specify `args` on scripts."),
            ));
        }

        if self.program.is_some() && self.quote.is_some() {
            return Err(ConfigError::BadTask(
                self.name.clone(),
                String::from("Cannot specify `quote` on commands."),
            ));
        }
        Ok(())
    }

    /// Sets common parameters for commands, like stdout, stderr, stdin, working directory and
    /// environment variables.
    ///
    /// # Arguments
    ///  
    /// * `command` - Command to set the parameters for
    /// * `config_file` - Configuration file
    fn set_command_basics(
        &self,
        command: &mut Command,
        config_file: &ConfigFile,
    ) -> DynErrResult<()> {
        command.stdout(Stdio::inherit());
        command.stderr(Stdio::inherit());
        command.stdin(Stdio::inherit());

        let config_file_folder = config_file.filepath.parent().unwrap();

        match &self.wd {
            None => {}
            Some(wd) => {
                let wd = get_path_relative_to_base(config_file_folder, wd);
                command.current_dir(wd);
            }
        };

        if let Some(env_file) = &config_file.env_file {
            let env_file = get_path_relative_to_base(config_file_folder, env_file);
            let env_variables = read_env_file(env_file.as_path())?;
            command.envs(env_variables);
        }

        match &config_file.env {
            None => {}
            Some(env) => {
                command.envs(env);
            }
        }

        command.envs(&self.env);
        Ok(())
    }

    /// Spawns a command and waits for its execution.
    ///
    /// # Arguments
    ///  
    /// * `command` - Command to spawn
    fn spawn_command(&self, command: &mut Command) -> DynErrResult<()> {
        let mut child = command.spawn()?;

        // let child handle ctrl-c to prevent dropping the parent and leaving the child running
        ctrlc::set_handler(move || {}).unwrap_or(());

        child.wait()?;
        Ok(())
    }

    /// Runs a program from a task.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the task, displayed in errors.
    /// * `args` - Arguments to format the task args with
    /// * `config_file` - Configuration file of the task
    fn run_program(&self, args: &ArgsMap, config_file: &ConfigFile) -> DynErrResult<()> {
        let program = self.program.as_ref().unwrap();
        let mut command = Command::new(program);
        self.set_command_basics(&mut command, config_file)?;

        if let Some(task_args) = &self.args {
            for task_arg in task_args {
                match format_arg(task_arg, args) {
                    Ok(task_args) => {
                        command.args(task_args);
                    }
                    Err(e) => {
                        return Err(ConfigError::BadTask(self.name.clone(), e.to_string()).into());
                    }
                }
            }
        }

        self.spawn_command(&mut command)
    }

    /// Runs a script from a task.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the task, displayed in errors.
    /// * `args` - Arguments to format the task args with
    /// * `config_file` - Configuration file of the task
    fn run_script(&self, args: &ArgsMap, config_file: &ConfigFile) -> DynErrResult<()> {
        let script = self.script.as_ref().unwrap();
        let mut command = Command::new(SHELL_PROGRAM);
        command.arg(SHELL_PROGRAM_ARG);

        self.set_command_basics(&mut command, config_file)?;

        let (quote_from_file, quote) = match &self.quote {
            None => (true, &config_file.quote),
            Some(quote) => (false, quote),
        };
        let quote = match quote.to_lowercase().as_str() {
            "always" => EscapeMode::Always,
            "never" => EscapeMode::Never,
            "spaces" => EscapeMode::OnSpace,
            _ => {
                let plain_val = match &self.quote {
                    None => &config_file.quote,
                    Some(val) => val,
                };
                let error = format!(
                    "Invalid quote option `{}`. Allowed values are `always`, `never` and `spaces`",
                    plain_val
                );

                return if quote_from_file {
                    Err(ConfigError::BadConfigFile(error).into())
                } else {
                    Err(ConfigError::BadTask(self.name.clone(), error).into())
                };
            }
        };

        match format_script(script, args, quote) {
            Ok(script) => {
                let script_file = get_temp_script(script)?;
                command.arg(script_file.to_str().unwrap());
            }
            Err(e) => {
                return Err(ConfigError::BadTask(self.name.clone(), e.to_string()).into());
            }
        }

        self.spawn_command(&mut command)
    }

    /// Runs a series of tasks from a task, in order.
    ///
    /// # Arguments
    /// * `args` - Arguments to format the task args with
    /// * `config_file` - Configuration file of the task
    fn run_serial(&self, args: &ArgsMap, config_files: &ConfigFiles) -> DynErrResult<()> {
        let serial = self.serial.as_ref().unwrap();
        let mut tasks: Vec<(&Task, &ConfigFile)> = Vec::new();
        for task_name in serial {
            if let Some((task, task_config_file)) = config_files.get_system_task(task_name) {
                tasks.push((task, task_config_file));
            } else {
                return Err(
                    ConfigError::BadConfigFile(format!("Task `{}` not found.", task_name)).into(),
                );
            }
        }
        for (task, task_config_file) in tasks {
            task.run(args, task_config_file, config_files)?;
        }
        Ok(())
    }

    /// Runs a task.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the task, displayed in errors.
    /// * `args` - Arguments to format the task args with
    /// * `config_file` - Configuration file of the task
    /// * `config_files` - global ConfigurationFiles instance
    pub fn run(
        &self,
        args: &ArgsMap,
        config_file: &ConfigFile,
        config_files: &ConfigFiles,
    ) -> DynErrResult<()> {
        if self.private {
            return Err(format!("Cannot run private task {}", self.name).into());
        }
        return if self.script.is_some() {
            self.run_script(args, config_file)
        } else if self.program.is_some() {
            self.run_program(args, config_file)
        } else if self.serial.is_some() {
            self.run_serial(args, config_files)
        } else {
            Err(ConfigError::BadTask(self.name.clone(), String::from("Nothing to run.")).into())
        };
    }
}
