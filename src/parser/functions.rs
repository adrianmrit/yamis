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

/// Validates the number of arguments for a function, but not their type
///
/// # Arguments
///
/// * `fn_name`: Name of the function to display in the error
/// * `args`: list of arguments
/// * `min`: min number of arguments
/// * `max`: max number of arguments
///
/// returns: Result<(), Box<dyn Error, Global>>
fn validate_arguments_length(
    fn_name: &str,
    args: &Vec<FunVal>,
    min: usize,
    max: usize,
) -> DynErrResult<()> {
    let len = args.len();
    if len < min {
        return if min == max {
            Err(format!(
                "{} requires {} arguments, but {} were given",
                fn_name, min, len
            )
            .into())
        } else {
            Err(format!(
                "{} requires at least {} arguments, but {} were given",
                fn_name, min, len
            )
            .into())
        };
    } else if len > max {
        return if max == min {
            Err(format!(
                "{} requires {} arguments, but {} were given",
                fn_name, max, len
            )
            .into())
        } else {
            Err(format!(
                "{} requires at most {} arguments, but {} were given",
                fn_name, max, len
            )
            .into())
        };
    }
    Ok(())
}

/// Validates that the argument at the given index is a string
///
/// # Arguments
///
/// * `args`: list of function arguments
/// * `index`: index of the argument
///
/// returns: Result<&str, Box<dyn Error, Global>>
fn validate_string<'a>(fn_name: &str, args: &'a [FunVal], index: usize) -> DynErrResult<&'a str> {
    match args[index] {
        FunVal::String(s) => Ok(s),
        FunVal::Vec(_) => Err(format!(
            "{} requires a string argument at index {}, but a list was given",
            fn_name, index
        )
        .into()),
    }
}

// Currently unused so raises a warning
// /// Validates that the argument at the given index is a list of strings
// ///
// /// # Arguments
// ///
// /// * `args`: list of function arguments
// /// * `index`: index of the argument
// ///
// /// returns: Result<&str, Box<dyn Error, Global>>
// fn validate_vec<'a>(
//     fn_name: &str,
//     args: &'a [FunVal],
//     index: usize,
// ) -> DynErrResult<&'a Vec<String>> {
//     match args[index] {
//         FunVal::String(_) => Err(format!(
//             "{} requires a list argument at index {}, but a string was given",
//             fn_name, index
//         )
//         .into()),
//         FunVal::Vec(l) => Ok(l),
//     }
// }

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
    match format_string(fmt_string, &[val]) {
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
    let fn_name = "map";
    validate_arguments_length(fn_name, args, 2, 2)?;
    let fmt_string = validate_string(fn_name, args, 0)?;

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

/// Like calling map and then joining the values with the empty string
///
/// # Arguments
///
/// * `args`: Function values
///
/// returns: Result<FunResult, Box<dyn Error, Global>>
fn jmap(args: &Vec<FunVal>) -> DynErrResult<FunResult> {
    let fn_name = "jmap";
    validate_arguments_length(fn_name, args, 2, 2)?;
    let fmt_string = validate_string(fn_name, args, 0)?;

    return match args.index(1) {
        FunVal::String(s) => {
            let result = map_format_string(fmt_string, s)?;
            Ok(FunResult::String(result))
        }
        FunVal::Vec(values) => {
            let mut result = String::with_capacity(values.capacity() * 5);
            for s in *values {
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
    let fn_name = "join";
    validate_arguments_length(fn_name, args, 2, 2)?;
    let join_val = validate_string(fn_name, args, 0)?;

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
    let fn_name = "fmt";
    validate_arguments_length(fn_name, args, 2, usize::MAX)?;
    let fmt_string = validate_string(fn_name, args, 0)?;
    let mut values: Vec<&str> = Vec::with_capacity(args.len() - 1);
    let mut i = 1;
    while i < args.len() {
        let arg = validate_string(fn_name, args, i)?;
        values.push(arg);
        i += 1;
    }
    Ok(FunResult::String(format_string(fmt_string, &values)?))
}

/// Split the string into multiple values. The first argument is the string to split by, and the
/// second is the string to split. If you want to split multiple passed values, then will need
/// to join them first and then split.
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
///
/// let vars = vec![FunVal::String(" and "), FunVal::Vec(&values)];
/// ```
fn split(args: &Vec<FunVal>) -> DynErrResult<FunResult> {
    let fn_name = "split";
    validate_arguments_length(fn_name, args, 2, 2)?;
    let split_val = validate_string(fn_name, args, 0)?;
    let split_string = validate_string(fn_name, args, 1)?;
    Ok(FunResult::Vec(
        split_string
            .split(split_val)
            .map(|s| s.to_string())
            .collect(),
    ))
}

/// Removes leading and trailing whitespaces (including newlines) from the string or each string
/// in list of strings.
///
/// # Arguments
///
/// * `args`: Function values
///
/// returns: Result<FunResult, Box<dyn Error, Global>>
fn trim(args: &Vec<FunVal>) -> DynErrResult<FunResult> {
    let fn_name = "trim";
    validate_arguments_length(fn_name, args, 1, 1)?;
    match args.index(0) {
        FunVal::String(s) => Ok(FunResult::String(s.trim().to_string())),
        FunVal::Vec(values) => {
            let mut result = Vec::with_capacity(values.capacity());
            for s in *values {
                result.push(s.trim().to_string());
            }
            Ok(FunResult::Vec(result))
        }
    }
}

/// Returns a FunctionRegistry with the default functions
fn load_default_functions() -> FunctionRegistry {
    let mut functions: HashMap<String, Function> = HashMap::new();
    functions.insert(String::from("map"), map);
    functions.insert(String::from("flat"), jmap);
    functions.insert(String::from("join"), join);
    functions.insert(String::from("fmt"), fmt);
    functions.insert(String::from("split"), split);
    functions.insert(String::from("trim"), trim);
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
fn test_jmap() {
    let vars = vec![
        FunVal::String("Hello {} ! ? {{ }}"),
        FunVal::String("world"),
    ];
    let result = jmap(&vars).unwrap();
    let expected = FunResult::String(String::from("Hello world ! ? { }"));
    assert_eq!(result, expected);

    let values = vec!["world".to_string(), "people".to_string()];
    let vars = vec![FunVal::String("Hello {}, "), FunVal::Vec(&values)];
    let result = jmap(&vars).unwrap();
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

#[test]
fn test_split() {
    let vars = vec![FunVal::String(","), FunVal::String("world,people")];
    let result = split(&vars).unwrap();
    let expected = FunResult::Vec(vec!["world".to_string(), "people".to_string()]);
    assert_eq!(result, expected);
}

#[test]
fn test_trim() {
    let vars = vec![FunVal::String(" world ")];
    let result = trim(&vars).unwrap();
    let expected = FunResult::String(String::from("world"));
    assert_eq!(result, expected);

    let values = vec![" world ".to_string(), " people ".to_string()];
    let vars = vec![FunVal::Vec(&values)];
    let result = trim(&vars).unwrap();
    let expected = FunResult::Vec(vec!["world".to_string(), "people".to_string()]);
    assert_eq!(result, expected);
}
