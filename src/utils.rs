use crate::tasks::Task;
use crate::types::DynErrResult;
use dotenv_parser::parse_dotenv;
use petgraph::graphmap::DiGraphMap;
use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

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
            for base in &current_task.bases {
                if !graph.contains_node(base) {
                    bases_stack.push(base);
                }
                graph.add_edge(current_task_name, base, ());
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
    Ok(match fs::read_to_string(path) {
        Ok(file_contents) => match parse_dotenv(&file_contents) {
            Ok(result) => result,
            Err(e) => return Err(e),
        },
        Err(e) => {
            return Err(format!(
                "There was an error reading the env file at {}:\n{}",
                path.display(),
                e
            )
            .into())
        }
    })
}

/// Formats an error message such that the first character is capitalized and each line
/// is prepended with 4 spaces.
///
/// # Arguments
///
/// * `error`: Error to format
///
/// returns: String
///
pub fn sub_error_str(error: &str) -> String {
    let lines: Vec<&str> = error.lines().rev().collect();
    // We add 4 extra spaces per line
    let mut new = String::with_capacity(4 * lines.len() + error.len());
    let mut lines = lines.iter();
    if let Some(first_line) = lines.next() {
        if !first_line.is_empty() {
            new.push_str("    ");
            new.push_str(&first_line[0..1].to_uppercase());
            new.push_str(&first_line[1..])
        }
    }
    for line in lines {
        new.push_str("    ");
        new.push_str(line);
    }
    new
}
