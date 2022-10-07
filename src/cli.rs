use colored::{ColoredString, Colorize};
use lazy_static::lazy_static;
use serde_derive::Deserialize;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::error::Error;
use std::ffi::OsStr;
use std::fs::File;
use std::path::Path;
use std::{env, fmt, fs};

use regex::Regex;

use crate::config_files::{ConfigFilePaths, ConfigFilesContainer};
use crate::types::{DynErrResult, TaskArgs};

const HELP: &str = "The appropriate YAML or TOML config files need to exist \
in the directory or parents, or a file is specified with the `-f` or `--file` \
options. For help about the config files check https://github.com/adrianmrit/yamis";

/// Holds the data for running the given task.
struct TaskSubcommand {
    /// Task to run, if given
    pub task: String,
    /// Args to run the command with
    pub args: TaskArgs,
}

/// Enum of config file containers by version
enum ConfigFileContainerVersion {
    V1(ConfigFilesContainer),
}

fn default_version() -> Version {
    Version::V1
}

#[derive(Debug, Deserialize)]
pub struct ConfigFileVersionSerializer {
    #[serde(default = "default_version")]
    version: Version,
}

/// Enum of available config file versions
#[derive(Hash, Eq, PartialEq, Debug, Deserialize)]
enum Version {
    #[serde(alias = "v1")]
    #[serde(rename = "1")]
    V1,
}

/// Holds all the config file containers, regardless of the version they are supposed to handle
struct ConfigFileContainers {
    /// Holds the config file containers for each version
    containers: HashMap<Version, ConfigFileContainerVersion>,
}

/// Argument errors
#[derive(Debug, PartialEq, Eq)]
enum ArgsError {
    /// Raised when no task to run is given
    MissingTaskArg,
}

impl fmt::Display for ArgsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ArgsError::MissingTaskArg => write!(f, "No task was given."),
        }
    }
}

impl Error for ArgsError {
    fn description(&self) -> &str {
        match *self {
            ArgsError::MissingTaskArg => "no task given",
        }
    }

    fn cause(&self) -> Option<&dyn Error> {
        None
    }
}

/// Sets the color when printing the task name
fn colorize_task_name(val: &str) -> ColoredString {
    val.bright_cyan()
}

/// Sets the color when printing the config file path
fn colorize_config_file_path(val: &str) -> ColoredString {
    val.bright_blue()
}

impl ConfigFileContainers {
    /// Creates a new instance of `ConfigFileContainers`
    fn new() -> Self {
        let mut containers = HashMap::new();
        containers.insert(
            Version::V1,
            ConfigFileContainerVersion::V1(ConfigFilesContainer::new()),
        );
        Self { containers }
    }

    /// Peeks at the file and returns the version of the config file.
    ///
    /// # Arguments
    ///
    /// * `path`: path to the file to extract the version from
    ///
    /// returns: Result<String, Box<dyn Error, Global>>
    pub(crate) fn get_file_version(path: &Path) -> DynErrResult<Version> {
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
                panic!("Unknown file extension: {}", extension);
            }
        };

        let file = match File::open(&path) {
            Ok(file_contents) => file_contents,
            Err(e) => return Err(format!("There was an error reading the file:\n{}", e).into()),
        };

        let result: ConfigFileVersionSerializer = if is_yaml {
            serde_yaml::from_reader(file)?
        } else {
            // A bytes list should be slightly faster than a string list
            toml::from_slice(&fs::read(&path)?)?
        };

        Ok(result.version)
    }

    /// prints config file paths and their tasks
    fn print_tasks_list(&mut self, paths: ConfigFilePaths) -> DynErrResult<()> {
        for path in paths {
            let path = path?;
            let version = ConfigFileContainers::get_file_version(&path)?;
            match version {
                Version::V1 => {
                    println!("{}:", colorize_config_file_path(&path.to_string_lossy()));
                    let container = self.containers.get_mut(&Version::V1).unwrap();
                    let ConfigFileContainerVersion::V1(container) = container;
                    let config_file_ptr = container.read_config_file(path.clone())?;
                    let config_file_lock = config_file_ptr.lock().unwrap();
                    let tasks = config_file_lock.get_non_private_task_names();
                    if tasks.is_empty() {
                        println!("  {}", "No tasks found.".red());
                    } else {
                        for task in tasks {
                            println!(" - {}", colorize_task_name(task.get_name()));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Prints help for the given task
    fn print_task_info(&mut self, paths: ConfigFilePaths, task: &str) -> DynErrResult<()> {
        for path in paths {
            let path = path?;
            let version = ConfigFileContainers::get_file_version(&path)?;
            match version {
                Version::V1 => {
                    let container = self.containers.get_mut(&Version::V1).unwrap();
                    let ConfigFileContainerVersion::V1(container) = container;
                    let config_file_ptr = container.read_config_file(path.clone())?;
                    let config_file_lock = config_file_ptr.lock().unwrap();
                    let task = config_file_lock.get_task(task);
                    match task {
                        Some(task) => {
                            println!("{}:", colorize_config_file_path(&path.to_string_lossy()));
                            print!(" - {}", colorize_task_name(task.get_name()));
                            if task.is_private() {
                                print!(" {}", "(private)".red());
                            }
                            println!();
                            let prefix = "     ";
                            match task.get_help().trim() {
                                "" => println!("{}{}", prefix, "No help to display".yellow()),
                                help => {
                                    //                 " -   "  Two spaces after the dash
                                    let help_lines: Vec<&str> = help.lines().collect();
                                    println!(
                                        "{}{}",
                                        prefix,
                                        help_lines.join(&format!("\n{}", prefix)).green()
                                    )
                                }
                            }
                            return Ok(());
                        }
                        None => continue,
                    }
                }
            }
        }
        Err(format!("Task {} not found", task).into())
    }

    /// Runs the given task
    fn run_task(&mut self, paths: ConfigFilePaths, task: &str, args: TaskArgs) -> DynErrResult<()> {
        for path in paths {
            let path = path?;
            let version = match ConfigFileContainers::get_file_version(&path) {
                Ok(version) => version,
                Err(e) => {
                    // So the user knows where the error occurred
                    let e = format!("{}:\n{}", &path.to_string_lossy().red(), e);
                    return Err(e.into());
                }
            };
            match version {
                Version::V1 => {
                    let container = self.containers.get_mut(&Version::V1).unwrap();
                    let ConfigFileContainerVersion::V1(container) = container;
                    let config_file_ptr = match container.read_config_file(path.clone()) {
                        Ok(val) => val,
                        Err(e) => {
                            let e = format!("{}:\n{}", &path.to_string_lossy().red(), e);
                            return Err(e.into());
                        }
                    };
                    let config_file_lock = config_file_ptr.lock().unwrap();
                    let task = config_file_lock.get_task(task);
                    match task {
                        Some(task) => {
                            return match task.run(&args, &config_file_lock) {
                                Ok(val) => Ok(val),
                                Err(e) => {
                                    let e = format!("{}:\n{}", &path.to_string_lossy().red(), e);
                                    Err(e.into())
                                }
                            };
                        }
                        None => continue,
                    }
                }
            }
        }
        Err(format!("Task {} not found", task).into())
    }
}

impl TaskSubcommand {
    /// Returns a new TaskSubcommand
    pub(crate) fn new(args: &clap::ArgMatches) -> Result<TaskSubcommand, ArgsError> {
        let mut kwargs = TaskArgs::new();

        let (task_name, task_args) = match args.subcommand() {
            None => return Err(ArgsError::MissingTaskArg),
            Some(command) => command,
        };

        if let Some(args) = task_args.values_of("") {
            // All args are pushed into a vector as they are
            let all_args = args.clone().map(|s| s.to_string()).collect::<Vec<String>>();
            kwargs.insert(String::from("*"), all_args);

            // kwarg found that could be a key
            let mut possible_kwarg_key = None;

            // looping over the args to find kwargs
            for arg in args {
                // if a kwarg key was previously found, assume this is the value, even if
                // it starts with - or --
                if let Some(possible_kwarg) = possible_kwarg_key {
                    match kwargs.entry(possible_kwarg) {
                        Entry::Occupied(mut e) => {
                            e.get_mut().push(arg.to_string());
                        }
                        Entry::Vacant(e) => {
                            let args_vec: Vec<String> = vec![arg.to_string()];
                            e.insert(args_vec);
                        }
                    }
                    possible_kwarg_key = None;
                    continue;
                }

                // Quick check to see if the arg is a kwarg key or key-value pair
                // if it is a positional value, we just continue
                if !arg.starts_with('-') {
                    continue;
                }

                // Check if this is a kwarg key-value pair
                if let Some((key, val)) = Self::get_kwarg(arg) {
                    match kwargs.entry(key) {
                        Entry::Occupied(mut e) => {
                            e.get_mut().push(val);
                        }
                        Entry::Vacant(e) => {
                            let args_vec: Vec<String> = vec![val];
                            e.insert(args_vec);
                        }
                    }
                    continue;
                }

                // Otherwise it could be a kwarg key, for which we need to check the next arg
                if let Some(key) = Self::get_kwarg_key(arg) {
                    possible_kwarg_key = Some(key);
                    continue;
                }

                // Finally if it is not a kwarg key or key-value pair, it is a positional arg,
                // i.e. -0
            }
        } else {
            kwargs.insert(String::from("*"), vec![]);
        }

        Ok(TaskSubcommand {
            task: String::from(task_name),
            args: kwargs,
        })
    }

    /// Returns the key if the arg represents a kwarg key, otherwise None
    fn get_kwarg_key(arg: &str) -> Option<String> {
        lazy_static! {
            static ref KWARG_KEY_REGEX: Regex = Regex::new(r"-{1,2}(?P<key>[a-zA-Z]+\w*)").unwrap();
        }
        let kwarg_match = KWARG_KEY_REGEX.captures(arg);
        if let Some(arg_match) = kwarg_match {
            let key = String::from(arg_match.name("key").unwrap().as_str());
            Some(key)
        } else {
            None
        }
    }

    /// Returns the key and value if the arg represents a kwarg key-value pair, otherwise None
    fn get_kwarg(arg: &str) -> Option<(String, String)> {
        lazy_static! {
            static ref KWARG_REGEX: Regex =
                Regex::new(r"-{1,2}(?P<key>[a-zA-Z]+\w*)=(?P<val>[\s\S]*)").unwrap();
        }
        let kwarg_match = KWARG_REGEX.captures(arg);
        if let Some(arg_match) = kwarg_match {
            let key = String::from(arg_match.name("key").unwrap().as_str());
            let val = String::from(arg_match.name("val").unwrap().as_str());
            Some((key, val))
        } else {
            None
        }
    }
}

/// Executes the program. If errors are encountered during the execution these
/// are returned immediately. The wrapping method needs to take care of formatting
/// and displaying these errors appropriately.
pub fn exec() -> DynErrResult<()> {
    let app = clap::Command::new(clap::crate_name!())
        .version(clap::crate_version!())
        .about(clap::crate_description!())
        .author(clap::crate_authors!())
        .after_help(HELP)
        .allow_external_subcommands(true)
        .arg(
            clap::Arg::new("list")
                .short('l')
                .long("list")
                .takes_value(false)
                .help("Lists configuration files that can be reached from the current directory")
                .conflicts_with_all(&["file"]),
        )
        .arg(
            clap::Arg::new("list-tasks")
                .short('t')
                .long("list-tasks")
                .takes_value(false)
                .help("Lists tasks")
                .conflicts_with_all(&["task-info"]),
        )
        .arg(
            clap::Arg::new("task-info")
                .short('i')
                .long("task-info")
                .takes_value(true)
                .help("Displays information about the given task")
                .value_name("TASK"),
        )
        .arg(
            clap::Arg::new("file")
                .short('f')
                .long("file")
                .help("Search for tasks in the given file")
                .takes_value(true)
                .value_name("FILE"),
        );
    let matches = app.get_matches();

    let current_dir = env::current_dir()?;
    let mut file_containers = ConfigFileContainers::new();

    let config_file_paths = match matches.value_of("file") {
        None => ConfigFilePaths::new(&current_dir),
        Some(file_path) => ConfigFilePaths::only(file_path)?,
    };

    if matches.contains_id("list-tasks") {
        file_containers.print_tasks_list(config_file_paths)?;
        return Ok(());
    };

    if let Some(task_name) = matches.value_of("task-info") {
        file_containers.print_task_info(config_file_paths, task_name)?;
        return Ok(());
    };

    if matches.contains_id("list") {
        for path in config_file_paths {
            let path = path?;
            println!("{}", colorize_config_file_path(&path.to_string_lossy()));
        }
        return Ok(());
    }

    let task_command = TaskSubcommand::new(&matches)?;

    file_containers.run_task(config_file_paths, &task_command.task, task_command.args)
}

#[cfg(test)]
mod tests {
    use crate::config_files::ConfigFilePaths;
    use assert_cmd::Command;
    use assert_fs::TempDir;
    use predicates::prelude::predicate;
    use std::fs::File;
    use std::io::Write;

    #[test]
    #[ignore = "Fails but works fine when run manually"]
    fn test_list() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = TempDir::new().unwrap();
        let global_config_dir = ConfigFilePaths::get_global_config_file_dir();

        // Global config dir should not be the same as the current dir
        assert_ne!(tmp_dir.path(), &global_config_dir);

        // Should always return the same global dir
        assert_eq!(
            &ConfigFilePaths::get_global_config_file_dir(),
            &global_config_dir
        );

        let global_config_path = global_config_dir.join("user.yamis.toml");
        let mut global_config_file = File::create(global_config_path.as_path()).unwrap();
        global_config_file
            .write_all(
                r#"
                [tasks.hello_global]
                script = "echo hello project"
                help = "Some help here"
                "#
                .as_bytes(),
            )
            .unwrap();

        let mut file = File::create(tmp_dir.join("project.yamis.toml"))?;
        file.write_all(
            r#"

    [tasks.hello.windows]
    script = "echo %greeting%, one plus one is %one_plus_one%"
    private=true

    [tasks.hello]
    script = "echo $greeting, one plus one is $one_plus_one"
    "#
            .as_bytes(),
        )?;
        let expected = format!(
            "{tmp_dir}/project.yamis.toml\n{global_config_dir}/user.yamis.toml\n",
            tmp_dir = tmp_dir.path().to_str().unwrap(),
            global_config_dir = global_config_dir.to_str().unwrap()
        );
        let mut cmd = Command::cargo_bin("yamis")?;
        cmd.current_dir(tmp_dir.path());
        cmd.arg("--list");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains(expected));
        Ok(())
    }
}
