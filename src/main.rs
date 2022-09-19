use colored::Colorize;

use yamis::cli::exec;

fn main() {
    match exec() {
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
