use std::collections::HashMap;
use std::env;
use yamis::args_format::{format_arg, format_script, EscapeMode, FormatError};

fn empty_env() -> HashMap<String, String> {
    HashMap::new()
}

#[test]
fn test_format_string() {
    let string = "{1} {2} {a} {a?} {b}{c?} {hello_world} {{1}} {{{1}}} {{{{1}}}} {*}";
    let mut vars = HashMap::new();
    vars.insert(String::from("a"), vec![String::from("arg_a")]);
    vars.insert(String::from("b"), vec![String::from("arg_b")]);
    vars.insert(
        String::from("*"),
        vec![
            String::from("arg_1"),
            String::from("arg_2"),
            String::from("arg_a"),
            String::from("arg_b"),
            String::from("hello world"),
        ],
    );
    vars.insert(
        String::from("hello_world"),
        vec![String::from("hello world")],
    );
    // optional not given
    // vars.insert("c".to_string(), "arg_c".to_string());

    let expected = "arg_1 arg_2 arg_a arg_a arg_b hello world {1} {arg_1} {{1}} arg_1 arg_2 arg_a arg_b hello world";
    assert_eq!(
        format_script(string, &vars, &empty_env(), &EscapeMode::Never).unwrap(),
        expected
    );
}

#[test]
fn test_format_string_multiple_values() {
    let string = "{v} {*}";

    let mut vars = HashMap::new();

    vars.insert(
        String::from("v"),
        vec![String::from("arg_1"), String::from("arg_2")],
    );
    vars.insert(
        String::from("*"),
        vec![String::from("--v=arg_1"), String::from("--v=arg_2")],
    );

    let expected = "arg_1 arg_2 --v=arg_1 --v=arg_2";
    assert_eq!(
        format_script(string, &vars, &empty_env(), &EscapeMode::Never).unwrap(),
        expected
    );
}

#[test]
fn test_format_string_prefix_suffix() {
    let string = "{(-f )v?(.txt)} {(--v=)v}{( -invalid=)not_given?()}";

    let mut vars = HashMap::new();

    vars.insert(
        String::from("v"),
        vec![String::from("arg_1"), String::from("arg_2")],
    );
    vars.insert(
        String::from("*"),
        vec![String::from("--v=arg_1"), String::from("--v=arg_2")],
    );

    let expected = "-f arg_1.txt -f arg_2.txt --v=arg_1 --v=arg_2";
    assert_eq!(
        format_script(string, &vars, &empty_env(), &EscapeMode::Never).unwrap(),
        expected
    );
}

#[test]
fn test_format_string_unclosed_tag() {
    let expected_err: Result<String, FormatError> =
        Err(FormatError::Invalid(String::from("Unclosed tag.")));
    let mut vars = HashMap::new();
    vars.insert(
        String::from("*"),
        vec![String::from("arg_1"), String::from("arg_2")],
    );

    let string = "{1} {2 {1}";
    assert_eq!(
        format_script(string, &vars, &empty_env(), &EscapeMode::Always),
        expected_err
    );

    let string = "{1} {2} {1";
    assert_eq!(
        format_script(string, &vars, &empty_env(), &EscapeMode::Always),
        expected_err
    );
}

#[test]
fn test_format_string_unescaped_open_token() {
    let expected_err: Result<String, FormatError> =
        Err(FormatError::Invalid(String::from("Unescaped '{'.")));
    let mut vars = HashMap::new();
    vars.insert(
        String::from("*"),
        vec![String::from("arg_1"), String::from("arg_2")],
    );

    let string = "{1} {2} {";
    assert_eq!(
        format_script(string, &vars, &empty_env(), &EscapeMode::Always),
        expected_err
    );
}

#[test]
fn test_format_string_unescaped_close_token() {
    let expected_err: Result<String, FormatError> =
        Err(FormatError::Invalid(String::from("Unescaped '}'.")));
    let mut vars = HashMap::new();
    vars.insert(
        String::from("*"),
        vec![String::from("arg_1"), String::from("2")],
    );

    let string = "}{1} {2}";
    assert_eq!(
        format_script(string, &vars, &empty_env(), &EscapeMode::Always),
        expected_err
    );
    let string = "{1} {2}}";
    assert_eq!(
        format_script(string, &vars, &empty_env(), &EscapeMode::Always),
        expected_err
    );
}

#[test]
fn test_format_string_invalid_arg() {
    let mut vars = HashMap::new();
    vars.insert(
        String::from("*"),
        vec![String::from("arg_2"), String::from("arg_1")],
    );

    let string = "{1} {-2} {1}";
    assert_eq!(
        format_script(string, &vars, &empty_env(), &EscapeMode::Always),
        Err(FormatError::Invalid(String::from(
            "Invalid argument tag `{-2}`."
        )))
    );

    let string = "{1} {-} {1}";
    assert_eq!(
        format_script(string, &vars, &empty_env(), &EscapeMode::Always),
        Err(FormatError::Invalid(String::from(
            "Invalid argument tag `{-}`."
        )))
    );

    let string = "{1} { } {1}";
    assert_eq!(
        format_script(string, &vars, &empty_env(), &EscapeMode::Always),
        Err(FormatError::Invalid(String::from(
            "Invalid argument tag `{ }`."
        )))
    );

    let string = "{1} {_a} {1}";
    assert_eq!(
        format_script(string, &vars, &empty_env(), &EscapeMode::Always),
        Err(FormatError::Invalid(String::from(
            "Invalid argument tag `{_a}`."
        )))
    );

    let string = "{1} {-_a} {1}";
    assert_eq!(
        format_script(string, &vars, &empty_env(), &EscapeMode::Always),
        Err(FormatError::Invalid(String::from(
            "Invalid argument tag `{-_a}`."
        )))
    );
}

#[test]
fn test_format_string_env() {
    let vars = HashMap::<String, Vec<String>>::new();
    let mut env = HashMap::new();

    env.insert(
        String::from("TEST_ENV_VARIABLE"),
        String::from("sample_val"),
    );

    env.insert(
        String::from("TEST_ENV_VARIABLE_2"),
        String::from("sample_val_2"),
    );

    env::set_var("TEST_ENV_VARIABLE_3", "sample_val_3");

    let string = "--f={$TEST_ENV_VARIABLE(.txt)} ({$MISSING_ENV_VARIABLE?}) {$TEST_ENV_VARIABLE_2} {$TEST_ENV_VARIABLE_3}";

    let expected = "--f=sample_val.txt () sample_val_2 sample_val_3";
    assert_eq!(
        format_script(string, &vars, &env, &EscapeMode::Never).unwrap(),
        expected
    );
}

#[test]
fn test_format_arg() {
    let mut vars = HashMap::new();

    vars.insert(
        String::from("v"),
        vec![String::from("arg_1"), String::from("arg_2")],
    );
    vars.insert(
        String::from("*"),
        vec![
            String::from("--v=arg_1"),
            String::from("--v=arg_2"),
            String::from("arg_3"),
        ],
    );

    let string = "--f={3}";
    let expected = "--f=arg_3";
    let actual = format_arg(string, &vars, &empty_env()).unwrap();
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0], expected);

    let string = "{v}";
    let expected = vec!["arg_1", "arg_2"];
    let actual = format_arg(string, &vars, &empty_env()).unwrap();
    assert_eq!(actual, expected);

    let string = "{4?}";
    let actual = format_arg(string, &vars, &empty_env()).unwrap();
    assert_eq!(actual, Vec::<String>::new());

    let string = "";
    let actual = format_arg(string, &vars, &empty_env()).unwrap();
    assert_eq!(actual, Vec::<String>::new());

    let string = "--{(f=)v}.txt";
    let expected = vec!["--f=arg_1.txt", "--f=arg_2.txt"];
    let actual = format_arg(string, &vars, &empty_env()).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn test_format_arg_invalid() {
    let mut vars = HashMap::new();
    vars.insert(
        String::from("*"),
        vec![String::from("arg_2"), String::from("arg_1")],
    );

    let string = "{1}{2}";
    assert_eq!(
        format_arg(string, &vars, &empty_env()),
        Err(FormatError::Invalid(String::from(
            "Arguments of commands can only have an argument tag."
        )))
    );
    let string = "{1}{1}";
    assert_eq!(
        format_arg(string, &vars, &empty_env()),
        Err(FormatError::Invalid(String::from(
            "Arguments of commands can only have an argument tag."
        )))
    );
    let string = "{1} {2}";
    assert_eq!(
        format_arg(string, &vars, &empty_env()),
        Err(FormatError::Invalid(String::from(
            "Arguments of commands can only have an argument tag."
        )))
    );
    let string = "{1}{2}{3}";
    assert_eq!(
        format_arg(string, &vars, &empty_env()),
        Err(FormatError::Invalid(String::from(
            "Arguments of commands can only have an argument tag."
        )))
    );
}

#[test]
fn test_format_arg_env() {
    let vars = HashMap::<String, Vec<String>>::new();
    let mut env = HashMap::new();

    env.insert(
        String::from("TEST_ENV_VARIABLE"),
        String::from("sample_val"),
    );

    let string = "--f={$TEST_ENV_VARIABLE}";
    let expected = vec!["--f=sample_val"];
    let actual = format_arg(string, &vars, &env).unwrap();
    assert_eq!(actual, expected);

    let string = "{$TEST_ENV_VARIABLE}";
    let expected = vec!["sample_val"];
    let actual = format_arg(string, &vars, &env).unwrap();
    assert_eq!(actual, expected);

    let string = "{$MISSING_ENV_VARIABLE?}";
    let actual = format_arg(string, &vars, &env).unwrap();
    assert_eq!(actual, Vec::<String>::new());

    env::set_var("NON_MISSING_ENV_VARIABLE", "value_non_missing_env_var");

    let string = "{$NON_MISSING_ENV_VARIABLE}";
    let expected = vec!["value_non_missing_env_var"];
    let actual = format_arg(string, &vars, &env).unwrap();
    assert_eq!(actual, expected);
}
