use assert_cmd::prelude::*;
use assert_fs::TempDir;
use predicates::prelude::*;
use std::fs::File;
use std::io::Write;
use std::process::Command;

#[test]
fn test_no_config_file_discovered() {
    let tmp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("yamis").unwrap();
    cmd.current_dir(tmp_dir.path());
    cmd.arg("echo");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("[YAMIS] Task echo not found"));
}

#[test]
fn test_run_simple_task() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = TempDir::new().unwrap();
    let mut file = File::create(tmp_dir.join("yamis.root.yml"))?;
    file.write_all(
        r#"
    tasks:
        hello:
            script: echo "hello world"
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
fn test_file_option() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = TempDir::new().unwrap();
    let mut file = File::create(tmp_dir.join("sample.yamis.yml"))?;
    file.write_all(
        r#"
    tasks:
        hello:
            script: "ls"

        hello.windows:
            script: "dir"
    "#
        .as_bytes(),
    )?;

    let mut cmd = Command::cargo_bin("yamis")?;
    cmd.current_dir(tmp_dir.path());
    cmd.args(["-f=sample.yamis.yml", "hello"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("sample.yamis.yml"));
    drop(file);
    drop(tmp_dir);
    Ok(())
}

#[test]
fn test_run_os_task() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = TempDir::new().unwrap();
    let mut file = File::create(tmp_dir.join("yamis.root.yml"))?;
    file.write_all(
        r#"
    tasks:
        hello.windows:
            script: echo hello windows

        hello.linux:
            script: echo hello linux

        hello.macos:
            script: echo hello macos

        hello_again:
            script: echo hello windows

        hello_again.linux:
            script: echo hello linux

        hello_again.macos:
            script: echo hello macos
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
    let mut file = File::create(tmp_dir.join("yamis.root.yml"))?;
    file.write_all(
        r#"
env:
    greeting: "hello world"
    one_plus_one: "two"

tasks:
    hello.windows:
        script: echo %greeting%, one plus one is %one_plus_one%
        env:
            greeting: "hi world"

    hello:
        script: "echo $greeting, one plus one is $one_plus_one"
        env:
            greeting: "hi world"
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

    let mut file = File::create(tmp_dir.join("yamis.root.yml"))?;
    file.write_all(
        r#"
env_file: ".env"

tasks:
    test.windows:
        script: "echo %VAR1% %VAR2% %VAR3%"

    test:
        script: "echo $VAR1 $VAR2 $VAR3"

    test_2.windows:
        script: "echo %VAR1% %VAR2% %VAR3%"
        env_file: ".env_2"
        env: {VAR1: "TASK_VAL1"}

    test_2:
        script: echo $VAR1 $VAR2 $VAR3
        env_file: .env_2
        env:
            VAR1: "TASK_VAL1"
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
        ("bash", "", "echo_args.sh", "echo $1 $2 $*".as_bytes())
    };
    let mut batch_file = File::create(tmp_dir.join(batch_file_name))?;
    batch_file.write_all(batch_file_content).unwrap();

    let mut file = File::create(tmp_dir.join("yamis.root.yml"))?;
    file.write_all(
        format!(
            r#"
    tasks:
        hello:
            program: {}
            args: {} {} hello world
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
fn test_run_cmds() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = TempDir::new().unwrap();
    let (program, param, batch_file_name, batch_file_content) = if cfg!(target_os = "windows") {
        ("cmd", "/C", "echo_args.cmd", "echo %1 %2".as_bytes())
    } else {
        ("bash", "", "echo_args.sh", "echo $1 $2".as_bytes())
    };
    let mut batch_file = File::create(tmp_dir.join(batch_file_name))?;
    batch_file.write_all(batch_file_content).unwrap();

    let mut file = File::create(tmp_dir.join("yamis.root.yml"))?;
    file.write_all(
        format!(
            r#"
    env:
        greeting: "hello world"

    tasks:
        testing:
            cmds:
                - {p} {pms} {bf} hello world
                - {p} {pms} {bf} "hello world" hello
                - {p} {pms} {bf} "hello\" world" hello
                - {p} {pms} {bf} "{{{{args.0}}}} {{{{ kwargs.name.0 }}}}" "{{{{ env.greeting }}}}"
                - {p} {pms} {bf} {{{{ TASK.name }}}} "{{{{ FILE.env.greeting }}}}"
            "#,
            p = program,
            pms = param,
            bf = batch_file_name
        )
        .as_bytes(),
    )?;

    let mut cmd = Command::cargo_bin("yamis")?;
    cmd.current_dir(tmp_dir.path());
    cmd.arg("testing");
    cmd.arg("hi");
    cmd.arg("--name=world");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(format!(
            r#"[YAMIS] testing.cmds.0: {p} {pms} {bf} hello world
hello world
[YAMIS] testing.cmds.1: {p} {pms} {bf} "hello world" hello
hello world hello
[YAMIS] testing.cmds.2: {p} {pms} {bf} "hello\" world" hello
hello" world hello
[YAMIS] testing.cmds.3: {p} {pms} {bf} "hi world" "hello world"
hi world hello world
[YAMIS] testing.cmds.4: {p} {pms} {bf} testing "hello world"
testing hello world
"#,
            p = program,
            pms = param,
            bf = batch_file_name
        )));
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

    let mut file = File::create(tmp_dir.join("yamis.root.yml"))?;
    file.write_all(
        format!(
            r#"
    tasks:
        hello:
            program: "{}"
            args: {} {} {{{{args.0}}}}

        bye:
            script: "echo Bye {{{{args.1}}}}"

        greet:
            serial: ["hello", "bye"]
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
    let mut file = File::create(tmp_dir.join("yamis.root.yml"))?;
    file.write_all(
        r#"
tasks:
    hello_base:
        env:
            greeting: "hello world"

    calc_base:
        env:
            one_plus_one: "2"

    hello:
        bases: ["hello_base", "calc_base"]
        script: "echo $greeting, 1+1=$one_plus_one"

    hello.windows:
        bases: ["hello_base", "calc_base"]
        script: "echo %greeting%, 1+1=%one_plus_one%"
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
        ("echo_args.sh", "echo $*".as_bytes())
    };
    let mut batch_file = File::create(tmp_dir.join(batch_file_name))?;
    batch_file.write_all(batch_file_content).unwrap();

    let mut file = File::create(tmp_dir.join("yamis.root.yml"))?;
    file.write_all(
        format!(
            r#"
tasks:
  echo_program:
    program: "bash"
    args: "{b}"
    private: true

    windows:
      program: "cmd.exe"
      args: "/C {b}"

  hello:
    bases: ["echo_program"]
    args_extend: "hello world"

    windows:
      bases: ["echo_program"]
      args+: "hello world"
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
