use crate::utils::get_task;
use assert_fs::TempDir;
use std::fs::File;
use std::io::Write;
use yamis::config_files::ConfigFiles;
use yamis::tasks::TaskError;
use yamis::types::DynErrResult;

mod utils;

#[test]
fn test_discovery() -> DynErrResult<()> {
    let tmp_dir = TempDir::new().unwrap();
    let path = tmp_dir.path().join("project.yamis.toml");
    let mut file = File::create(path.as_path())?;
    file.write_all(
        r#"
    [tasks.hello_world]
    script = "echo hello world"
    "#
        .as_bytes(),
    )?;

    let config = ConfigFiles::discover(&tmp_dir.path()).unwrap();
    assert_eq!(config.configs.len(), 1);

    match config.get_task("non_existent") {
        None => {}
        Some((_, _)) => {
            panic!("task non_existent should not exist");
        }
    }

    match config.get_task("hello_world") {
        None => {
            panic!("task hello_world should exist");
        }
        Some((_, _)) => {}
    }

    let config = ConfigFiles::for_path(path.as_path()).unwrap();
    assert_eq!(config.configs.len(), 1);
    Ok(())
}

#[test]
fn test_validate() {
    let task = get_task(
        "sample",
        r#"
        script = "hello world"
        program = "some_program"
    "#,
        None,
    );
    let expected_error = TaskError::ImproperlyConfigured(
        String::from("sample"),
        String::from("Cannot specify `script` and `program` at the same time."),
    );
    assert_eq!(task.unwrap_err().to_string(), expected_error.to_string());

    let task = get_task(
        "sample",
        r#"
        interpreter = []
    "#,
        None,
    );
    let expected_error = TaskError::ImproperlyConfigured(
        String::from("sample"),
        String::from("`interpreter` parameter cannot be an empty array."),
    );
    assert_eq!(task.unwrap_err().to_string(), expected_error.to_string());

    let task = get_task(
        "sample",
        r#"
        script = "echo hello"
        serial = ["sample"]
    "#,
        None,
    );

    let expected_error = TaskError::ImproperlyConfigured(
        String::from("sample"),
        String::from("Cannot specify `script` and `serial` at the same time."),
    );
    assert_eq!(task.unwrap_err().to_string(), expected_error.to_string());

    let task = get_task(
        "sample",
        r#"
        program = "python"
        serial = ["sample"]
    "#,
        None,
    );

    let expected_error = TaskError::ImproperlyConfigured(
        String::from("sample"),
        String::from("Cannot specify `program` and `serial` at the same time."),
    );
    assert_eq!(task.unwrap_err().to_string(), expected_error.to_string());

    let task = get_task(
        "sample",
        r#"
        quote = "spaces"
    "#,
        None,
    );

    let expected_error = TaskError::ImproperlyConfigured(
        String::from("sample"),
        String::from("`quote` parameter can only be set for scripts."),
    );
    assert_eq!(task.unwrap_err().to_string(), expected_error.to_string());
}
