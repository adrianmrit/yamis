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
#[cfg(windows)] // echo does not prints the quotes in unix
fn test_escape_always_windows() -> Result<(), Box<dyn std::error::Error>> {
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
#[cfg(windows)] // echo does not prints the quotes in unix
fn test_escape_on_space_windows() -> Result<(), Box<dyn std::error::Error>> {
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

// TODO: Test escaping in Unix
//   Not critical since we already test the script formatter

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

#[test]
fn test_env_file() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = TempDir::new().unwrap();
    let mut env_file = File::create(tmp_dir.join(".env"))?;
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

    let mut env_file_2 = File::create(tmp_dir.join(".env_2"))?;
    env_file_2
        .write_all(
            r#"
    VAR1=OTHER_VAL1
    VAR2=OTHER_VAL2
    "#
            .as_bytes(),
        )
        .unwrap();

    let mut file = File::create(tmp_dir.join("project.yamis.toml"))?;
    file.write_all(
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
    )?;

    let mut cmd = Command::cargo_bin("yamis")?;
    cmd.current_dir(tmp_dir.path());
    cmd.arg("test");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("VAL1 VAL2 VAL3"));

    let mut cmd = Command::cargo_bin("yamis")?;
    cmd.current_dir(tmp_dir.path());
    cmd.arg("test_2");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("TASK_VAL1 OTHER_VAL2 VAL3"));

    Ok(())
}

#[test]
fn test_run_program() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = TempDir::new().unwrap();
    let (program, param, batch_file_name, batch_file_content) = if cfg!(target_os = "windows") {
        ("cmd", "/C", "echo_args.cmd", "echo %1 %2 %*".as_bytes())
    } else {
        ("bash", "", "echo_args.bash", "echo $1 $2 $*".as_bytes())
    };
    let mut batch_file = File::create(tmp_dir.join(batch_file_name))?;
    batch_file.write_all(batch_file_content).unwrap();

    let mut file = File::create(tmp_dir.join("project.yamis.toml"))?;
    file.write_all(
        format!(
            r#"
            [tasks.hello]
            program = "{}"
            args = ["{}", "{}", "hello", "world"]
            "#,
            program, param, batch_file_name
        )
        .as_bytes(),
    )?;

    let mut cmd = Command::cargo_bin("yamis")?;
    cmd.current_dir(tmp_dir.path());
    cmd.arg("hello");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("hello world hello world"));

    Ok(())
}

#[test]
fn test_run_serial() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = TempDir::new().unwrap();
    let (program, param, batch_file_name, batch_file_content) = if cfg!(target_os = "windows") {
        ("cmd", "/C", "echo_args.cmd", "echo Hello %*".as_bytes())
    } else {
        ("bash", "", "echo_args.bash", "echo Hello $*".as_bytes())
    };
    let mut batch_file = File::create(tmp_dir.join(batch_file_name))?;
    batch_file.write_all(batch_file_content).unwrap();

    let mut file = File::create(tmp_dir.join("project.yamis.toml"))?;
    file.write_all(
        format!(
            r#"
            [tasks.hello]
            program = "{}"
            args = ["{}", "{}", "{{1}}"]
            
            [tasks.bye]
            quote = "never"
            script = "echo Bye {{2}}"
            
            [tasks.greet]
            serial = ["hello", "bye"]
            "#,
            program, param, batch_file_name
        )
        .as_bytes(),
    )?;

    let mut cmd = Command::cargo_bin("yamis")?;
    cmd.current_dir(tmp_dir.path());
    cmd.arg("greet");
    cmd.args(vec!["world", "everyone"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Hello world"))
        .stdout(predicate::str::contains("Bye everyone"));

    Ok(())
}

#[test]
fn test_env_inheritance() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = TempDir::new().unwrap();
    let mut file = File::create(tmp_dir.join("project.yamis.toml"))?;
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
    )?;

    let mut cmd = Command::cargo_bin("yamis")?;
    cmd.current_dir(tmp_dir.path());
    cmd.arg("hello");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("hello world, 1+1=2"));
    Ok(())
}

#[test]
fn test_extend_args() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = TempDir::new().unwrap();
    let (batch_file_name, batch_file_content) = if cfg!(target_os = "windows") {
        ("echo_args.cmd", "echo %*".as_bytes())
    } else {
        ("echo_args.bash", "echo $1 $2 $*".as_bytes())
    };
    let mut batch_file = File::create(tmp_dir.join(batch_file_name))?;
    batch_file.write_all(batch_file_content).unwrap();

    let mut file = File::create(tmp_dir.join("project.yamis.toml"))?;
    file.write_all(
        format!(
            r#"
            [tasks.echo_program]
            program = "bash"
            args = ["-c", "{b}"]
            private=true
            
            [tasks.echo_program.windows]
            program = "cmd.exe"
            args = ["/C", "{b}"]
            private=true

            [tasks.hello]
            bases = ["echo_program"]
            args_extend = ["hello", "world"]
            
            [tasks.hello.windows]
            bases = ["echo_program.windows"]
            args_extend = ["hello", "world"]
            "#,
            b = batch_file_name
        )
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
