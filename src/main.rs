use yamis::args::get_args;
use yamis::tasks::ConfigFile;

fn main() {
    let args = get_args();
    let config = ConfigFile::load("src/sample.toml");
    let task = &config.unwrap().tasks.unwrap()["py"];
    task.run(&args).unwrap();
}
