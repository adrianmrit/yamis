use clap::ArgAction;
use colored::{ColoredString, Colorize};
use lazy_static::lazy_static;
use serde_derive::Deserialize;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::path::Path;
use std::{env, fmt, fs};

use regex::Regex;

use crate::config_files::{
    ConfigFilePaths, ConfigFilesContainer, GlobalConfigFilePath, PathIterator, SingleConfigFilePath,
};
use crate::print_utils::YamisOutput;
use crate::types::{DynErrResult, TaskArgs};
use crate::updater;

const HELP: &str = "For documentation check https://github.com/adrianmrit/yamis.";

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

        let file = match File::open(path) {
            Ok(file_contents) => file_contents,
            Err(e) => return Err(format!("There was an error reading the file:\n{}", e).into()),
        };

        let result: ConfigFileVersionSerializer = if is_yaml {
            serde_yaml::from_reader(file)?
        } else {
            toml::from_str(&fs::read_to_string(path)?)?
        };

        Ok(result.version)
    }

    /// prints config file paths and their tasks
    fn print_tasks_list(&mut self, paths: PathIterator) -> DynErrResult<()> {
        let mut found = false;
        for path in paths {
            found = true;
            let version = ConfigFileContainers::get_file_version(&path)?;
            match version {
                Version::V1 => {
                    println!("{}:", colorize_config_file_path(&path.to_string_lossy()));
                    let container = self.containers.get_mut(&Version::V1).unwrap();
                    let ConfigFileContainerVersion::V1(container) = container;
                    let config_file_ptr = container.read_config_file(path.clone())?;
                    let config_file_lock = config_file_ptr.lock().unwrap();
                    let task_names = config_file_lock.get_public_task_names();
                    if task_names.is_empty() {
                        println!("  {}", "No tasks found.".red());
                    } else {
                        for task in task_names {
                            println!(" - {}", colorize_task_name(task));
                        }
                    }
                }
            }
        }
        if !found {
            println!("No config files found.");
        }
        Ok(())
    }

    /// Prints help for the given task
    fn print_task_info(&mut self, paths: PathIterator, task: &str) -> DynErrResult<()> {
        for path in paths {
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
    fn run_task(&mut self, paths: PathIterator, task: &str, args: TaskArgs) -> DynErrResult<()> {
        for path in paths {
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
                    let task = config_file_lock.get_public_task(task);
                    match task {
                        Some(task) => {
                            println!("{}", &path.to_string_lossy().yamis_info());
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

// TODO: Handle
impl TaskSubcommand {
    /// Returns a new TaskSubcommand
    pub(crate) fn new(args: &clap::ArgMatches) -> Result<TaskSubcommand, ArgsError> {
        let mut kwargs = TaskArgs::new();

        let (task_name, task_args) = match args.subcommand() {
            None => return Err(ArgsError::MissingTaskArg),
            Some(command) => command,
        };

        if let Some(args) = task_args.get_many::<OsString>("") {
            // All args are pushed into a vector as they are
            let all_args = args
                .clone()
                .map(|s| s.to_string_lossy().to_string())
                .collect::<Vec<String>>();
            kwargs.insert(String::from("*"), all_args);

            // kwarg found that could be a key
            let mut possible_kwarg_key = None;

            // looping over the args to find kwargs
            for arg in args {
                let arg = arg.to_string_lossy().to_string();
                // if a kwarg key was previously found, assume this is the value, even if
                // it starts with - or --
                if let Some(possible_kwarg) = possible_kwarg_key {
                    match kwargs.entry(possible_kwarg) {
                        Entry::Occupied(mut e) => {
                            e.get_mut().push(arg);
                        }
                        Entry::Vacant(e) => {
                            let args_vec: Vec<String> = vec![arg];
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
                if let Some((key, val)) = Self::get_kwarg(&arg) {
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
                if let Some(key) = Self::get_kwarg_key(&arg) {
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
                .help("Lists configuration files that can be reached from the current directory")
                .action(ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("list-tasks")
                .short('t')
                .long("list-tasks")
                .help("Lists tasks")
                .conflicts_with_all(["task-info"])
                .action(ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("task-info")
                .short('i')
                .long("task-info")
                .action(ArgAction::Set)
                .help("Displays information about the given task")
                .value_name("TASK"),
        )
        .arg(
            clap::Arg::new("file")
                .short('f')
                .long("file")
                .action(ArgAction::Set)
                .help("Search for tasks in the given file")
                .value_name("FILE"),
        )
        .arg(
            clap::Arg::new("global")
                .short('g')
                .long("global")
                .help("Search for tasks in ~/yamis/yamis.global.{yml,yaml}")
                .conflicts_with_all(["file"])
                .action(ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("update")
                .long("update")
                .help("Checks for updates and updates the binary if necessary")
                .exclusive(true)
                .action(ArgAction::SetTrue),
        );
    let matches = app.get_matches();

    if matches.get_one::<bool>("update").cloned().unwrap_or(false) {
        updater::update()?;
        return Ok(());
    } else {
        match updater::check_update_available() {
            Ok(result) => {
                if let Some(msg) = result {
                    println!("{}", msg.yamis_prefix_info());
                }
            }
            Err(e) => {
                let err_msg = format!("Error checking for updates: {}", e);
                eprintln!("{}", err_msg.yamis_error());
            }
        }
    }

    let current_dir = env::current_dir()?;
    let mut file_containers = ConfigFileContainers::new();

    let config_file_paths: PathIterator = match matches.get_one::<String>("file") {
        None => match matches.get_one::<bool>("global").cloned().unwrap_or(false) {
            true => GlobalConfigFilePath::new(),
            false => ConfigFilePaths::new(&current_dir),
        },
        Some(file_path) => SingleConfigFilePath::new(file_path),
    };

    if matches
        .get_one::<bool>("list-tasks")
        .cloned()
        .unwrap_or(false)
    {
        file_containers.print_tasks_list(config_file_paths)?;
        return Ok(());
    };

    if let Some(task_name) = matches.get_one::<String>("task-info") {
        file_containers.print_task_info(config_file_paths, task_name)?;
        return Ok(());
    };

    if matches.get_one::<bool>("list").cloned().unwrap_or(false) {
        for path in config_file_paths {
            println!("{}", colorize_config_file_path(&path.to_string_lossy()));
        }
        return Ok(());
    }

    let task_command = TaskSubcommand::new(&matches)?;

    file_containers.run_task(config_file_paths, &task_command.task, task_command.args)
}

#[cfg(test)]
mod tests {
    use assert_cmd::Command;
    use assert_fs::TempDir;
    use predicates::prelude::{predicate, PredicateBooleanExt};
    use std::fs::File;

    #[test]
    // #[ignore = "Fails but works fine when run manually"]
    fn test_list() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_dir = TempDir::new().unwrap();
        let tmp_dir_path = tmp_dir.path();
        File::create(tmp_dir_path.join("yamis.private.yml"))?;
        File::create(tmp_dir_path.join("yamis.root.yml"))?;
        File::create(tmp_dir_path.join("yamis.other.yml"))?;

        let expected_private = format!(
            "{tmp_dir}\n",
            tmp_dir = tmp_dir_path.join("yamis.private.yml").to_str().unwrap()
        );
        let expected_root = format!(
            "{tmp_dir}\n",
            tmp_dir = tmp_dir_path.join("yamis.root.yml").to_str().unwrap()
        );
        let mut cmd = Command::cargo_bin("yamis")?;
        cmd.current_dir(tmp_dir_path);
        cmd.arg("--list");
        cmd.assert().success().stdout(
            predicate::str::contains(expected_private)
                .and(predicate::str::ends_with(expected_root)),
        );
        Ok(())
    }
}
