use crate::utils::get_task;
use assert_fs::TempDir;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use yamis::config_files::ConfigFile;
use yamis::tasks::TaskError;

mod utils;

#[test]
fn test_env_inheritance() {
    let tmp_dir = TempDir::new().unwrap();
    let config_file_path = tmp_dir.join("project.yamis.toml");
    let mut file = File::create(&config_file_path).unwrap();
    file.write_all(
        r#" 
    [tasks.hello_base.env]
    greeting = "hello world"
    
    [tasks.calc_base.env]
    one_plus_one = "2"
    
    [tasks.hello]
    bases = ["hello_base", "calc_base"]
    script = "echo $greeting, 1+1=$one_plus_one"
    
    [tasks.hello.windows]
    bases = ["hello_base", "calc_base"]
    script = "echo %greeting%, 1+1=%one_plus_one%"
    "#
        .as_bytes(),
    )
    .unwrap();

    let config_file = ConfigFile::load(config_file_path).unwrap();

    let task = config_file.get_task("hello").unwrap();

    let env = task.get_env(&config_file);
    let expected = HashMap::from([
        ("greeting".to_string(), "hello world".to_string()),
        ("one_plus_one".to_string(), "2".to_string()),
    ]);
    assert_eq!(env, expected);
}

#[test]
fn test_read_env() {
    let tmp_dir = TempDir::new().unwrap();
    let project_config_path = tmp_dir.join("project.yamis.toml");
    let mut project_config_file = File::create(project_config_path.as_path()).unwrap();
    project_config_file
        .write_all(
            r#"
            env_file = ".env"
            
            [tasks.test.windows]
            quote = "never"
            script = "echo %VAR1% %VAR2% %VAR3%"
            
            [tasks.test]
            quote = "never"
            script = "echo $VAR1 $VAR2 $VAR3"
            
            [tasks.test_2.windows]
            quote = "never"
            script = "echo %VAR1% %VAR2% %VAR3%"
            env_file = ".env_2"
            env = {"VAR1" = "TASK_VAL1"}
            
            [tasks.test_2]
            quote = "never"
            script = "echo $VAR1 $VAR2 $VAR3"
            env_file = ".env_2"
            
            [tasks.test_2.env]
            VAR1 = "TASK_VAL1"
            "#
            .as_bytes(),
        )
        .unwrap();

    let mut env_file = File::create(tmp_dir.join(".env").as_path()).unwrap();
    env_file
        .write_all(
            r#"
    VAR1=VAL1
    VAR2=VAL2
    VAR3=VAL3
    "#
            .as_bytes(),
        )
        .unwrap();

    let mut env_file_2 = File::create(tmp_dir.join(".env_2").as_path()).unwrap();
    env_file_2
        .write_all(
            r#"
    VAR1=OTHER_VAL1
    VAR2=OTHER_VAL2
    "#
            .as_bytes(),
        )
        .unwrap();

    let config_file = ConfigFile::load(project_config_path).unwrap();

    let task = config_file.get_task("test").unwrap();
    let env = task.get_env(&config_file);

    let expected = HashMap::from([
        ("VAR1".to_string(), "VAL1".to_string()),
        ("VAR2".to_string(), "VAL2".to_string()),
        ("VAR3".to_string(), "VAL3".to_string()),
    ]);
    assert_eq!(env, expected);

    let task = config_file.get_task("test_2").unwrap();
    let env = task.get_env(&config_file);
    let expected = HashMap::from([
        ("VAR1".to_string(), "TASK_VAL1".to_string()),
        ("VAR2".to_string(), "OTHER_VAL2".to_string()),
        ("VAR3".to_string(), "VAL3".to_string()),
    ]);
    assert_eq!(env, expected);
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
        program = "python"
    "#,
        None,
    );

    let expected_error = TaskError::ImproperlyConfigured(
        String::from("sample"),
        String::from("`quote` parameter can only be set for scripts."),
    );
    assert_eq!(task.unwrap_err().to_string(), expected_error.to_string());
}
