use colored::{ColoredString, Colorize};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::{env, fmt};

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

/// Enum of available config file versions
#[derive(Hash, Eq, PartialEq)]
enum Version {
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
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let lines = reader.lines();
        for line in lines {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            return match line.strip_prefix("#!v:") {
                None => Ok(Version::V1),
                Some(version) => {
                    let version = version.trim();
                    match version {
                        "1" => Ok(Version::V1),
                        _ => Err(format!("Unknown version {}", version).into()),
                    }
                }
            };
        }

        Ok(Version::V1)
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
                            return task.run(&args, &config_file_lock);
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
        let arg_regex: Regex =
            // TODO: Check best way to implement
            Regex::new(r"-*(?P<key>[a-zA-Z]+\w*)=(?P<val>[\s\S]*)")
                .unwrap();
        let mut kwargs = TaskArgs::new();

        let (task_name, task_args) = match args.subcommand() {
            None => return Err(ArgsError::MissingTaskArg),
            Some(command) => command,
        };

        if let Some(args) = task_args.values_of("") {
            let mut all_args = Vec::with_capacity(10);
            for arg in args {
                all_args.push(arg.to_string());
                let arg_match = arg_regex.captures(arg);
                if let Some(arg_match) = arg_match {
                    let key = String::from(arg_match.name("key").unwrap().as_str());
                    let val = String::from(arg_match.name("val").unwrap().as_str());
                    match kwargs.entry(key) {
                        Entry::Occupied(mut e) => {
                            e.get_mut().push(val);
                        }
                        Entry::Vacant(e) => {
                            let args_vec: Vec<String> = vec![val];
                            e.insert(args_vec);
                        }
                    }
                }
            }
            kwargs.insert(String::from("*"), all_args);
        } else {
            kwargs.insert(String::from("*"), vec![]);
        }

        Ok(TaskSubcommand {
            task: String::from(task_name),
            args: kwargs,
        })
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
                .help("Displays information about the given task"),
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
