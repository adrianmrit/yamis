use std::collections::HashMap;
use yamis::args_format::{format_script, EscapeMode, FormatError};

#[test]
fn test_format_string() {
    let string = "{1} {2} {a} {a?} {b} {c?} {hello_world} {{1}} {{{1}}} {{{{1}}}} {*}";
    let mut vars = HashMap::new();
    vars.insert(String::from("1"), vec![String::from("arg_1")]);
    vars.insert(String::from("2"), vec![String::from("arg_2")]);
    vars.insert(String::from("a"), vec![String::from("arg_a")]);
    vars.insert(String::from("b"), vec![String::from("arg_b")]);
    vars.insert(String::from("*"), vec![String::from("arg_*")]);
    vars.insert(
        String::from("hello_world"),
        vec![String::from("hello world")],
    );
    // optional not given
    // vars.insert("c".to_string(), "arg_c".to_string());

    let expected = "arg_1 arg_2 arg_a arg_a arg_b  \"hello world\" {1} {arg_1} {{1}} arg_*";
    assert_eq!(
        format_script(string, &vars, EscapeMode::OnSpace).unwrap(),
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
        format_script(string, &vars, EscapeMode::Never).unwrap(),
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
        format_script(string, &vars, EscapeMode::Never).unwrap(),
        expected
    );
}

#[test]
fn test_format_string_unclosed_tag() {
    let expected_err: Result<String, FormatError> =
        Err(FormatError::Invalid(String::from("Unclosed tag.")));
    let mut vars = HashMap::new();
    vars.insert(String::from("1"), vec![String::from("arg_1")]);
    vars.insert(String::from("2"), vec![String::from("arg_2")]);

    let string = "{1} {2 {1}";
    assert_eq!(
        format_script(string, &vars, EscapeMode::Always),
        expected_err
    );

    let string = "{1} {2} {1";
    assert_eq!(
        format_script(string, &vars, EscapeMode::Always),
        expected_err
    );
}

#[test]
fn test_format_string_unescaped_open_token() {
    let expected_err: Result<String, FormatError> =
        Err(FormatError::Invalid(String::from("Unescaped '{'.")));
    let mut vars = HashMap::new();
    vars.insert(String::from("1"), vec![String::from("arg_1")]);
    vars.insert(String::from("2"), vec![String::from("arg_2")]);

    let string = "{1} {2} {";
    assert_eq!(
        format_script(string, &vars, EscapeMode::Always),
        expected_err
    );
}

#[test]
fn test_format_string_unescaped_close_token() {
    let expected_err: Result<String, FormatError> =
        Err(FormatError::Invalid(String::from("Unescaped '}'.")));
    let mut vars = HashMap::new();
    vars.insert(String::from("1"), vec![String::from("arg_1")]);
    vars.insert(String::from("2"), vec![String::from("arg_2")]);

    let string = "}{1} {2}";
    assert_eq!(
        format_script(string, &vars, EscapeMode::Always),
        expected_err
    );
    let string = "{1} {2}}";
    assert_eq!(
        format_script(string, &vars, EscapeMode::Always),
        expected_err
    );
}

#[test]
fn test_format_string_invalid_arg() {
    let mut vars = HashMap::new();
    vars.insert(String::from("1"), vec![String::from("arg_1")]);
    vars.insert(String::from("2"), vec![String::from("arg_2")]);

    let string = "{1} {-2} {1}";
    assert_eq!(
        format_script(string, &vars, EscapeMode::Always),
        Err(FormatError::Invalid(String::from(
            "Invalid argument tag `{-2}`."
        )))
    );
    let string = "{1} {-} {1}";
    assert_eq!(
        format_script(string, &vars, EscapeMode::Always),
        Err(FormatError::Invalid(String::from(
            "Invalid argument tag `{-}`."
        )))
    );
    let string = "{1} { } {1}";
    assert_eq!(
        format_script(string, &vars, EscapeMode::Always),
        Err(FormatError::Invalid(String::from(
            "Invalid argument tag `{ }`."
        )))
    );
    let string = "{1} {_a} {1}";
    assert_eq!(
        format_script(string, &vars, EscapeMode::Always),
        Err(FormatError::Invalid(String::from(
            "Invalid argument tag `{_a}`."
        )))
    );
    let string = "{1} {-_a} {1}";
    assert_eq!(
        format_script(string, &vars, EscapeMode::Always),
        Err(FormatError::Invalid(String::from(
            "Invalid argument tag `{-_a}`."
        )))
    );
}
