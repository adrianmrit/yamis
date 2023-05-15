use std::collections::HashMap;
use std::env::temp_dir;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{error, fmt, fs, mem};

use crate::args::ArgsContext;
use crate::config_files::ConfigFile;
use crate::defaults::default_false;
use crate::print_utils::{YamisOutput, INFO_COLOR};
use colored::Colorize;
use serde::{de, Deserialize, Serialize};
use tera::{Context, Tera};

use crate::types::DynErrResult;
use crate::utils::{get_path_relative_to_base, read_env_file, split_command, TMP_FOLDER_NAMESPACE};
use md5::{Digest, Md5};

cfg_if::cfg_if! {
    if #[cfg(target_os = "windows")] {
        // Will run the actual script in CMD, but we don't need to specify /C option
        const DEFAULT_SCRIPT_RUNNER: &str = "powershell {{ script_path }}";
        const DEFAULT_SCRIPT_EXTENSION: &str = "cmd";
    } else if #[cfg(target_os = "linux")] {
        const DEFAULT_SCRIPT_RUNNER: &str = "bash {{ script_path }}";
        const DEFAULT_SCRIPT_EXTENSION: &str = "sh";
    } else if #[cfg(target_os = "macos")] {
        const DEFAULT_SCRIPT_RUNNER: &str = "bash {{ script_path }}";
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

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct TaskNameOption {
    task: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CmdOption {
    #[serde(flatten)]
    command: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(untagged)]
pub enum Cmd {
    #[serde(rename = "task_name")]
    TaskName(String),
    #[serde(rename = "task")]
    Task(Box<Task>),
    #[serde(rename = "cmd")]
    Cmd(String),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum StringOrTask {
    String(String),
    Task(Box<Task>),
}

impl<'de> de::Deserialize<'de> for Cmd {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct CmdVisitor;

        impl<'de> de::Visitor<'de> for CmdVisitor {
            type Value = Cmd;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("cmd, task name or task")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Cmd::Cmd(value.to_string()))
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                match map.next_key::<String>()? {
                    Some(key) => match key.as_str() {
                        "task" => {
                            let string_or_task: StringOrTask = map.next_value()?;
                            match string_or_task {
                                StringOrTask::String(s) => Ok(Cmd::TaskName(s)),
                                StringOrTask::Task(t) => Ok(Cmd::Task(t)),
                            }
                        }
                        "cmd" => {
                            let cmd: String = map.next_value()?;
                            Ok(Cmd::Cmd(cmd))
                        }
                        _ => Err(de::Error::unknown_field(
                            key.as_str(),
                            &["task_name", "task", "cmd"],
                        )),
                    },
                    None => Err(de::Error::missing_field("task_name or task")),
                }

                // Deserialize::deserialize(de::value::MapAccessDeserializer::new(map))
            }
        }

        deserializer.deserialize_any(CmdVisitor {})
    }
}

/// Represents a Task
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Task {
    /// Name of the task
    #[serde(skip_deserializing)]
    name: String,
    /// Help of the task
    help: Option<String>,
    /// Script to run
    script: Option<String>,
    /// Interpreter program to use
    script_runner: Option<String>,
    /// Script extension
    #[serde(alias = "script_ext")]
    script_extension: Option<String>,
    /// A program to run
    program: Option<String>,
    /// Args to pass to a command
    args: Option<String>,
    /// Run commands
    cmds: Option<Vec<Cmd>>,
    /// Extends args from bases
    #[serde(alias = "args+")]
    args_extend: Option<String>,
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
        inherit_value!(self.help, base_task.help);
        inherit_value!(self.script, base_task.script);
        inherit_value!(self.script_runner, base_task.script_runner);
        inherit_value!(self.script_extension, base_task.script_extension);
        inherit_value!(self.program, base_task.program);
        inherit_value!(self.args, base_task.args);
        inherit_value!(self.cmds, base_task.cmds);
        inherit_value!(self.env_file, base_task.env_file);

        // We merge the envs, so the base env is not overwritten
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
                self.args = mem::replace(&mut self.args, Some(String::new()));
            }
            if let Some(args) = &mut self.args {
                args.push(' ');
                args.push_str(&new_args);
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
    fn get_env(&self, env: &HashMap<String, String>) -> HashMap<String, String> {
        let mut new_env = self.env.clone();
        for (key, val) in env {
            new_env.entry(key.clone()).or_insert_with(|| val.clone());
        }
        new_env
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

        if self.script.is_some() && self.args.is_some() {
            return Err(TaskError::ImproperlyConfigured(
                self.name.clone(),
                String::from("Cannot specify `args` on scripts."),
            ));
        }

        Ok(())
    }

    // Returns the Tera instance for the Tera template engine.
    fn get_tera_instance(&self) -> Tera {
        Tera::default()
    }

    /// Returns the context for the Tera template engine.
    fn get_tera_context(
        &self,
        args: &ArgsContext,
        config_file: &ConfigFile,
        env: &HashMap<String, String>,
    ) -> Context {
        let mut context = Context::new();

        context.insert("args", &args.args);
        context.insert("kwargs", &args.kwargs);
        context.insert("pkwargs", &args.pkwargs);
        context.insert("env", &env);
        context.insert("TASK", self);
        context.insert("FILE", config_file);

        context
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
        env: &HashMap<String, String>,
    ) -> DynErrResult<()> {
        command.envs(env);
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
    fn spawn_command(&self, command: &mut Command, dry_run: bool) -> DynErrResult<()> {
        if dry_run {
            println!("{}", "Dry run mode, nothing executed.".yamis_info());
            return Ok(());
        }
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

    /// Runs a program
    fn run_program(
        &self,
        args: &ArgsContext,
        config_file: &ConfigFile,
        env: &HashMap<String, String>,
        dry_mode: bool,
    ) -> DynErrResult<()> {
        let program = self.program.as_ref().unwrap();
        let mut command = Command::new(program);
        self.set_command_basics(&mut command, config_file, env)?;

        let mut tera = self.get_tera_instance();
        let context = self.get_tera_context(args, config_file, env);

        if let Some(task_args) = &self.args {
            let task_name = &self.name;
            let template_name = format!("tasks.{task_name}.args");

            tera.add_raw_template(&template_name, task_args)?;

            let rendered_args = tera.render(&template_name, &context)?;
            let rendered_args_list = split_command(&rendered_args);
            dbg!(&rendered_args_list);
            println!(
                "{}",
                format!("{}: {} {}", self.name, program, rendered_args).yamis_info()
            );
            command.args(rendered_args_list);
        } else {
            println!("{}", format!("{}: {}", self.name, program).yamis_info());
        }

        self.spawn_command(&mut command, dry_mode)
    }

    fn run_cmds_cmd(
        &self,
        cmd: &str,
        cmd_index: usize,
        args: &ArgsContext,
        config_file: &ConfigFile,
        env: &HashMap<String, String>,
        dry_run: bool,
    ) -> DynErrResult<()> {
        let mut tera = Tera::default();
        let context = self.get_tera_context(args, config_file, env);

        let task_name = &self.name;
        let task_name = &format!("{task_name}.cmds.{cmd_index}");
        let template_name = &format!("tasks.{task_name}");
        tera.add_raw_template(template_name, cmd)?;

        let cmd = tera.render(template_name, &context)?;
        let cmd_args = split_command(&cmd);
        let program = &cmd_args[0];
        let cmd_args = &cmd_args[1..];
        let mut command: Command = Command::new(program);
        self.set_command_basics(&mut command, config_file, env)?;
        command.args(cmd_args.iter());

        println!("{}", format!("{task_name}: {cmd}").yamis_info());
        self.spawn_command(&mut command, dry_run)
    }

    fn run_cmds_task_name(
        &self,
        task_name: &str,
        cmd_index: usize,
        args: &ArgsContext,
        config_file: &ConfigFile,
        dry_run: bool,
    ) -> DynErrResult<()> {
        let display_task_name = format!("{}.cmds.{}.{}", self.name, cmd_index, task_name);
        if let Some(mut task) = config_file.get_task(task_name) {
            task.name = display_task_name;
            task.run(args, config_file, dry_run)
        } else {
            Err(TaskError::RuntimeError(
                self.name.clone(),
                format!("Task `{}` not found.", task_name),
            )
            .into())
        }
    }

    fn run_cmds_task(
        &self,
        task: &Task,
        cmd_index: usize,
        args: &ArgsContext,
        config_file: &ConfigFile,
        dry_run: bool,
    ) -> DynErrResult<()> {
        let mut task = task.clone();
        let task_name = format!("{}.cmds.{}", self.name, cmd_index);
        task.setup(&task_name, config_file.directory())?;
        let base_task_names = task.bases.clone();
        for base_name in base_task_names.iter() {
            // Because the bases have been loaded already, there cannot be any circular dependencies
            // Todo, get reference to base task instead of cloning it
            let base_task = config_file.get_task(base_name);
            match base_task {
                Some(base_task) => task.extend_task(&base_task),
                None => {
                    return Err(TaskError::RuntimeError(
                        self.name.clone(),
                        format!("Task `{}` not found.", base_name),
                    )
                    .into())
                }
            }
        }
        let new_env = task.get_env(&self.env);
        task.env = new_env;
        task.run(args, config_file, dry_run)
    }

    /// Runs the commands specified with the cmds option.
    fn run_cmds(
        &self,
        args: &ArgsContext,
        config_file: &ConfigFile,
        env: &HashMap<String, String>,
        dry_run: bool,
    ) -> DynErrResult<()> {
        for (i, cmd) in self.cmds.as_ref().unwrap().iter().enumerate() {
            match cmd {
                Cmd::Cmd(cmd) => {
                    self.run_cmds_cmd(cmd, i, args, config_file, env, dry_run)?;
                }
                Cmd::TaskName(task_name) => {
                    self.run_cmds_task_name(task_name, i, args, config_file, dry_run)?;
                }
                Cmd::Task(task) => {
                    self.run_cmds_task(task, i, args, config_file, dry_run)?;
                }
            }
        }
        Ok(())
    }

    /// Runs a script
    fn run_script(
        &self,
        args: &ArgsContext,
        config_file: &ConfigFile,
        env: &HashMap<String, String>,
        dry_run: bool,
    ) -> DynErrResult<()> {
        let script = self.script.as_ref().unwrap();

        let mut tera = Tera::default();
        let mut context = self.get_tera_context(args, config_file, env);
        let task_name = &self.name;
        let template_name = format!("tasks.{task_name}.script");
        tera.add_raw_template(&template_name, script)?;
        let script = tera.render(&template_name, &context)?;
        let default_script_extension = String::from(DEFAULT_SCRIPT_EXTENSION);
        let script_extension = self
            .script_extension
            .as_ref()
            .unwrap_or(&default_script_extension);

        let script_path = get_temp_script(
            &script,
            script_extension,
            &self.name,
            config_file.filepath.as_path(),
        )?;

        cfg_if::cfg_if! {
            if #[cfg(target_os = "windows")]
            {
                let script_path = script_path.to_str().unwrap();
                let script_path = script_path.replace('\\', "\\\\");
                context.insert("script_path", &script_path);
            } else {
                context.insert("script_path", &script_path);
            }
        }

        // Interpreter is a list, because sometimes there is need to pass extra arguments to the
        // interpreter, such as the /C option in the batch case
        let script_runner = if let Some(script_runner) = &self.script_runner {
            script_runner
        } else {
            DEFAULT_SCRIPT_RUNNER
        };

        let script_runner_template_name = format!("tasks.{task_name}.script_runner");
        tera.add_raw_template(&script_runner_template_name, script_runner)?;

        let script_runner = tera.render(&script_runner_template_name, &context)?;
        let script_runner_values = split_command(&script_runner);

        let mut command = Command::new(&script_runner_values[0]);

        // The script runner might not contain the actual script path, but we just leave it as a feature ;)
        if script_runner_values.len() > 1 {
            command.args(script_runner_values[1..].iter());
        }

        self.set_command_basics(&mut command, config_file, env)?;

        println!("{}", format!("{task_name}: {script_runner}").yamis_info());
        println!("{}", "Script Begin:".yamis_info());
        println!("{}", script.color(INFO_COLOR));
        println!("{}", "Script End.".yamis_info());

        self.spawn_command(&mut command, dry_run)
    }

    /// Helper function for running a task. Accepts the environment variables as a HashMap.
    /// So that we can reuse the environment variables for multiple tasks.
    pub fn run(
        &self,
        args: &ArgsContext,
        config_file: &ConfigFile,
        dry_run: bool,
    ) -> DynErrResult<()> {
        let env = match config_file.env.as_ref() {
            Some(env) => self.get_env(env),
            None => self.env.clone(),
        };
        return if self.script.is_some() {
            self.run_script(args, config_file, &env, dry_run)
        } else if self.program.is_some() {
            self.run_program(args, config_file, &env, dry_run)
        } else if self.cmds.is_some() {
            self.run_cmds(args, config_file, &env, dry_run)
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
        let mut task: Task = serde_yaml::from_str(definition).unwrap();
        task.setup(name, base_path.unwrap_or_else(|| Path::new("")))?;
        Ok(task)
    }

    #[test]
    fn test_env_inheritance() {
        let tmp_dir = TempDir::new().unwrap();
        let config_file_path = tmp_dir.join("yamis.root.yml");
        let mut file = File::create(&config_file_path).unwrap();
        file.write_all(
            r#"
version: 2

tasks:
    hello_base:
        env:
            greeting: hello world

    calc_base:
        env:
            one_plus_one: 2

    hello:
        bases: ["hello_base", "calc_base"]
        script: "echo $greeting, 1+1=$one_plus_one"

    hello.windows:
        bases: ["hello_base", "calc_base"]
        script: "echo %greeting%, 1+1=%one_plus_one%"
    "#
            .as_bytes(),
        )
        .unwrap();

        let config_file = ConfigFile::load(config_file_path).unwrap();

        let task = config_file.get_task("hello").unwrap();

        let expected = HashMap::from([
            ("greeting".to_string(), "hello world".to_string()),
            ("one_plus_one".to_string(), "2".to_string()),
        ]);
        assert_eq!(task.env, expected);
    }

    #[test]
    fn test_args_inheritance() {
        let tmp_dir = TempDir::new().unwrap();
        let config_file_path = tmp_dir.join("yamis.root.yml");
        let mut file = File::create(&config_file_path).unwrap();
        file.write_all(
            r#"
    version: 2

    tasks:
        bash:
            program: "bash"

        bash_inline:
            bases: ["bash"]
            args_extend: "-c"

        hello:
            bases: ["bash_inline"]
            args_extend: echo hello

        hello_2:
            bases: ["hello"]
            args: -c "echo hello"
    "#
            .as_bytes(),
        )
        .unwrap();

        let config_file = ConfigFile::load(config_file_path).unwrap();

        let task = config_file.get_task("hello").unwrap();
        assert_eq!(task.args.as_ref().unwrap(), "-c echo hello");

        let task = config_file.get_task("hello_2").unwrap();
        assert_eq!(task.args.as_ref().unwrap(), &"-c \"echo hello\"");
    }

    #[test]
    fn test_get_task_help() {
        let tmp_dir = TempDir::new().unwrap();
        let config_file_path = tmp_dir.join("yamis.root.yml");
        let mut file = File::create(&config_file_path).unwrap();
        file.write_all(
            r#"
version: 2

tasks:
    base:
        help: >
            New lines
            should be
            trimmed

        program: "bash"

    help_inherited:
        bases: ["base"]

    no_help:
        program: "bash"

    help_removed:
        bases: ["base"]
        help: ""

    help_overriden:
        bases: ["base"]
        help: |
            First line
            Second line
    "#
            .as_bytes(),
        )
        .unwrap();

        let config_file = ConfigFile::load(config_file_path).unwrap();

        let task = config_file.get_task("base").unwrap();
        assert_eq!(task.get_help(), "New lines should be trimmed");

        let task = config_file.get_task("help_inherited").unwrap();
        assert_eq!(task.get_help(), "New lines should be trimmed");

        let task = config_file.get_task("no_help").unwrap();
        assert_eq!(task.get_help(), "");

        let task = config_file.get_task("help_removed").unwrap();
        assert_eq!(task.get_help(), "");

        let task = config_file.get_task("help_overriden").unwrap();
        assert_eq!(task.get_help(), "First line\nSecond line");
    }

    #[test]
    fn test_read_env() {
        let tmp_dir = TempDir::new().unwrap();
        let project_config_path = tmp_dir.join("yamis.root.yml");
        let mut project_config_file = File::create(project_config_path.as_path()).unwrap();
        project_config_file
            .write_all(
                r#"
env_file: ".env"

version: 2

tasks:
    test.windows:
        script: "echo %VAR1% %VAR2% %VAR3%"

    test:
        script: "echo $VAR1 $VAR2 $VAR3"

    test_2.windows:
        script: "echo %VAR1% %VAR2% %VAR3%"
        env_file: ".env_2"
        env: 
            VAR1: TASK_VAL1

    test_2:
        script: "echo $VAR1 $VAR2 $VAR3"
        env_file: ".env_2"
        env:
            VAR1: "TASK_VAL1"
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
        let env = task.get_env(config_file.env.as_ref().unwrap());

        let expected = HashMap::from([
            ("VAR1".to_string(), "VAL1".to_string()),
            ("VAR2".to_string(), "VAL2".to_string()),
            ("VAR3".to_string(), "VAL3".to_string()),
        ]);
        assert_eq!(env, expected);

        let task = config_file.get_task("test_2").unwrap();
        let env = task.get_env(config_file.env.as_ref().unwrap());
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
        script: "hello world"
        program: "some_program"
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
        script_runner: ""
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
        script: "sample script"
        args: "some args"
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
        let project_config_path = tmp_dir.join("yamis.root.yml");
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
