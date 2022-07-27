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
    script = "echo \"hello world\""
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

#[test]
fn test_escape_always() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = TempDir::new().unwrap();
    let mut file = File::create(tmp_dir.join("project.yamis.toml"))?;
    file.write_all(
        r#"
    [tasks.say_hello]
    quote = "always"
    script = "echo {1} {2} {hello}{4?} {*}"
    "#
        .as_bytes(),
    )?;

    let mut cmd = Command::cargo_bin("yamis")?;
    cmd.current_dir(tmp_dir.path());
    cmd.arg("say_hello");
    cmd.args(["hello", "world", "--hello=hello world"]);
    cmd.assert().success().stdout(predicate::str::contains(
        "\"hello\" \"world\" \"hello world\" \"hello\" \"world\" \"--hello=hello world\"",
    ));
    Ok(())
}

#[test]
fn test_escape_on_space() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = TempDir::new().unwrap();
    let mut file = File::create(tmp_dir.join("project.yamis.toml"))?;
    file.write_all(
        r#"
    [tasks.say_hello]
    quote = "spaces"
    script = "echo {1} {2} {hello}{4?} {*}"
    "#
        .as_bytes(),
    )?;

    let mut cmd = Command::cargo_bin("yamis")?;
    cmd.current_dir(tmp_dir.path());
    cmd.arg("say_hello");
    cmd.args(["hello", "world", "--hello=hello world"]);
    cmd.assert().success().stdout(predicate::str::contains(
        "hello world \"hello world\" hello world \"--hello=hello world\"",
    ));
    Ok(())
}

#[test]
fn test_escape_never() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = TempDir::new().unwrap();
    let mut file = File::create(tmp_dir.join("project.yamis.toml"))?;
    file.write_all(
        r#"
    [tasks.say_hello]
    quote = "never"
    script = "echo {1} {2} {hello}{4?} {*}"
    "#
        .as_bytes(),
    )?;

    let mut cmd = Command::cargo_bin("yamis")?;
    cmd.current_dir(tmp_dir.path());
    cmd.arg("say_hello");
    cmd.args(["hello", "world", "--hello=hello world"]);
    cmd.assert().success().stdout(predicate::str::contains(
        "hello world hello world hello world --hello=hello world",
    ));
    Ok(())
}

#[test]
fn test_run_os_task() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = TempDir::new().unwrap();
    let mut file = File::create(tmp_dir.join("project.yamis.toml"))?;
    file.write_all(
        r#"
    [tasks.hello.windows]
    script = "echo hello windows"
    
    [tasks.hello.linux]
    script = "echo hello linux"
    
    [tasks.hello.macos]
    script = "echo hello macos"
    
    [tasks.hello_again]
    script = "echo hello windows"
    
    [tasks.hello_again.linux]
    script = "echo hello linux"
    
    [tasks.hello_again.macos]
    script = "echo hello macos"
    "#
        .as_bytes(),
    )?;

    let expected = if cfg!(target_os = "windows") {
        "hello windows"
    } else if cfg!(target_os = "linux") {
        "hello linux"
    } else {
        "hello macos"
    };

    let mut cmd = Command::cargo_bin("yamis")?;
    cmd.current_dir(tmp_dir.path());
    cmd.arg("hello");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(expected));

    let mut cmd = Command::cargo_bin("yamis")?;
    cmd.current_dir(tmp_dir.path());
    cmd.arg("hello_again");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(expected));
    Ok(())
}

#[test]
fn test_set_env() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = TempDir::new().unwrap();
    let mut file = File::create(tmp_dir.join("project.yamis.toml"))?;
    file.write_all(
        r#"
    [env]
    greeting = "hello world"
    one_plus_one = "two"
    
    [tasks.hello.windows]
    script = "echo %greeting%, one plus one is %one_plus_one%"
    
    [tasks.hello]
    script = "echo $greeting, one plus one is $one_plus_one"
    
    [tasks.hello.env]
    greeting = "hi world"
    
    [tasks.hello.windows.env]
    greeting = "hi world"
    "#
        .as_bytes(),
    )?;

    let mut cmd = Command::cargo_bin("yamis")?;
    cmd.current_dir(tmp_dir.path());
    cmd.arg("hello");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("hi world, one plus one is two"));
    Ok(())
}
