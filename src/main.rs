use colored::Colorize;
use std::env;
use std::error::Error;
use yamis::program;
use yamis::tasks::{ConfigFile, ConfigFiles};

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
