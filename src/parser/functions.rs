use std::collections::HashMap;
use std::ops::Index;

use lazy_static::lazy_static;

use crate::format_str::format_string;
use crate::types::DynErrResult;

/// Wraps a value passed to a function, which can be either a str pointer or pointer to a
/// Vec of Strings
#[derive(PartialEq, Eq, Debug)]
pub enum FunVal<'a> {
    String(&'a str),
    Vec(&'a Vec<String>),
}

/// Wraps a function result, which can be either a String or Vec of Strings.
#[derive(PartialEq, Eq, Debug)]
pub enum FunResult {
    String(String),
    Vec(Vec<String>),
}

impl FunResult {
    /// Converts the result to a value
    pub(crate) fn as_val(&self) -> FunVal {
        match self {
            FunResult::String(val) => FunVal::String(val),
            FunResult::Vec(val) => FunVal::Vec(val),
        }
    }
}

/// Signature that functions must follow
type Function = fn(&Vec<FunVal>) -> DynErrResult<FunResult>;

/// Maps name to function pointers, where all the functions must follow
/// [Function] signature
pub struct FunctionRegistry {
    /// Hashmap of functions
    pub(crate) functions: HashMap<String, Function>,
}

/// Used by [map] to format a single string value
fn map_format_string(fmt_string: &str, val: &str) -> DynErrResult<String> {
    match format_string(fmt_string, &vec![val]) {
        Ok(val) => Ok(val),
        Err(e) => Err(format!("Error formatting the string:\n{e}").into()),
    }
}

/// Formats one or multiple values, returning one or multiple values.
///
/// # Arguments
///
/// * `args`: Function values
///
/// returns: Result<FunResult, Box<dyn Error, Global>>
///
/// # Examples
///
/// ```ignore
/// let values = vec!["world".to_string(), "people".to_string()];
/// let vars = vec![FunVal::String("Hello {} ! ? {{ }}"), FunVal::Vec(&values)];
/// let result = map(&vars).unwrap();
/// let expected = FunResult::Vec(vec![
///     "Hello world ! ? { }".to_string(),
///     "Hello people ! ? { }".to_string(),
/// ]);
/// assert_eq!(result, expected);
///
/// let vars = vec![
///     FunVal::String("Hello {} ! ? {{ }}"),
///     FunVal::String("world"),
/// ];
/// let result = map(&vars).unwrap();
/// let expected = FunResult::String(String::from("Hello world ! ? { }"));
/// assert_eq!(result, expected);
/// ```
fn map(args: &Vec<FunVal>) -> DynErrResult<FunResult> {
    if args.len() != 2 {
        return Err("map takes exactly two arguments".into());
    }
    let fmt_string = match args.index(0) {
        FunVal::String(s) => s,
        FunVal::Vec(_) => return Err("The first argument of map should be a string".into()),
    };
    return match args.index(1) {
        FunVal::String(s) => {
            let result = map_format_string(fmt_string, s)?;
            Ok(FunResult::String(result))
        } // TODO: format and return only this one
        FunVal::Vec(l) => {
            let mut result = Vec::with_capacity(l.capacity());
            for s in *l {
                result.push(map_format_string(fmt_string, s)?);
            }
            Ok(FunResult::Vec(result))
        }
    };
}

/// Similar to map, but the result is always a string
///
/// # Arguments
///
/// * `args`: Function values
///
/// returns: Result<FunResult, Box<dyn Error, Global>>
///
/// # Examples
///
fn flat(args: &Vec<FunVal>) -> DynErrResult<FunResult> {
    if args.len() != 2 {
        return Err("flat takes exactly two arguments".into());
    }
    let fmt_string = match args.index(0) {
        FunVal::String(s) => s,
        FunVal::Vec(_) => return Err("The first argument of flat should be a string".into()),
    };
    return match args.index(1) {
        FunVal::String(s) => {
            let result = map_format_string(fmt_string, s)?;
            Ok(FunResult::String(result))
        } // TODO: format and return only this one
        FunVal::Vec(l) => {
            let mut result = String::with_capacity(l.capacity() * 5);
            for s in *l {
                result.push_str(&map_format_string(fmt_string, s)?);
            }
            Ok(FunResult::String(result))
        }
    };
}

/// Returns a FunctionRegistry with the default functions
fn load_default_functions() -> FunctionRegistry {
    let mut functions: HashMap<String, Function> = HashMap::new();
    functions.insert(String::from("map"), map);
    functions.insert(String::from("flat"), flat);
    FunctionRegistry { functions }
}

lazy_static! {
    /// Instance of [FunctionRegistry] holding the default functions
    pub static ref DEFAULT_FUNCTIONS: FunctionRegistry = load_default_functions();
}

#[test]
fn test_map() {
    let vars = vec![
        FunVal::String("Hello {} ! ? {{ }}"),
        FunVal::String("world"),
    ];
    let result = map(&vars).unwrap();
    let expected = FunResult::String(String::from("Hello world ! ? { }"));
    assert_eq!(result, expected);

    let values = vec!["world".to_string(), "people".to_string()];
    let vars = vec![FunVal::String("Hello {} ! ? {{ }}"), FunVal::Vec(&values)];
    let result = map(&vars).unwrap();
    let expected = FunResult::Vec(vec![
        "Hello world ! ? { }".to_string(),
        "Hello people ! ? { }".to_string(),
    ]);
    assert_eq!(result, expected);

    let values = vec!["world".to_string(), "people".to_string()];
    let vars = vec![FunVal::String("Hello { ! ? {{ }}"), FunVal::Vec(&values)];
    let result = map(&vars).unwrap_err().to_string();
    let expected_result = r#"Error formatting the string:
 --> 1:7
  |
1 | Hello { ! ? {{ }}
  |       ^---
  |
  = expected EOI, literal, or tag"#;
    assert_eq!(result, expected_result);
}

#[test]
fn test_flat() {
    let vars = vec![
        FunVal::String("Hello {} ! ? {{ }}"),
        FunVal::String("world"),
    ];
    let result = flat(&vars).unwrap();
    let expected = FunResult::String(String::from("Hello world ! ? { }"));
    assert_eq!(result, expected);

    let values = vec!["world".to_string(), "people".to_string()];
    let vars = vec![FunVal::String("Hello {}, "), FunVal::Vec(&values)];
    let result = flat(&vars).unwrap();
    let expected = FunResult::String(String::from("Hello world, Hello people, "));
    assert_eq!(result, expected);

    let values = vec!["world".to_string(), "people".to_string()];
    let vars = vec![FunVal::String("Hello { ! ? {{ }}"), FunVal::Vec(&values)];
    let result = map(&vars).unwrap_err().to_string();
    let expected_result = r#"Error formatting the string:
 --> 1:7
  |
1 | Hello { ! ? {{ }}
  |       ^---
  |
  = expected EOI, literal, or tag"#;
    assert_eq!(result, expected_result);
}
