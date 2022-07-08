extern crate core;

use std::env;
use std::env::Args;
use std::error::Error;

use crate::args::YamisArgs;
use crate::tasks::ConfigFiles;

pub mod args;
pub mod tasks;

pub fn program() -> Result<(), Box<dyn Error>> {
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
