use dotenv_parser::parse_dotenv;
use std::collections::{BTreeMap, HashMap};
use std::env::temp_dir;
use std::ffi::OsStr;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{env, error, fmt, fs};

use crate::args::ArgsMap;
use crate::args_format::{format_arg, format_script, EscapeMode};
use serde_derive::Deserialize;
use uuid::Uuid;

use crate::types::DynErrResult;

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

/// Errors related to config files and tasks
#[derive(Debug, PartialEq)]
pub enum ConfigError {
    /// Raised when a config file is not found for a given path
    FileNotFound(String), // Given config file not found
    /// Raised when no config file is found during auto-discovery
    NoConfigFile, // No config file was discovered
    /// Bad Config error
    BadConfigFile(String),
    /// Raised when there is an error in a task
    BadTask(String, String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ConfigError::FileNotFound(ref s) => write!(f, "File {} not found.", s),
            ConfigError::NoConfigFile => write!(f, "No config file found."),
            ConfigError::BadConfigFile(ref s) => write!(f, "Bad config file. {}", s),
            ConfigError::BadTask(ref name, ref reason) => {
                write!(f, "Error on tasks.{}:\n    {}", name, reason)
            }
        }
    }
}

impl error::Error for ConfigError {
    fn description(&self) -> &str {
        match *self {
            ConfigError::FileNotFound(_) => "file not found",
            ConfigError::NoConfigFile => "no config discovered",
            ConfigError::BadConfigFile(_) => "bad config file",
            ConfigError::BadTask(_, _) => "bad task",
        }
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        None
    }
}

#[derive(Debug, Deserialize)]
/// Represents a Task
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
    env: Option<HashMap<String, String>>,
    /// Env file to read environment variables from
    env_file: Option<String>,
    /// Working dir
    wd: Option<String>,
    /// Task to run instead if the OS is linux
    linux: Option<Box<Task>>,
    /// Task to run instead if the OS is windows
    windows: Option<Box<Task>>,
    /// Task to run instead if the OS is macos
    macos: Option<Box<Task>>,
}

fn default_quote() -> String {
    String::from("always")
}

#[derive(Debug, Deserialize)]
// TODO: Deny invalid fields
// #[serde(deny_unknown_fields)]
/// Represents a config file.
pub struct ConfigFile {
    /// Path of the file.
    #[serde(skip)]
    filepath: PathBuf,
    /// Whether to automatically quote argument with spaces unless task specified
    #[serde(default = "default_quote")]
    quote: String,
    /// Tasks inside the config file.
    tasks: Option<HashMap<String, Task>>,
    /// Env variables for all the tasks.
    env: Option<HashMap<String, String>>,
    /// Env file to read environment variables from
    env_file: Option<String>,
}

/// Used to discover files.
pub struct ConfigFiles {
    /// First config file to check.
    pub configs: Vec<ConfigFile>,
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

/// Returns the path relative to the base. If path is already absolute, it will be returned instead.
///
/// # Arguments
///
/// * `base`: Base path
/// * `path`: Path to make relative to the base
///
/// returns: PathBuf
fn get_path_relative_to_base<B: AsRef<OsStr> + ?Sized, P: AsRef<OsStr> + ?Sized>(
    base: &B,
    path: &P,
) -> PathBuf {
    let path = Path::new(path);
    if !path.is_absolute() {
        let base = Path::new(base);
        return base.join(path);
    }
    path.to_path_buf()
}

/// Reads the content of an environment file from the given path and returns a BTreeMap.
///
/// # Arguments
/// * `path`: Path of the environment file
///
/// returns: DynErrResult<BTreeMap<String, String>>
fn read_env_file<S: AsRef<OsStr> + ?Sized>(path: &S) -> DynErrResult<BTreeMap<String, String>> {
    let path = Path::new(path);
    Ok(match fs::read_to_string(path) {
        Ok(file_contents) => match parse_dotenv(&file_contents) {
            Ok(result) => result,
            Err(e) => return Err(e),
        },
        Err(e) => {
            return Err(format!(
                "There was an error reading the env file at {}:\n{}",
                path.display(),
                e
            )
            .into())
        }
    })
}

impl Task {
    fn setup(&mut self, name: &str) {
        self.name = String::from(name);
    }

    /// Validates the task configuration.
    ///
    /// # Arguments
    ///  
    /// * `name` - Name of the task
    pub fn validate(&self, name: &str) -> Result<(), ConfigError> {
        if self.script.is_some() && self.program.is_some() {
            return Err(ConfigError::BadTask(
                String::from(name),
                String::from("Task cannot specify `script` and `program` at the same time."),
            ));
        }
        if self.script.is_some() && self.serial.is_some() {
            return Err(ConfigError::BadTask(
                String::from(name),
                String::from("Cannot specify `script` and `serial` at the same time."),
            ));
        }

        if self.program.is_some() && self.serial.is_some() {
            return Err(ConfigError::BadTask(
                String::from(name),
                String::from("Cannot specify `program` and `serial` at the same time."),
            ));
        }
        if self.script.is_some() && self.args.is_some() {
            return Err(ConfigError::BadTask(
                String::from(name),
                String::from("Cannot specify `args` on scripts."),
            ));
        }

        if self.program.is_some() && self.quote.is_some() {
            return Err(ConfigError::BadTask(
                String::from(name),
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

        if let Some(env_file) = &self.env_file {
            let env_file = get_path_relative_to_base(config_file_folder, env_file);
            let env_variables = read_env_file(env_file.as_path())?;
            command.envs(env_variables);
        }

        match &self.env {
            None => {}
            Some(env) => {
                command.envs(env);
            }
        }
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
    fn run_program(
        &self,
        name: &str,
        args: &ArgsMap,
        config_file: &ConfigFile,
    ) -> DynErrResult<()> {
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
                        return Err(ConfigError::BadTask(String::from(name), e.to_string()).into());
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
    fn run_script(&self, name: &str, args: &ArgsMap, config_file: &ConfigFile) -> DynErrResult<()> {
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
                    Err(ConfigError::BadTask(String::from(name), error).into())
                };
            }
        };

        match format_script(script, args, quote) {
            Ok(script) => {
                let script_file = get_temp_script(script)?;
                command.arg(script_file.to_str().unwrap());
            }
            Err(e) => {
                return Err(ConfigError::BadTask(String::from(name), e.to_string()).into());
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
        let mut tasks: Vec<(String, &Task, &ConfigFile)> = Vec::new();
        for task_name in serial {
            if let Some((task_name, task, task_config_file)) = config_files.get_task(task_name) {
                tasks.push((task_name, task, task_config_file));
            } else {
                return Err(
                    ConfigError::BadConfigFile(format!("Task `{}` not found.", task_name)).into(),
                );
            }
        }
        for (task_name, task, task_config_file) in tasks {
            task.run(&task_name, args, task_config_file, config_files)?;
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
        name: &str,
        args: &ArgsMap,
        config_file: &ConfigFile,
        config_files: &ConfigFiles,
    ) -> DynErrResult<()> {
        return if self.script.is_some() {
            self.run_script(name, args, config_file)
        } else if self.program.is_some() {
            self.run_program(name, args, config_file)
        } else if self.serial.is_some() {
            self.run_serial(args, config_files)
        } else {
            Err(ConfigError::BadTask(String::from(name), String::from("Nothing to run.")).into())
        };
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
        conf.setup_tasks();
        conf.filepath = path.to_path_buf();
        Ok(conf)
    }

    fn setup_tasks(&mut self) {
        if let Some(tasks) = &mut self.tasks {
            for (name, task) in tasks {
                task.setup(name);
            }
        }
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
    pub fn get_task(&self, task_name: &str) -> Option<(String, &Task, &ConfigFile)> {
        for conf in &self.configs {
            if let Some(task) = conf.get_task(task_name) {
                if env::consts::OS == "linux" {
                    if let Some(linux_task) = &task.linux {
                        return Some((format!("{}.linux", task_name), &*linux_task, conf));
                    }
                } else if env::consts::OS == "windows" {
                    if let Some(windows_task) = &task.windows {
                        return Some((format!("{}.windows", task_name), &*windows_task, conf));
                    }
                } else if env::consts::OS == "macos" {
                    if let Some(macos_task) = &task.macos {
                        return Some((format!("{}.macos", task_name), &*macos_task, conf));
                    }
                }
                return Some((String::from(task_name), task, conf));
            }
        }
        None
    }
}
