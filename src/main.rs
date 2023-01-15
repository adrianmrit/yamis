#[cfg(feature = "runtime")]
use yamis::print_utils::YamisOutput;

#[cfg(feature = "runtime")]
use yamis::cli::exec;

#[cfg(feature = "runtime")]
fn main() {
    match exec() {
        Ok(_) => {}
        Err(e) => {
            eprint!("{}", e.to_string().yamis_error());
            std::process::exit(1);
        }
    }
}
