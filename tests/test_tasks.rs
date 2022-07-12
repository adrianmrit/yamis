use assert_fs::TempDir;
use std::fs::File;
use std::io::Write;
use yamis::tasks::ConfigFiles;
use yamis::types::DynErrResult;

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
            assert!(false, "task non_existent should not exist");
        }
    }

    match config.get_task("hello_world") {
        None => {
            assert!(false, "task hello_world should exist");
        }
        Some((_, _)) => {}
    }

    let config = ConfigFiles::for_path(path.as_path()).unwrap();
    assert_eq!(config.configs.len(), 1);
    Ok(())
}

#[test]
fn test_task_by_platform() -> DynErrResult<()> {
    let tmp_dir = TempDir::new().unwrap();
    dbg!(tmp_dir.path());
    let path = tmp_dir.join("project.yamis.toml");

    let mut file = File::create(path.as_path())?;
    file.write_all(
        r#"
    [tasks.os_sample]
    script = "echo hello linux"

    [tasks.os_sample.windows]
    script = "echo hello windows"

    [tasks.os_sample.macos]
    script = "echo hello macos"
    "#
        .as_bytes(),
    )?;

    let config = ConfigFiles::discover(&tmp_dir.path()).unwrap();
    assert_eq!(config.configs.len(), 1);

    match config.get_task("os_sample") {
        None => {}
        Some((task, _)) => {
            if cfg!(target_os = "windows") {
                assert_eq!(
                    task.script.clone().unwrap(),
                    String::from("echo hello windows")
                );
            } else if cfg!(target_os = "linux") {
                assert_eq!(
                    task.script.clone().unwrap(),
                    String::from("echo hello linux")
                );
            } else {
                assert_eq!(
                    task.script.clone().unwrap(),
                    String::from("echo hello linux")
                );
            }
        }
    }
    Ok(())
}
