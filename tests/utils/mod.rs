use std::path::Path;
use yamis::tasks::Task;
use yamis::types::DynErrResult;

pub fn get_task(name: &str, definition: &str, base_path: Option<&Path>) -> DynErrResult<Task> {
    let mut task: Task = toml::from_str(definition).unwrap();
    task.setup(name, base_path.unwrap_or_else(|| Path::new("")))?;
    Ok(task)
}
