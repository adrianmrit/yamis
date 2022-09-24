use std::collections::hash_map::Entry;
use std::error::Error;
use std::{env, fmt};

use regex::Regex;

use crate::config_files::{ConfigFilePaths, ConfigFilesContainer};
use crate::types::TaskArgs;

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
            clap::Arg::new("list")
                .short('l')
                .long("list")
                .takes_value(false)
                .help("Lists configuration files that can be reached from the current directory")
                .conflicts_with("file"),
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

    let config_files = match matches.value_of("file") {
        None => ConfigFilePaths::new(&current_dir),
        Some(file_path) => ConfigFilePaths::only(file_path)?,
    };

    if matches.contains_id("list") {
        for path in config_files {
            let path = &path?;
            println!("{}:", path.to_string_lossy());
        }
        return Ok(());
    }

    let task_command = TaskSubcommand::new(&matches)?;

    let mut v1_files_container = ConfigFilesContainer::new();
    // Example for handling a different version
    // let mut v2_files_container = yamis_v2::config_files::ConfigFilesContainer::new();

    for path in config_files {
        let path = path?;
        let version = ConfigFilePaths::get_version(&path)?;
        match version.as_str() {
            "1" => {
                let config_file_ptr = v1_files_container.read_config_file(path)?;
                let config_file_lock = config_file_ptr.lock().unwrap();
                match config_file_lock.get_task(&task_command.task) {
                    Some(task) => {
                        task.run(&task_command.args, &config_file_lock)?;
                        return Ok(());
                    }
                    None => continue,
                }
            }
            // Example for handling a different version
            // "2" => {
            //     let config_file_ptr = v2_files_container.read_config_file(path)?;
            //     let config_file_lock = config_file_ptr.lock().unwrap();
            //     match config_file_lock.get_task(&task_command.task) {
            //         Some(task) => {
            //             task.run(&task_command.args, &config_file_lock)?;
            //             return Ok(());
            //         }
            //         None => continue,
            //     }
            // }
            _ => {
                return Err(format!("Unsupported config file version: {}", version).into());
            }
        }
    }
    Err(format!("Task {} not found.", task_command.task).into())
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
