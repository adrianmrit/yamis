use crate::parser::functions::{FunResult, DEFAULT_FUNCTIONS};
use crate::types::{DynErrResult, TaskArgs};
use pest::error::{Error as PestError, ErrorVariant};
use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;
use serde_derive::Deserialize;
use std::cmp::{max, min};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::{error, fmt};

mod functions;

/// Modes to escape (add quotes) the arguments passed to the script
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EscapeMode {
    /// Always quote the arguments
    Always,
    /// Only add quotes if the argument has spaces
    Spaces,
    /// Never quote the argument
    Never,
}

/// Represents the slice from the user, either by index or range
enum Slice {
    Index(isize),
    Range(Option<isize>, Option<isize>),
}

/// Represents the actual slice after the indexes are resolved correctly
enum RealSlice {
    Index(usize),
    Range(usize, usize),
}

/// Error raised when there is an error parsing an integer
#[derive(Debug)]
struct IntParsingError(String);

impl Display for IntParsingError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Error parsing `{}` as an integer", self.0)
    }
}

impl error::Error for IntParsingError {
    fn description(&self) -> &str {
        "bad config file"
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        None
    }
}

/// Returns a custom error for the given span and message
fn custom_span_error(span: pest::Span, msg: String) -> PestError<Rule> {
    PestError::new_from_span(ErrorVariant::CustomError { message: msg }, span)
}

// fn custom_pos_error(pos: pest::Position, msg: String) -> PestError<Rule> {
//     PestError::new_from_pos(ErrorVariant::CustomError { message: msg }, pos)
// }

/// Pest parser for script
#[derive(Parser)]
#[grammar = "parser/grammar.pest"]
struct ScriptParser;

/// Renames the rules for better error messages
fn rename_rules(rule: &Rule) -> String {
    match rule {
        Rule::WHITESPACE => "whitespace".to_string(),
        Rule::RANGE_SEPARATOR => "..".to_string(),
        Rule::digits => "integer".to_string(),
        Rule::optional => "?".to_string(),
        Rule::index => "integer".to_string(),
        Rule::range_from => "integer".to_string(),
        Rule::range_to => "integer".to_string(),
        Rule::range => "range".to_string(),
        Rule::slice => "slice".to_string(),
        Rule::arg => "positional argument".to_string(),
        Rule::all_args => "$@".to_string(),
        Rule::kwarg_name => "keyword argument".to_string(),
        Rule::kwarg => "keyword argument".to_string(),
        Rule::env_var_name => "environment variable name".to_string(),
        Rule::env_var => "environment variable".to_string(),
        Rule::fun_name => "function identifier".to_string(),
        Rule::expression_inner => "expression".to_string(),
        Rule::expression => "expression".to_string(),
        Rule::fun_params => "function parameters".to_string(),
        Rule::fun => "function".to_string(),
        Rule::tag => "tag".to_string(),
        Rule::special_val => "valid escaped character".to_string(),
        Rule::escape => "valid escaped value".to_string(),
        // Rule::escape_dq => "valid escaped value".to_string(),
        // Rule::escape_sq => "valid escaped value".to_string(),
        Rule::string_content => "string".to_string(),
        Rule::string => "string".to_string(),
        Rule::esc_ob => "{{".to_string(),
        Rule::esc_cb => "{{".to_string(),
        Rule::literal_content => "literal".to_string(),
        Rule::literal => "literal".to_string(),
        Rule::comment => "comment".to_string(),
        Rule::all => "comment, tag or literal".to_string(),
        Rule::task_arg => "tag or literal".to_string(),
        __other__ => format!("{:?}", __other__),
    }
}

/// Parses a string into an integer
fn parse_int(s: &str) -> Result<isize, IntParsingError> {
    s.parse::<isize>()
        .map_err(|_| IntParsingError(s.to_string()))
}

/// Returns a `Slice` enum from the pair
fn get_slice_repr(slice: Pair<Rule>) -> DynErrResult<Slice> {
    let mut slice_inner = slice.into_inner();
    let val = slice_inner.next().unwrap();
    match val.as_rule() {
        Rule::index => Ok(Slice::Index(parse_int(val.as_str())?)),
        Rule::range => {
            let mut from = None;
            let mut to = None;
            let val_inner = val.into_inner();
            for val in val_inner {
                match val.as_rule() {
                    Rule::range_from => from = Some(parse_int(val.as_str())?),
                    Rule::range_to => to = Some(parse_int(val.as_str())?),
                    v => panic!("Unexpected rule {:?}", v),
                }
            }
            Ok(Slice::Range(from, to))
        }
        v => panic!("Unexpected rule {:?}", v),
    }
}

/// Slices a string
fn slice_string(val: String, slice: RealSlice) -> FunResult {
    match slice {
        RealSlice::Index(i) => {
            if i >= val.len() {
                return FunResult::String("".to_string());
            }
            FunResult::String(val.chars().nth(i).unwrap().to_string())
        }
        RealSlice::Range(from, to) => {
            if from >= val.len() || from >= to {
                return FunResult::String("".to_string());
            }
            FunResult::String(String::from(val.get(from..to).unwrap_or("")))
        }
    }
}

/// Slices a vector
fn slice_vec(mut val: Vec<String>, slice: RealSlice) -> FunResult {
    match slice {
        RealSlice::Index(i) => FunResult::String(String::from(&val[i])),
        RealSlice::Range(from, to) => {
            if from >= val.len() || from >= to {
                FunResult::Vec(vec![])
            } else {
                let result = val.drain(from..to).collect();
                FunResult::Vec(result)
            }
        }
    }
}

/// Slices a value
fn slice_val(val: FunResult, slice: RealSlice) -> FunResult {
    match val {
        FunResult::String(v) => slice_string(v, slice),
        FunResult::Vec(v) => slice_vec(v, slice),
    }
}

/// Parses the inner value of a expression, excluding immediate following slices and modifiers
fn parse_expression_inner(
    expression_inner: Pair<Rule>,
    cli_args: &TaskArgs,
    env: &HashMap<String, String>,
) -> DynErrResult<FunResult> {
    let mut expression_inner = expression_inner.into_inner();
    let param = expression_inner.next().unwrap();
    match param.as_rule() {
        Rule::fun => parse_fun(param, cli_args, env),
        Rule::arg => parse_arg(param, cli_args),
        Rule::kwarg => parse_kwargs(param, cli_args),
        Rule::all_args => parse_all(cli_args),
        Rule::env_var => parse_env_var(param, env),
        Rule::string => parse_string(param),
        v => panic!("Unexpected rule {:?}", v),
    }
}

fn parse_slice(expression: Pair<Rule>, val: FunResult, optional: bool) -> DynErrResult<FunResult> {
    let val_len = match val {
        FunResult::String(ref v) => v.len(),
        FunResult::Vec(ref v) => v.len(),
    } as isize;
    // usize is 2^32 in 32 bit systems, or 2^64 in 64 bit systems
    // i32 is between -2^31 and 2^31-1, which are much smaller than usize.
    // So we can safely cast to i32
    // There are still edge cases if the slice is larger than 2^31, but that's very unlikely
    let span = expression.as_span();
    let slice = get_slice_repr(expression)?;
    match slice {
        Slice::Index(i) => {
            let real_index = if i < 0 { val_len + i } else { i };
            if real_index >= val_len || real_index < 0 {
                if !optional {
                    Err(custom_span_error(
                        span,
                        String::from("Index out of bounds for mandatory expression"),
                    )
                    .into())
                } else {
                    Ok(FunResult::String("".to_string()))
                }
            } else {
                Ok(slice_val(val, RealSlice::Index(real_index as usize)))
            }
        }
        Slice::Range(from, to) => {
            let from = from.unwrap_or(0);
            let to = min(to.unwrap_or(val_len), val_len);
            let real_from = if from < 0 { val_len + from } else { from };
            let real_to = if to < 0 { val_len + to } else { to };
            if real_from >= val_len || real_from < 0 || real_from > real_to {
                if !optional {
                    Err(custom_span_error(
                        span,
                        String::from("Range out of bounds for mandatory expression"),
                    )
                    .into())
                } else {
                    Ok(FunResult::Vec(vec![]))
                }
            } else {
                Ok(slice_val(
                    val,
                    RealSlice::Range(real_from as usize, max(real_to, 0) as usize),
                ))
            }
        }
    }
}

/// Parses an expression
fn parse_expression(
    expression: Pair<Rule>,
    cli_args: &TaskArgs,
    env: &HashMap<String, String>,
) -> DynErrResult<FunResult> {
    // We need to get the string representation even if there is no error because into_inner
    // consumes the pair, making it impossible (at least that I know of) to get the
    // representation later.
    let expression_copy = expression.clone();
    let mut expression_inner_values = expression.into_inner();
    let expression_inner = expression_inner_values.next().unwrap();
    let span = expression_inner.as_span();
    let mut val = match expression_inner.as_rule() {
        Rule::expression_inner => parse_expression_inner(expression_inner, cli_args, env)?,
        v => panic!("Unexpected rule {:?}", v),
    };
    // We check if it is optional first so that we can return the appropriate error message
    let optional = match expression_copy.into_inner().last() {
        Some(v) => v.as_rule() == Rule::optional,
        None => false,
    };
    for slice_or_modifier in expression_inner_values {
        match slice_or_modifier.as_rule() {
            Rule::slice => {
                val = parse_slice(slice_or_modifier, val, optional)?;
            }
            Rule::optional => (), // we already checked if it is optional
            v => panic!("Unexpected rule {:?}", v),
        }
    }
    if !optional && val.is_empty() {
        Err(custom_span_error(
            span,
            String::from("Mandatory expression did not return a value"),
        )
        .into())
    } else {
        Ok(val)
    }
}

/// Parses a function
fn parse_fun(
    function_pair: Pair<Rule>,
    cli_args: &TaskArgs,
    env: &HashMap<String, String>,
) -> DynErrResult<FunResult> {
    let function_span = function_pair.as_span();
    let mut function_inner = function_pair.into_inner();
    let fun_name_pair = function_inner.next().unwrap();
    let fun_name = fun_name_pair.as_str();
    let arguments = function_inner.next();
    let fun = match DEFAULT_FUNCTIONS.functions.get(fun_name) {
        None => {
            return Err(custom_span_error(
                fun_name_pair.as_span(),
                format!("Undefined function `{}`", fun_name_pair.as_str()),
            )
            .into())
        }
        Some(fun) => fun,
    };

    let arguments: Vec<FunResult> = match arguments {
        None => {
            vec![]
        }
        Some(arguments) => {
            let mut arguments_list: Vec<FunResult> = vec![];
            for param in arguments.into_inner() {
                let param = parse_expression(param, cli_args, env)?;
                arguments_list.push(param);
            }
            arguments_list
        }
    };
    match fun(&arguments.iter().map(|v| v.as_val()).collect()) {
        Ok(v) => Ok(v),
        Err(e) => Err(custom_span_error(
            function_span,
            format!("Error running function `{}`: {}", fun_name, e),
        )
        .into()),
    }
}

/// Parses a string
fn parse_string(tag: Pair<Rule>) -> DynErrResult<FunResult> {
    let tag_inner = tag.into_inner();
    let mut result = String::new();
    for pair in tag_inner {
        match pair.as_rule() {
            Rule::string_content => result.push_str(pair.as_str()),
            Rule::escape => {
                let mut inner = pair.into_inner();
                let val = inner.next().unwrap();
                match val.as_str() {
                    "n" => result.push('\n'),
                    "r" => result.push('\r'),
                    "t" => result.push('\t'),
                    "\\" => result.push('\\'),
                    "0" => result.push('\0'),
                    "\"" => result.push('"'),
                    "'" => result.push('\''),
                    v => {
                        panic!("Unexpected escaped value {}", v)
                    }
                }
                if let Some(other) = inner.next() {
                    panic!("Unexpected pair {:?}", other)
                }
            }
            v => panic!("Unexpected rule {:?}", v),
        }
    }
    Ok(FunResult::String(result))
}

/// Parses an argument
fn parse_arg(tag: Pair<Rule>, cli_args: &TaskArgs) -> DynErrResult<FunResult> {
    let mut tag_inner = tag.into_inner();
    let arg_index = tag_inner.next().unwrap().as_str();
    let real_index: usize = usize::from_str(arg_index).unwrap() - 1;
    let val: Option<&String> = cli_args.get("*").unwrap().get(real_index);
    match val {
        None => Ok(FunResult::Vec(vec![])),
        Some(val) => Ok(FunResult::String(String::from(val))),
    }
}

/// Parses named arguments
fn parse_kwargs(tag: Pair<Rule>, cli_args: &TaskArgs) -> DynErrResult<FunResult> {
    let mut tag_inner = tag.into_inner();
    let arg_name = tag_inner.next().unwrap().as_str();
    let values = cli_args.get(arg_name);
    match values {
        None => Ok(FunResult::Vec(vec![])),
        Some(values) => Ok(FunResult::Vec(values.clone())),
    }
}

/// Parses environment variables
fn parse_env_var(tag: Pair<Rule>, env: &HashMap<String, String>) -> DynErrResult<FunResult> {
    let mut tag_inner = tag.into_inner();
    let env_var_name = tag_inner.next().unwrap();
    let env_var = env.get(env_var_name.as_str());
    match env_var {
        None => Ok(FunResult::Vec(vec![])),
        Some(val) => Ok(FunResult::String(val.clone())),
    }
}

/// Parses the star variable
fn parse_all(cli_args: &TaskArgs) -> DynErrResult<FunResult> {
    // * is assumed to exist
    match cli_args.get("*") {
        None => Ok(FunResult::Vec(vec![])),
        Some(v) => Ok(FunResult::Vec(v.clone())),
    }
}

/// Parses a tag
fn parse_tag(
    tag: Pair<Rule>,
    cli_args: &TaskArgs,
    env: &HashMap<String, String>,
) -> DynErrResult<FunResult> {
    if let Some(tag) = tag.into_inner().next() {
        return parse_expression(tag, cli_args, env);
    }
    panic!("tag should have inner values");
}

/// Parses the script, returning a String
///
/// # Arguments
///
/// * `script`: Script to parse
/// * `args`: cli arguments
/// * `env`: env variables
///
/// returns: Result<String, Box<dyn Error, Global>>
///
pub fn parse_script<S: AsRef<str>>(
    script: S,
    args: &TaskArgs,
    env: &HashMap<String, String>,
    escape_mode: &EscapeMode,
) -> DynErrResult<String> {
    let tokens = ScriptParser::parse(Rule::all, script.as_ref());

    let mut result = String::new();

    let tokens = match tokens {
        Ok(mut tokens) => tokens.next().unwrap().into_inner(),
        Err(e) => return Err(e.renamed_rules(rename_rules).to_string().into()),
    };

    for token in tokens {
        match token.as_rule() {
            Rule::comment => {} // just ignore
            Rule::literal => {
                for literal in token.into_inner() {
                    match literal.as_rule() {
                        Rule::esc_ob => result.push('{'),
                        Rule::esc_cb => result.push('}'),
                        Rule::literal_content => result.push_str(literal.as_str()),
                        v => {
                            panic!("Unexpected rule {:?}", v);
                        }
                    }
                }
            }
            Rule::tag => {
                let tag_val = parse_tag(token, args, env)?;
                match tag_val {
                    FunResult::String(val) => {
                        let escape = match escape_mode {
                            EscapeMode::Always => true,
                            EscapeMode::Spaces => val.contains(' '),
                            EscapeMode::Never => false,
                        };
                        if escape {
                            result.push('"');
                        }
                        result.push_str(&val);
                        if escape {
                            result.push('"');
                        }
                    }
                    FunResult::Vec(values) => {
                        if !values.is_empty() {
                            let last_val_index = values.len() - 1;
                            for (i, val) in values.iter().enumerate() {
                                let escape = match escape_mode {
                                    EscapeMode::Always => true,
                                    EscapeMode::Spaces => val.contains(' '),
                                    EscapeMode::Never => false,
                                };

                                if escape {
                                    result.push('"');
                                }
                                result.push_str(val);
                                if escape {
                                    result.push('"');
                                }
                                if i != last_val_index {
                                    result.push(' ');
                                }
                            }
                        }
                    }
                }
            }
            Rule::EOI => {
                break;
            }
            v => {
                panic!("Unexpected rule {:?}", v);
            }
        }
    }
    Ok(result)
}

/// Parses the param, returning either a string or list of strings
///
/// # Arguments
///
/// * `script`: Script to parse
/// * `args`: cli arguments
/// * `env`: env variables
///
/// returns: Result<String, Box<dyn Error, Global>>
///
fn parse_param(
    param: &str,
    args: &TaskArgs,
    env: &HashMap<String, String>,
) -> DynErrResult<FunResult> {
    let pairs = ScriptParser::parse(Rule::task_arg, param);

    let mut pairs = match pairs {
        Ok(mut tokens) => tokens.next().unwrap().into_inner(),
        Err(e) => return Err(e.renamed_rules(rename_rules).to_string().into()),
    };

    match pairs.peek().unwrap().as_rule() {
        Rule::tag => {
            let tag = pairs.next().unwrap();
            let next = pairs.next().unwrap();
            match next.as_rule() {
                Rule::EOI => (), // expected
                v => {
                    panic!("Unexpected rule {:?}", v);
                }
            }
            parse_tag(tag, args, env)
        }
        Rule::literal => {
            let mut buffer = String::new();
            for pair in pairs {
                match pair.as_rule() {
                    Rule::EOI => (),
                    Rule::literal => {
                        for pair in pair.into_inner() {
                            match pair.as_rule() {
                                Rule::esc_ob => buffer.push('{'),
                                Rule::esc_cb => buffer.push('}'),
                                Rule::literal_content => buffer.push_str(pair.as_str()),
                                v => panic!("Unexpected rule {:?}", v),
                            }
                        }
                    }
                    v => panic!("Unexpected rule {:?}", v),
                }
            }
            Ok(FunResult::String(buffer))
        }
        Rule::EOI => Ok(FunResult::String(String::new())),
        v => panic!("Unexpected rule {:?}", v),
    }
}

/// Parses the given params
///
/// # Arguments
///
/// * `script`: Script to parse
/// * `args`: cli arguments
/// * `env`: env variables
///
/// returns: Result<String, Box<dyn Error, Global>>
///
pub fn parse_params(
    params: &Vec<String>,
    args: &TaskArgs,
    env: &HashMap<String, String>,
) -> DynErrResult<Vec<String>> {
    let mut result = Vec::with_capacity(params.capacity());
    for param in params {
        match parse_param(param, args, env)? {
            FunResult::String(val) => result.push(val),
            FunResult::Vec(values) => result.extend(values),
        }
    }
    Ok(result)
}

#[test]
fn test_parse_script() {
    let mut vars = HashMap::<String, Vec<String>>::new();
    let mut env = HashMap::new();

    let script = "hello {$@?}";
    let result = parse_script(script, &vars, &env, &EscapeMode::Never).unwrap();
    assert_eq!(result, "hello ");

    env.insert(
        String::from("TEST_ENV_VARIABLE"),
        String::from("sample_val"),
    );

    vars.insert(
        String::from("*"),
        vec![
            String::from("positional"),
            String::from("--key=val1"),
            String::from("--key=val2"),
        ],
    );

    vars.insert(
        String::from("key"),
        vec![String::from("val1"), String::from("val2")],
    );

    let script =
        "Echo {{Hello}} {$@}{hello?} {key} {$1} {$2} {$5?} {$TEST_ENV_VARIABLE} {$TEST_ENV_VARIABLE2?}";
    let result = parse_script(script, &vars, &env, &EscapeMode::Never).unwrap();
    assert_eq!(
        result,
        "Echo {Hello} positional --key=val1 --key=val2 val1 val2 positional --key=val1  sample_val "
    );

    let script = r#"Echo {{map(Hello)}} {map("--f=\"%s.txt\"",key)}"#;

    let result = parse_script(script, &vars, &env, &EscapeMode::Never).unwrap();
    assert_eq!(
        result,
        "Echo {map(Hello)} --f=\"val1.txt\" --f=\"val2.txt\""
    );

    let script = r#"
print("hello world")
a = [{map("%s\n",jmap("\n      '\\%s\\',",$@))}]
print("values are:", a)"#;

    let expected = r#"
print("hello world")
a = [
      '\positional\',
      '\--key=val1\',
      '\--key=val2\',
]
print("values are:", a)"#;

    let result = parse_script(script, &vars, &env, &EscapeMode::Never).unwrap();
    assert_eq!(result, expected);

    let script = "echo {$@[0]} {$@[-1]} {$@[-3:]} {key[:5]}{key[5]?}{key[5:]?}";
    let result = parse_script(script, &vars, &env, &EscapeMode::Never).unwrap();
    assert_eq!(
        result,
        "echo positional --key=val2 positional --key=val1 --key=val2 val1 val2"
    );

    let script =
        "echo {key[0][0]} {key[:5][0][1]} {key[0][2:3]} {key[0][3:]} {key[0][4]?} {key[:5][10:][1]?} end";
    let result = parse_script(script, &vars, &env, &EscapeMode::Never).unwrap();
    assert_eq!(result, "echo v a l 1   end");

    let script = "echo {key[3][0]}";
    let result = parse_script(script, &vars, &env, &EscapeMode::Never).unwrap_err();
    assert!(result
        .to_string()
        .ends_with("Index out of bounds for mandatory expression"));

    let script = "echo {key[0][10]}";
    let result = parse_script(script, &vars, &env, &EscapeMode::Never).unwrap_err();
    assert!(result
        .to_string()
        .ends_with("Index out of bounds for mandatory expression"));

    let script = "echo {key[0][-5]}";
    let result = parse_script(script, &vars, &env, &EscapeMode::Never).unwrap_err();
    assert!(result
        .to_string()
        .ends_with("Index out of bounds for mandatory expression"));

    let script = "echo {key[5:0]}";
    let result = parse_script(script, &vars, &env, &EscapeMode::Never).unwrap_err();
    assert!(result
        .to_string()
        .ends_with("Range out of bounds for mandatory expression"));

    let script = "echo {key[-10:5]}";
    let result = parse_script(script, &vars, &env, &EscapeMode::Never).unwrap_err();
    assert!(result
        .to_string()
        .ends_with("Range out of bounds for mandatory expression"));
}

#[test]
fn test_parse_script_errors() {
    let vars = HashMap::<String, Vec<String>>::new();
    let env = HashMap::new();

    let script = "hello {$";
    let result = parse_script(script, &vars, &env, &EscapeMode::Never).unwrap_err();
    assert_eq!(result.to_string(), " --> 1:9\n  |\n1 | hello {$\n  |         ^---\n  |\n  = expected integer or environment variable name");

    // TODO: Test more parsing errors
}

#[test]
fn test_parse_params() {
    let mut vars = HashMap::<String, Vec<String>>::new();
    let mut env = HashMap::new();

    env.insert(
        String::from("TEST_ENV_VARIABLE"),
        String::from("sample_val"),
    );

    vars.insert(
        String::from("*"),
        vec![
            String::from("positional"),
            String::from("--key=val1"),
            String::from("--key=val2"),
        ],
    );

    vars.insert(
        String::from("key"),
        vec![String::from("val1"), String::from("val2")],
    );

    let params = vec![
        "Echo",
        "{{Hello}}",
        "{$@}",
        "{key}",
        "{$1}",
        "{$2}",
        "{$5?}",
        "{$TEST_ENV_VARIABLE}",
        "{$TEST_ENV_VARIABLE2?}",
    ];

    let result =
        parse_params(&params.iter().map(|v| v.to_string()).collect(), &vars, &env).unwrap();
    assert_eq!(
        result,
        vec![
            "Echo",
            "{Hello}",
            "positional",
            "--key=val1",
            "--key=val2",
            "val1",
            "val2",
            "positional",
            "--key=val1",
            "sample_val"
        ]
    );

    let params = vec![
        "Echo",
        "{{map(Hello)}}",
        r#"{ map("--f=\"%s.txt\"", key) }"#,
    ];

    let result =
        parse_params(&params.iter().map(|v| v.to_string()).collect(), &vars, &env).unwrap();
    assert_eq!(
        result,
        vec![
            "Echo",
            "{map(Hello)}",
            "--f=\"val1.txt\"",
            "--f=\"val2.txt\""
        ]
    );

    let params = vec![
        "Echo",
        "{{jmap(Hello)}}",
        r#"{ jmap("--f=\"%s.txt\" ", key) }"#,
    ];

    let result =
        parse_params(&params.iter().map(|v| v.to_string()).collect(), &vars, &env).unwrap();
    assert_eq!(
        result,
        vec![
            "Echo",
            "{jmap(Hello)}",
            "--f=\"val1.txt\" --f=\"val2.txt\" "
        ]
    );
}
