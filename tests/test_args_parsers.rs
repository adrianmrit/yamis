use std::collections::HashMap;
use yamis::args::{
    format_string, FormatError, INVALID_ARG_CHAR_ERROR, UNCLOSED_TAG_ERROR,
    UNESCAPED_CLOSE_TOKEN_ERROR, UNESCAPED_OPEN_TOKEN_ERROR,
};

#[test]
fn test_format_string() {
    let string = "{1}{ 1} {2} {a} {a?} {b} {c?}{ c? } {hello_world} {{1}} {{{1}}} {{{{1}}}} {*}";
    let mut vars = HashMap::new();
    vars.insert("1".to_string(), "arg_1".to_string());
    vars.insert("2".to_string(), "arg_2".to_string());
    vars.insert("a".to_string(), "arg_a".to_string());
    vars.insert("b".to_string(), "arg_b".to_string());
    vars.insert("*".to_string(), "arg_*".to_string());
    vars.insert("hello_world".to_string(), "hello world".to_string());
    // optional not given
    // vars.insert("c".to_string(), "arg_c".to_string());

    let expected = "arg_1 arg_1 arg_2 arg_a arg_a arg_b  hello world {1} {arg_1} {{1}} arg_*";
    assert_eq!(format_string(&string, &vars).unwrap(), expected);
}

#[test]
fn test_format_string_unclosed_tag() {
    let expected_err: Result<String, FormatError> =
        Err(FormatError::Invalid(String::from(UNCLOSED_TAG_ERROR)));
    let mut vars = HashMap::new();
    vars.insert("1".to_string(), "arg_1".to_string());
    vars.insert("2".to_string(), "arg_2".to_string());

    let string = "{1} {2 {1}";
    assert_eq!(format_string(&string, &vars), expected_err);

    let string = "{1} {2} {1";
    assert_eq!(format_string(&string, &vars), expected_err);
}

#[test]
fn test_format_string_unescaped_open_token() {
    let expected_err: Result<String, FormatError> = Err(FormatError::Invalid(String::from(
        UNESCAPED_OPEN_TOKEN_ERROR,
    )));
    let mut vars = HashMap::new();
    vars.insert("1".to_string(), "arg_1".to_string());
    vars.insert("2".to_string(), "arg_2".to_string());

    let string = "{1} {2} {";
    assert_eq!(format_string(&string, &vars), expected_err);
}

#[test]
fn test_format_string_unescaped_close_token() {
    let expected_err: Result<String, FormatError> = Err(FormatError::Invalid(String::from(
        UNESCAPED_CLOSE_TOKEN_ERROR,
    )));
    let mut vars = HashMap::new();
    vars.insert("1".to_string(), "arg_1".to_string());
    vars.insert("2".to_string(), "arg_2".to_string());

    let string = "}{1} {2}";
    assert_eq!(format_string(&string, &vars), expected_err);
    let string = "{1} {2}}";
    assert_eq!(format_string(&string, &vars), expected_err);
}

#[test]
fn test_format_string_invalid_arg() {
    let mut vars = HashMap::new();
    let expected_err: Result<String, FormatError> =
        Err(FormatError::Invalid(String::from(INVALID_ARG_CHAR_ERROR)));
    vars.insert("1".to_string(), "arg_1".to_string());
    vars.insert("2".to_string(), "arg_2".to_string());

    let string = "{1} {-2} {1}";
    assert_eq!(format_string(&string, &vars), expected_err);
    let string = "{1} {-} {1}";
    assert_eq!(format_string(&string, &vars), expected_err);
    let string = "{1} { } {1}";
    assert_eq!(format_string(&string, &vars), expected_err);
    let string = "{1} {_a} {1}";
    assert_eq!(format_string(&string, &vars), expected_err);
    let string = "{1} {-_a} {1}";
    assert_eq!(format_string(&string, &vars), expected_err);
}
