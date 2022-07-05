extern crate core;

use std::env;
use std::env::Args;
use crate::args::YamisArgs;
use crate::tasks::{ConfigFiles, Task};

pub mod args;
pub mod tasks;

// TODO: Properly handle errors
pub fn program(args: Args) {
    let args = YamisArgs::new(env::args());
    match args {
        YamisArgs::CommandArgs(args) => {
            let config_file= match args.file {
                None => {ConfigFiles::discover().unwrap()}
                Some(file_path) => {ConfigFiles::for_path(&file_path).unwrap()}
            };
            match args.task {
                None => {panic!("not implemented")}
                Some(task) => {
                    let task = config_file.get_task(&task);
                    match task {
                        None => {panic!("task not found")}
                        Some(task) => {
                            task.run(&args.args).unwrap();
                        }
                    }
                }
            }
        }
    }
}