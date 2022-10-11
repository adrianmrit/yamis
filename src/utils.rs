use crate::tasks::Task;
use crate::types::DynErrResult;
use dotenv_parser::parse_dotenv;
use petgraph::graphmap::DiGraphMap;
use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::{env, fs};

/// Returns the task name as per the current OS.
///
/// # Arguments
///
/// * `task_name`: Plain name of the task
///
/// returns: ()
///
/// # Examples
///
/// ```ignore
/// // Assuming it is a linux system
/// assert_eq!(to_os_task_name("sample"), "sample.linux");
/// ```
pub fn to_os_task_name(task_name: &str) -> String {
    format!("{}.{}", task_name, env::consts::OS)
}

/// Returns a directed graph containing dependency relations dependency for the given tasks, where
/// the nodes are the names of the tasks. The graph does not include tasks that do not depend, or
/// are not dependencies of other tasks. It is also possible that the graph contains multiple
/// connected components, that is, subgraphs that are not part of larger connected subgraphs.
///
/// # Arguments
///
/// * `tasks`: Hashmap of name to task
///
/// returns: Result<GraphMap<&str, (), Directed>, Box<dyn Error, Global>>
pub fn get_task_dependency_graph<'a>(
    tasks: &'a HashMap<String, Task>,
) -> DynErrResult<DiGraphMap<&'a str, ()>> {
    let mut graph: DiGraphMap<&'a str, ()> = DiGraphMap::new();

    let mut bases_stack: Vec<&str> = vec![];
    for (task_name, task) in tasks {
        let mut current_task = task;
        let mut current_task_name: &str = task_name;

        if current_task.bases.is_empty() {
            continue;
        }

        loop {
            for base_name in &current_task.bases {
                let os_base_name = to_os_task_name(base_name);
                let base_name = if tasks.contains_key(&os_base_name) {
                    // os_base_name needs to be a reference to the string in the HashMap
                    let (os_base_name, _) = tasks.get_key_value(&os_base_name).unwrap();
                    os_base_name
                } else {
                    base_name
                };
                if !graph.contains_node(base_name) {
                    bases_stack.push(base_name);
                }
                graph.add_edge(current_task_name, base_name, ());
            }
            while let Some(base) = bases_stack.pop() {
                match tasks.get(base) {
                    None => {
                        return Err(format!(
                            "Task {} cannot inherit from non-existing task {}.",
                            current_task_name, base
                        )
                        .into())
                    }
                    Some(new_current_task) => {
                        current_task = new_current_task;
                        current_task_name = base;
                    }
                }
            }
            if bases_stack.is_empty() {
                break;
            }
        }
    }

    Ok(graph)
}

/// Returns the path relative to the base. If path is already absolute, it will be returned instead.
///
/// # Arguments
///
/// * `base`: Base path
/// * `path`: Path to make relative to the base
///
/// returns: PathBuf
pub fn get_path_relative_to_base<B: AsRef<OsStr> + ?Sized, P: AsRef<OsStr> + ?Sized>(
    base: &B,
    path: &P,
) -> PathBuf {
    let path = Path::new(path);
    if !path.is_absolute() {
        let base = Path::new(base);
        return base.join(path);
    }
    path.to_path_buf()
}

/// Reads the content of an environment file from the given path and returns a BTreeMap.
///
/// # Arguments
/// * `path`: Path of the environment file
///
/// returns: DynErrResult<BTreeMap<String, String>>
pub fn read_env_file<S: AsRef<OsStr> + ?Sized>(path: &S) -> DynErrResult<BTreeMap<String, String>> {
    let path = Path::new(path);
    let result = match fs::read_to_string(path) {
        Ok(content) => parse_dotenv(&content),
        Err(err) => {
            return Err(format!("Failed to read env file at {}: {}", path.display(), err).into())
        }
    };

    match result {
        Ok(envs) => Ok(envs),
        Err(err) => Err(format!("Failed to parse env file at {}: {}", path.display(), err).into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use std::env;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_read_env_file_not_found() {
        let env_file_path = env::current_dir().unwrap().join("non_existent.env");
        let env_map = read_env_file(&env_file_path).unwrap_err();
        cfg_if::cfg_if! {
            if #[cfg(target_os = "windows")] {
                let expected_error: &str = "The system cannot find the file specified. (os error 2)";
            } else {
                let expected_error: &str = "No such file or directory (os error 2)";
            }
        }
        assert_eq!(
            env_map.to_string(),
            format!(
                "Failed to read env file at {}: {}",
                env_file_path.display(),
                expected_error
            )
        );
    }

    #[test]
    fn test_read_env_file_invalid() {
        let tmp_dir = TempDir::new().unwrap();
        let env_file_path = tmp_dir.join(".env");
        let mut file = File::create(&env_file_path).unwrap();
        file.write_all(r#"INVALID_ENV_FILE"#.as_bytes()).unwrap();
        let env_map = read_env_file(&env_file_path).unwrap_err();
        dbg!(env_map.to_string());
        let expected_err = format!("Failed to parse env file at {}: ", env_file_path.display());
        assert!(env_map.to_string().contains(&expected_err),);
    }

    #[test]
    fn test_read_env_file() {
        let tmp_dir = TempDir::new().unwrap();
        let env_file_path = tmp_dir.join(".env");
        let mut file = File::create(&env_file_path).unwrap();
        file.write_all(
            r#"
    TEST_VAR=test_value
    "#
            .as_bytes(),
        )
        .unwrap();
        let env_map = read_env_file(&env_file_path).unwrap();
        assert_eq!(env_map.get("TEST_VAR"), Some(&"test_value".to_string()));
    }

    #[test]
    fn test_get_path_relative_to_base() {
        let base = "/home/user";
        let path = "test";
        let path = get_path_relative_to_base(base, path);
        assert_eq!(path, PathBuf::from("/home/user/test"));

        let base = "/home/user";
        let path = "/test";
        let path = get_path_relative_to_base(base, path);
        assert_eq!(path, PathBuf::from("/test"));
    }
}
