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
