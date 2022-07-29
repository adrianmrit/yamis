use colored::Colorize;
use std::env;
use std::error::Error;
use yamis::args::YamisArgs;
use yamis::tasks::ConfigFiles;

/// Runs the program but returns errors. The main method should be
/// the one to print the actual error.
fn program() -> Result<(), Box<dyn Error>> {
    let args = YamisArgs::new(env::args());
    return match args {
        YamisArgs::CommandArgs(args) => {
            let config_files = match args.file {
                None => ConfigFiles::discover(&env::current_dir()?)?,
                Some(file_path) => ConfigFiles::for_path(&file_path)?,
            };
            match args.task {
                None => return Err("No task to run was given.".into()),
                Some(task_name) => {
                    let task_and_config = config_files.get_task(&task_name);
                    match task_and_config {
                        None => return Err(format!("Task {task_name} not found.").into()),
                        Some((task, config)) => {
                            task.validate()?;
                            task.run(&args.args, config, &config_files)?;
                            Ok(())
                        }
                    }
                }
            }
        }
    };
}

fn main() {
    match program() {
        Ok(_) => {}
        Err(e) => {
            let err_msg = e.to_string();
            let prefix = "[YAMIS]".bright_yellow();
            for line in err_msg.lines() {
                eprintln!("{} {}", prefix, line.red());
            }
            std::process::exit(1);
        }
    }
}
