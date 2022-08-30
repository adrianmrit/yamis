use crate::config_files::ConfigFiles;
use regex::Regex;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::error::Error;
use std::{env, fmt};

const HELP: &str = "The appropriate YAML or TOML config files need to exist \
in the directory or parents, or a file is specified with the `-f` or `--file` \
options. For help about the config files check https://github.com/adrianmrit/yamis";

/// Extra args passed that will be mapped to the task.
pub type TaskArgs = HashMap<String, Vec<String>>;

/// Holds the data for running the given task.
struct TaskSubcommand {
    /// Task to run, if given
    pub task: String,
    /// Args to run the command with
    pub args: TaskArgs,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ArgsError {
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
pub fn exec() -> Result<(), Box<dyn Error>> {
    let app = clap::Command::new(clap::crate_name!())
        .version(clap::crate_version!())
        .about(clap::crate_description!())
        .author(clap::crate_authors!())
        .after_help(HELP)
        .allow_external_subcommands(true)
        .arg(
            clap::Arg::new("file")
                .short('f')
                .long("file")
                .help("Search for tasks in the given file")
                .takes_value(true)
                .value_name("FILE"),
        );
    let matches = app.get_matches();

    let task_command = TaskSubcommand::new(&matches)?;

    let config_files = match matches.value_of("file") {
        None => ConfigFiles::discover(&env::current_dir()?)?,
        Some(file_path) => ConfigFiles::for_path(&file_path)?,
    };

    let name_task_and_config = config_files.get_system_task(&task_command.task);
    match name_task_and_config {
        None => Err(format!("Task {} not found.", task_command.task).into()),
        Some((task, config)) => {
            task.run(&task_command.args, config, &config_files)?;
            Ok(())
        }
    }
}
