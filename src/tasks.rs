use std::collections::HashMap;
use std::env::temp_dir;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::{error, fmt, fs, mem};

use crate::config_files::ConfigFile;
use crate::defaults::default_false;
use crate::parser::{parse_params, parse_script, EscapeMode};
use serde_derive::Deserialize;

use crate::types::{DynErrResult, TaskArgs};
use crate::utils::{get_path_relative_to_base, read_env_file, TMP_FOLDER_NAMESPACE};
use md5::{Digest, Md5};

cfg_if::cfg_if! {
    if #[cfg(target_os = "windows")] {
        // Will run the actual script in CMD, but we don't need to specify /C option
        const DEFAULT_INTERPRETER: &str = "powershell";
        const DEFAULT_SCRIPT_EXTENSION: &str = "cmd";
    } else if #[cfg(target_os = "linux")] {
        const DEFAULT_INTERPRETER: &str = "bash";
        const DEFAULT_SCRIPT_EXTENSION: &str = "sh";
    } else if #[cfg(target_os = "macos")] {
        const DEFAULT_INTERPRETER: &str = "bash";
        const DEFAULT_SCRIPT_EXTENSION: &str = "sh";
    }else {
        compile_error!("Unsupported platform.");
    }
}

/// Task errors
#[derive(Debug, PartialEq, Eq)]
pub enum TaskError {
    /// Raised when there is an error running a task
    RuntimeError(String, String),
    /// Raised when the task is improperly configured
    ImproperlyConfigured(String, String),
}

impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TaskError::RuntimeError(ref name, ref reason) => {
                write!(f, "Error running tasks.{}:\n{}", name, reason)
            }
            TaskError::ImproperlyConfigured(ref name, ref reason) => {
                write!(f, "Improperly configured tasks.{}:\n{}", name, reason)
            }
        }
    }
}

impl error::Error for TaskError {}

/// Represents a Task
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    /// Name of the task
    #[serde(skip)]
    name: String,
    /// Help of the task
    help: Option<String>,
    /// Whether to automatically quote argument with spaces
    quote: Option<EscapeMode>,
    /// Script to run
    script: Option<String>,
    /// Interpreter program to use
    script_runner: Option<String>,
    /// Extra arguments to pass to the script runner
    script_runner_args: Option<Vec<String>>,
    /// Script extension
    #[serde(alias = "script_extension")]
    script_ext: Option<String>,
    /// A program to run
    program: Option<String>,
    /// Args to pass to a command
    args: Option<Vec<String>>,
    /// Extends args from bases
    #[serde(alias = "args+")]
    args_extend: Option<Vec<String>>,
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
        use std::fs::OpenOptions;
        fn create_script_file<P: AsRef<Path>>(path: P) -> DynErrResult<File> {
            Ok(OpenOptions::new()
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
fn get_temp_script(
    content: &str,
    extension: &str,
    task_name: &str,
    config_file_path: &Path,
) -> DynErrResult<PathBuf> {
    let mut path = temp_dir();
    path.push(TMP_FOLDER_NAMESPACE);
    fs::create_dir_all(&path)?;

    let extension = if extension.is_empty() {
        String::new()
    } else if extension.starts_with('.') {
        String::from(extension)
    } else {
        format!(".{}", extension)
    };

    // get md5 hash of the task_name, config_file_path and content
    let mut hasher = Md5::new();
    hasher.update(task_name.as_bytes());
    hasher.update(config_file_path.to_str().unwrap().as_bytes());
    hasher.update(content.as_bytes());
    let hash = hasher.finalize();

    let file_name = format!("{:X}{}", hash, extension);
    path.push(file_name);
    dbg!(&path);

    // Uses the temp file as a cache, so it doesn't have to create it every time
    // we run the same script.
    if path.exists() {
        return Ok(path);
    }
    let mut file = create_script_file(&path)?;
    file.write_all(content.as_bytes())?;
    Ok(path)
}

/// Shortcut to inherit values from the task
macro_rules! inherit_value {
    ( $from_task:expr, $from_base:expr ) => {
        if $from_task.is_none() && $from_base.is_some() {
            $from_task = $from_base.clone();
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
        if self.quote.is_none() && base_task.quote.is_some() {
            self.quote = Some(base_task.quote.as_ref().unwrap().clone());
        }
        inherit_value!(self.help, base_task.help);
        inherit_value!(self.script, base_task.script);
        inherit_value!(self.script_runner, base_task.script_runner);
        inherit_value!(self.script_runner_args, base_task.script_runner_args);
        inherit_value!(self.script_ext, base_task.script_ext);
        inherit_value!(self.program, base_task.program);
        inherit_value!(self.args, base_task.args);
        inherit_value!(self.serial, base_task.serial);
        inherit_value!(self.env_file, base_task.env_file);

        if !base_task.env.is_empty() {
            let old_env = mem::replace(&mut self.env, base_task.env.clone());

            for (key, val) in old_env {
                self.env.insert(key, val);
            }
        } else if self.env.is_empty() {
            self.env.extend(base_task.env.clone());
        }

        if self.args_extend.is_some() {
            let new_args = mem::replace(&mut self.args_extend, None).unwrap();
            if self.args.is_none() {
                self.args = mem::replace(&mut self.args, Some(Vec::<String>::new()));
            }
            if let Some(args) = &mut self.args {
                args.extend(new_args);
            } else {
                self.args = Some(new_args);
            }
        }
    }

    /// Returns the name of the task
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Returns weather the task is private or not
    pub fn is_private(&self) -> bool {
        self.private
    }

    /// Returns the help for the task
    pub fn get_help(&self) -> &str {
        match self.help {
            Some(ref help) => help.trim(),
            None => "",
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

    /// Returns the environment variables by merging the ones from the config file with
    /// the ones from the task, where the task takes precedence.
    ///
    /// # Arguments
    ///
    /// * `config_file`: Config file to load extra environment variables from
    ///
    /// returns: HashMap<String, String, RandomState>
    fn get_env(&self, config_file: &ConfigFile) -> HashMap<String, String> {
        let mut env = self.env.clone();
        if let Some(config_file_env) = &config_file.env {
            for (key, val) in config_file_env {
                env.entry(key.clone()).or_insert_with(|| val.clone());
            }
        }
        env
    }

    /// Validates the task configuration.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the task
    fn validate(&self) -> Result<(), TaskError> {
        if self.script.is_some() && self.program.is_some() {
            return Err(TaskError::ImproperlyConfigured(
                self.name.clone(),
                String::from("Cannot specify `script` and `program` at the same time."),
            ));
        }

        if self.script_runner.is_some() && self.script_runner.as_ref().unwrap().is_empty() {
            return Err(TaskError::ImproperlyConfigured(
                self.name.clone(),
                String::from("`script_runner` parameter cannot be an empty string."),
            ));
        }

        if self.script.is_some() && self.serial.is_some() {
            return Err(TaskError::ImproperlyConfigured(
                self.name.clone(),
                String::from("Cannot specify `script` and `serial` at the same time."),
            ));
        }

        if self.program.is_some() && self.serial.is_some() {
            return Err(TaskError::ImproperlyConfigured(
                self.name.clone(),
                String::from("Cannot specify `program` and `serial` at the same time."),
            ));
        }

        if self.script.is_some() && self.args.is_some() {
            return Err(TaskError::ImproperlyConfigured(
                self.name.clone(),
                String::from("Cannot specify `args` on scripts."),
            ));
        }

        if (self.program.is_some() | self.serial.is_some()) && self.quote.is_some() {
            return Err(TaskError::ImproperlyConfigured(
                self.name.clone(),
                String::from("`quote` parameter can only be set for scripts."),
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

        let config_file_folder = config_file.directory();

        let wd = match &self.wd {
            None => config_file.working_directory(),
            Some(wd) => Some(get_path_relative_to_base(config_file_folder, wd)),
        };

        if let Some(wd) = wd {
            command.current_dir(wd);
        }

        Ok(())
    }

    /// Spawns a command and waits for its execution.
    ///
    /// # Arguments
    ///
    /// * `command` - Command to spawn
    fn spawn_command(&self, command: &mut Command) -> DynErrResult<()> {
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) => {
                return Err(TaskError::RuntimeError(self.name.clone(), format!("{}", e)).into());
            }
        };

        // let child handle ctrl-c to prevent dropping the parent and leaving the child running
        ctrlc::set_handler(move || {}).unwrap_or(());

        let result = child.wait()?;
        match result.success() {
            true => Ok(()),
            false => match result.code() {
                None => Err(TaskError::RuntimeError(
                    self.name.clone(),
                    String::from("Process did not terminate correctly"),
                )
                .into()),
                Some(code) => Err(TaskError::RuntimeError(
                    self.name.clone(),
                    format!("Process terminated with exit code {}", code),
                )
                .into()),
            },
        }
    }

    /// Runs a program from a task.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the task, displayed in errors.
    /// * `args` - Arguments to format the task args with
    /// * `config_file` - Configuration file of the task
    fn run_program(&self, args: &TaskArgs, config_file: &ConfigFile) -> DynErrResult<()> {
        let program = self.program.as_ref().unwrap();
        let mut command = Command::new(program);
        self.set_command_basics(&mut command, config_file)?;

        let env = self.get_env(config_file);
        command.envs(&env);

        if let Some(task_args) = &self.args {
            match parse_params(task_args, args, &env) {
                Ok(task_args) => {
                    // Programs need to exclude empty arguments, otherwise they might be passed as real parameters
                    command.args(task_args.iter().filter(|val| !val.is_empty()));
                }
                Err(e) => {
                    return Err(
                        TaskError::ImproperlyConfigured(self.name.clone(), e.to_string()).into(),
                    );
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
    fn run_script(&self, args: &TaskArgs, config_file: &ConfigFile) -> DynErrResult<()> {
        let script = self.script.as_ref().unwrap();

        // Interpreter is a list, because sometimes there is need to pass extra arguments to the
        // interpreter, such as the /C option in the batch case
        let script_runner = if let Some(script_runner) = &self.script_runner {
            script_runner
        } else {
            DEFAULT_INTERPRETER
        };

        let default_script_extension = String::from(DEFAULT_SCRIPT_EXTENSION);
        let script_extension = self
            .script_ext
            .as_ref()
            .unwrap_or(&default_script_extension);

        let mut command = Command::new(script_runner);

        if let Some(script_runner_args) = &self.script_runner_args {
            command.args(script_runner_args);
        }

        let env = self.get_env(config_file);
        command.envs(&env);

        self.set_command_basics(&mut command, config_file)?;

        let quote = if self.quote.is_some() {
            self.quote.as_ref().unwrap()
        } else {
            &config_file.quote
        };

        match parse_script(script, args, &env, quote) {
            Ok(script) => {
                let script_file = get_temp_script(
                    &script,
                    script_extension,
                    &self.name,
                    config_file.filepath.as_path(),
                )?;
                command.arg(script_file.to_str().unwrap());
            }
            Err(e) => {
                return Err(
                    TaskError::ImproperlyConfigured(self.name.clone(), e.to_string()).into(),
                );
            }
        }

        self.spawn_command(&mut command)
    }

    /// Runs a series of tasks from a task, in order.
    ///
    /// # Arguments
    /// * `args` - Arguments to format the task args with
    /// * `config_file` - Configuration file of the task
    fn run_serial(&self, args: &TaskArgs, config_file: &ConfigFile) -> DynErrResult<()> {
        let serial = self.serial.as_ref().unwrap();
        let mut tasks: Vec<Arc<Task>> = Vec::new();
        for task_name in serial {
            if let Some(task) = config_file.get_task(task_name) {
                tasks.push(task);
            } else {
                return Err(TaskError::RuntimeError(
                    self.name.clone(),
                    format!("Task `{}` not found.", task_name),
                )
                .into());
            }
        }
        for task in tasks {
            task.run(args, config_file)?;
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
    pub fn run(&self, args: &TaskArgs, config_file: &ConfigFile) -> DynErrResult<()> {
        return if self.script.is_some() {
            self.run_script(args, config_file)
        } else if self.program.is_some() {
            self.run_program(args, config_file)
        } else if self.serial.is_some() {
            self.run_serial(args, config_file)
        } else {
            Err(
                TaskError::ImproperlyConfigured(self.name.clone(), String::from("Nothing to run."))
                    .into(),
            )
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_files::ConfigFile;
    use assert_fs::TempDir;
    use std::collections::HashMap;
    use std::fs;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;

    pub fn get_task(
        name: &str,
        definition: &str,
        base_path: Option<&Path>,
    ) -> Result<Task, Box<dyn std::error::Error>> {
        let mut task: Task = toml::from_str(definition).unwrap();
        task.setup(name, base_path.unwrap_or_else(|| Path::new("")))?;
        Ok(task)
    }

    #[test]
    fn test_env_inheritance() {
        let tmp_dir = TempDir::new().unwrap();
        let config_file_path = tmp_dir.join("project.yamis.toml");
        let mut file = File::create(&config_file_path).unwrap();
        file.write_all(
            r#"
    [tasks.hello_base.env]
    greeting = "hello world"

    [tasks.calc_base.env]
    one_plus_one = "2"

    [tasks.hello]
    bases = ["hello_base", "calc_base"]
    script = "echo $greeting, 1+1=$one_plus_one"

    [tasks.hello.windows]
    bases = ["hello_base", "calc_base"]
    script = "echo %greeting%, 1+1=%one_plus_one%"
    "#
            .as_bytes(),
        )
        .unwrap();

        let config_file = ConfigFile::load(config_file_path).unwrap();

        let task = config_file.get_task("hello").unwrap();

        let env = task.get_env(&config_file);
        let expected = HashMap::from([
            ("greeting".to_string(), "hello world".to_string()),
            ("one_plus_one".to_string(), "2".to_string()),
        ]);
        assert_eq!(env, expected);
    }

    #[test]
    fn test_quotes_inheritance() {
        let tmp_dir = TempDir::new().unwrap();
        let config_file_path = tmp_dir.join("project.yamis.toml");
        let mut file = File::create(&config_file_path).unwrap();
        file.write_all(
            r#"
    [tasks.hello_base]
    quote = "spaces"

    [tasks.calc_base]
    quote = "never"

    [tasks.hello]
    bases = ["hello_base", "calc_base"]
    script = "echo hello_1"

    [tasks.hello_2]
    bases = ["calc_base", "hello_base"]
    script = "echo hello_2"
    "#
            .as_bytes(),
        )
        .unwrap();

        let config_file = ConfigFile::load(config_file_path).unwrap();

        let task = config_file.get_task("hello").unwrap();
        let task_ref = task.as_ref();
        assert_eq!(task_ref.quote.as_ref().unwrap(), &EscapeMode::Spaces);

        let task = config_file.get_task("hello_2").unwrap();
        let task_ref = task.as_ref();
        assert_eq!(task_ref.quote.as_ref().unwrap(), &EscapeMode::Never);
    }

    #[test]
    fn test_args_inheritance() {
        let tmp_dir = TempDir::new().unwrap();
        let config_file_path = tmp_dir.join("project.yamis.toml");
        let mut file = File::create(&config_file_path).unwrap();
        file.write_all(
            r#"
    [tasks.bash]
    program = "bash"

    [tasks.bash_inline]
    bases = ["bash"]
    args_extend = ["-c"]

    [tasks.hello]
    bases = ["bash_inline"]
    args_extend = ["echo", "hello"]

    [tasks.hello_2]
    bases = ["hello"]
    args = ["-c", "echo", "hello"]
    "#
            .as_bytes(),
        )
        .unwrap();

        let config_file = ConfigFile::load(config_file_path).unwrap();

        let task = config_file.get_task("hello").unwrap();
        let task_ref = task.as_ref();
        assert_eq!(
            task_ref.args.as_ref().unwrap(),
            &vec!["-c".to_string(), "echo".to_string(), "hello".to_string()]
        );

        let task = config_file.get_task("hello_2").unwrap();
        let task_ref = task.as_ref();
        assert_eq!(
            task_ref.args.as_ref().unwrap(),
            &vec!["-c".to_string(), "echo".to_string(), "hello".to_string()]
        );
    }

    #[test]
    fn test_get_task_help() {
        let tmp_dir = TempDir::new().unwrap();
        let config_file_path = tmp_dir.join("project.yamis.toml");
        let mut file = File::create(&config_file_path).unwrap();
        file.write_all(
            r#"
[tasks.base]
help = """
New lines should be trimmed
"""
program = "bash"

[tasks.help_inherited]
bases = ["base"]

[tasks.no_help]
program = "bash"

[tasks.help_removed]
bases = ["base"]
help = ""

[tasks.help_overriden]
bases = ["base"]
help = """
First line
Second line
"""
    "#
            .as_bytes(),
        )
        .unwrap();

        let config_file = ConfigFile::load(config_file_path).unwrap();

        let task = config_file.get_task("base").unwrap();
        let task_ref = task.as_ref();
        assert_eq!(task_ref.get_help(), "New lines should be trimmed");

        let task = config_file.get_task("help_inherited").unwrap();
        let task_ref = task.as_ref();
        assert_eq!(task_ref.get_help(), "New lines should be trimmed");

        let task = config_file.get_task("no_help").unwrap();
        let task_ref = task.as_ref();
        assert_eq!(task_ref.get_help(), "");

        let task = config_file.get_task("help_removed").unwrap();
        let task_ref = task.as_ref();
        assert_eq!(task_ref.get_help(), "");

        let task = config_file.get_task("help_overriden").unwrap();
        let task_ref = task.as_ref();
        assert_eq!(task_ref.get_help(), "First line\nSecond line");
    }

    #[test]
    fn test_read_env() {
        let tmp_dir = TempDir::new().unwrap();
        let project_config_path = tmp_dir.join("project.yamis.toml");
        let mut project_config_file = File::create(project_config_path.as_path()).unwrap();
        project_config_file
            .write_all(
                r#"
            env_file = ".env"

            [tasks.test.windows]
            quote = "never"
            script = "echo %VAR1% %VAR2% %VAR3%"

            [tasks.test]
            quote = "never"
            script = "echo $VAR1 $VAR2 $VAR3"

            [tasks.test_2.windows]
            quote = "never"
            script = "echo %VAR1% %VAR2% %VAR3%"
            env_file = ".env_2"
            env = {"VAR1" = "TASK_VAL1"}

            [tasks.test_2]
            quote = "never"
            script = "echo $VAR1 $VAR2 $VAR3"
            env_file = ".env_2"

            [tasks.test_2.env]
            VAR1 = "TASK_VAL1"
            "#
                .as_bytes(),
            )
            .unwrap();

        let mut env_file = File::create(tmp_dir.join(".env").as_path()).unwrap();
        env_file
            .write_all(
                r#"
    VAR1=VAL1
    VAR2=VAL2
    VAR3=VAL3
    "#
                .as_bytes(),
            )
            .unwrap();

        let mut env_file_2 = File::create(tmp_dir.join(".env_2").as_path()).unwrap();
        env_file_2
            .write_all(
                r#"
    VAR1=OTHER_VAL1
    VAR2=OTHER_VAL2
    "#
                .as_bytes(),
            )
            .unwrap();

        let config_file = ConfigFile::load(project_config_path).unwrap();

        let task = config_file.get_task("test").unwrap();
        let env = task.get_env(&config_file);

        let expected = HashMap::from([
            ("VAR1".to_string(), "VAL1".to_string()),
            ("VAR2".to_string(), "VAL2".to_string()),
            ("VAR3".to_string(), "VAL3".to_string()),
        ]);
        assert_eq!(env, expected);

        let task = config_file.get_task("test_2").unwrap();
        let env = task.get_env(&config_file);
        let expected = HashMap::from([
            ("VAR1".to_string(), "TASK_VAL1".to_string()),
            ("VAR2".to_string(), "OTHER_VAL2".to_string()),
            ("VAR3".to_string(), "VAL3".to_string()),
        ]);
        assert_eq!(env, expected);
    }

    #[test]
    fn test_validate() {
        let task = get_task(
            "sample",
            r#"
        script = "hello world"
        program = "some_program"
    "#,
            None,
        );
        let expected_error = TaskError::ImproperlyConfigured(
            String::from("sample"),
            String::from("Cannot specify `script` and `program` at the same time."),
        );
        assert_eq!(task.unwrap_err().to_string(), expected_error.to_string());

        let task = get_task(
            "sample",
            r#"
        script_runner = ""
    "#,
            None,
        );
        let expected_error = TaskError::ImproperlyConfigured(
            String::from("sample"),
            String::from("`script_runner` parameter cannot be an empty string."),
        );
        assert_eq!(task.unwrap_err().to_string(), expected_error.to_string());

        let task = get_task(
            "sample",
            r#"
        script = "echo hello"
        serial = ["sample"]
    "#,
            None,
        );

        let expected_error = TaskError::ImproperlyConfigured(
            String::from("sample"),
            String::from("Cannot specify `script` and `serial` at the same time."),
        );
        assert_eq!(task.unwrap_err().to_string(), expected_error.to_string());

        let task = get_task(
            "sample",
            r#"
        program = "python"
        serial = ["sample"]
    "#,
            None,
        );

        let expected_error = TaskError::ImproperlyConfigured(
            String::from("sample"),
            String::from("Cannot specify `program` and `serial` at the same time."),
        );
        assert_eq!(task.unwrap_err().to_string(), expected_error.to_string());

        let task = get_task(
            "sample",
            r#"
        quote = "spaces"
        program = "python"
    "#,
            None,
        );

        let expected_error = TaskError::ImproperlyConfigured(
            String::from("sample"),
            String::from("`quote` parameter can only be set for scripts."),
        );
        assert_eq!(task.unwrap_err().to_string(), expected_error.to_string());

        let task = get_task(
            "sample",
            r#"
        script = "sample script"
        args = ["some", "args"]
    "#,
            None,
        );

        let expected_error = TaskError::ImproperlyConfigured(
            String::from("sample"),
            String::from("Cannot specify `args` on scripts."),
        );
        assert_eq!(task.unwrap_err().to_string(), expected_error.to_string());
    }

    #[test]
    fn test_create_temp_script() {
        let tmp_dir = TempDir::new().unwrap();
        let project_config_path = tmp_dir.join("project.yamis.toml");
        let script = "echo hello world";
        let extension = "sh";
        let task_name = "sample";
        let script_path =
            get_temp_script(script, extension, task_name, project_config_path.as_path()).unwrap();
        assert!(script_path.exists());
        assert_eq!(script_path.extension().unwrap(), extension);
        let script_content = fs::read_to_string(script_path).unwrap();
        assert_eq!(script_content, script);

        let extension = "";
        let task_name = "sample2";
        let script_path =
            get_temp_script(script, extension, task_name, project_config_path.as_path()).unwrap();
        assert!(script_path.exists());
        assert!(script_path.extension().is_none());
        let script_content = fs::read_to_string(script_path).unwrap();
        assert_eq!(script_content, script);

        let extension = ".sh";
        let task_name = "sample3";
        let script_path =
            get_temp_script(script, extension, task_name, project_config_path.as_path()).unwrap();
        assert!(script_path.exists());
        assert_eq!(script_path.extension().unwrap(), "sh");
        let script_content = fs::read_to_string(script_path).unwrap();
        assert_eq!(script_content, script);
    }
}
