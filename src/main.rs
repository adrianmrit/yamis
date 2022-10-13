#[cfg(feature = "runtime")]
use colored::Colorize;
use yamis::print_utils::YamisOutput;

#[cfg(feature = "runtime")]
use yamis::cli::exec;

#[cfg(feature = "runtime")]
fn main() {
    match exec() {
        Ok(_) => {}
        Err(e) => {
            let err_msg = e.to_string().red();
            eprint!("{}", err_msg.yamis_prefix(colored::Color::Red));
            std::process::exit(1);
        }
    }
}
