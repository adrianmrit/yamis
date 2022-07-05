extern crate core;

use std::env;
use std::env::Args;
use colored::Colorize;
use crate::args::YamisArgs;
use crate::tasks::{ConfigFiles, Task};

pub mod args;
pub mod tasks;

// TODO: Properly handle errors
pub fn program(args: Args) {
    let args = YamisArgs::new(env::args());
    match args {
        YamisArgs::CommandArgs(args) => {
            let config_files= match args.file {
                None => {
                    match ConfigFiles::discover() {
                        Ok(c) => {c}
                        Err(e) => {eprintln!("{}", e.to_string().red()); return;}
                    }
                }
                Some(file_path) => {ConfigFiles::for_path(&file_path).unwrap()}
            };
            match args.task {
                None => {eprintln!("not implemented"); return;}
                Some(task_name) => {
                    let task = config_files.get_task(&task_name);
                    match task {
                        None => {
                            eprintln!("{} {} {}", "Task".red(), task_name.red(), "not found.".red());
                            return;
                        }
                        Some(task) => {
                            match task.run(&args.args) {
                                Ok(_) => {}
                                Err(e) => {eprintln!("{}", e.to_string().red())}
                            }
                        }
                    }
                }
            }
        }
    }
}