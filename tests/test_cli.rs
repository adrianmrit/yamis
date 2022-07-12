use assert_cmd::prelude::*;
use assert_fs::TempDir;
use predicates::prelude::*;
use std::fs::File;
use std::io::Write;
use std::process::Command;
use yamis::types::DynErrResult;

#[test]
fn test_no_config_file_discovered() -> DynErrResult<()> {
    let tmp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("yamis")?;
    cmd.current_dir(tmp_dir.path());
    cmd.arg("echo");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("No config file found."));
    Ok(())
}

#[test]
fn test_run_simple_task() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = TempDir::new().unwrap();
    let mut file = File::create(tmp_dir.join("project.yamis.toml"))?;
    file.write_all(
        r#"
    [tasks.hello]
    script = "hello world"
    "#
        .as_bytes(),
    )?;

    let mut cmd = Command::cargo_bin("yamis")?;
    cmd.current_dir(tmp_dir.path());
    cmd.arg("hello");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("hello world"));

    Ok(())
}
