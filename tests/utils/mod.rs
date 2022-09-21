use std::path::Path;
use yamis::tasks::Task;

pub fn get_task(
    name: &str,
    definition: &str,
    base_path: Option<&Path>,
) -> Result<Task, Box<dyn std::error::Error>> {
    let mut task: Task = toml::from_str(definition).unwrap();
    task.setup(name, base_path.unwrap_or_else(|| Path::new("")))?;
    Ok(task)
}
