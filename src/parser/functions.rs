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

    pub fn is_empty(&self) -> bool {
        match self {
            FunResult::String(val) => val.is_empty(),
            FunResult::Vec(val) => val.is_empty(),
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

/// Joins multiple values.
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
/// let vars = vec![FunVal::String(" and "), FunVal::Vec(&values)];
/// let result = map(&vars).unwrap();
/// let expected = FunResult::String("world and people".to_string());
/// assert_eq!(result, expected);
/// ```
fn join(args: &Vec<FunVal>) -> DynErrResult<FunResult> {
    if args.len() != 2 {
        return Err("join takes exactly two arguments".into());
    }
    let join_val = match args.index(0) {
        FunVal::String(s) => s,
        FunVal::Vec(_) => return Err("The first argument of join should be a string".into()),
    };
    match args.index(1) {
        FunVal::String(s) => Ok(FunResult::String(s.to_string())),
        FunVal::Vec(values) => {
            if values.is_empty() {
                Ok(FunResult::String(String::new()))
            } else if values.len() == 1 {
                Ok(FunResult::String(values.first().unwrap().clone()))
            } else {
                let mut result = String::with_capacity(values.capacity() * 5);

                for val in &values[0..values.len() - 1] {
                    result.push_str(val);
                    result.push_str(join_val);
                }

                result.push_str(&values[values.len() - 1]);
                Ok(FunResult::String(result))
            }
        }
    }
}

/// Formats teh string
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
/// let vars = vec![FunVal::String(" and "), FunVal::Vec(&values)];
/// let result = map(&vars).unwrap();
/// let expected = FunResult::String("world and people".to_string());
/// assert_eq!(result, expected);
/// ```
fn fmt(args: &Vec<FunVal>) -> DynErrResult<FunResult> {
    if args.len() < 2 {
        return Err("fmt takes at least two arguments".into());
    }
    let mut args_iter = args.iter();
    let fmt_string = match args_iter.next().unwrap() {
        FunVal::String(s) => s,
        FunVal::Vec(_) => return Err("The first argument of fmt should be a string".into()),
    };
    let mut values: Vec<&str> = Vec::with_capacity(args.len() - 1);
    for (i, arg) in args_iter.enumerate() {
        match arg {
            FunVal::String(s) => values.push(*s),
            FunVal::Vec(_) => {
                return Err(format!("fmt got multiple values at argument at position {i}").into())
            }
        }
    }
    Ok(FunResult::String(format_string(fmt_string, &values)?))
}

/// Returns a FunctionRegistry with the default functions
fn load_default_functions() -> FunctionRegistry {
    let mut functions: HashMap<String, Function> = HashMap::new();
    functions.insert(String::from("map"), map);
    functions.insert(String::from("flat"), flat);
    functions.insert(String::from("join"), join);
    functions.insert(String::from("fmt"), fmt);
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

#[test]
fn test_join() {
    let values = vec!["world".to_string(), "people".to_string()];
    let vars = vec![FunVal::String(", "), FunVal::Vec(&values)];
    let result = join(&vars).unwrap();
    let expected = FunResult::String(String::from("world, people"));
    assert_eq!(result, expected);

    let vars = vec![FunVal::String(","), FunVal::String("world")];
    let result = join(&vars).unwrap();
    let expected = FunResult::String(String::from("world"));
    assert_eq!(result, expected);
}

#[test]
fn test_fmt() {
    let vars = vec![
        FunVal::String("Hello {} and {}"),
        FunVal::String("world"),
        FunVal::String("people"),
    ];
    let result = fmt(&vars).unwrap();
    let expected = FunResult::String(String::from("Hello world and people"));
    assert_eq!(result, expected);
}
