use yamis::args::get_args;
use yamis::tasks::{ConfigFile, ConfigFiles};

fn main() {
    let args = get_args();
    let configs = ConfigFiles::discover().unwrap();
    let task = configs.get_task("python").unwrap();
    task.run(&args).unwrap();
}
