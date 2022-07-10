use colored::Colorize;
use std::env;
use std::error::Error;
use yamis::args::YamisArgs;
use yamis::tasks::{ConfigFile, ConfigFiles};

/// Runs the program but returns errors. The main method should be
/// the one to print the actual error.
fn program() -> Result<(), Box<dyn Error>> {
    let args = YamisArgs::new(env::args());
    return match args {
        YamisArgs::CommandArgs(args) => {
            let config_files = match args.file {
                None => ConfigFiles::discover()?,
                Some(file_path) => ConfigFiles::for_path(&file_path)?,
            };
            match args.task {
                None => Err("No task to run was given.")?,
                Some(task_name) => {
                    let task_and_config = config_files.get_task(&task_name);
                    match task_and_config {
                        None => Err(format!("Task {task_name} not found."))?,
                        Some((task, config)) => {
                            task.run(&args.args, config)?;
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
            return;
        }
    }
}
