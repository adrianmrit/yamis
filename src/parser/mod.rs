use crate::cli::TaskArgs;
use crate::parser::functions::{FunResult, DEFAULT_FUNCTIONS};
use crate::types::DynErrResult;
use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;
use serde_derive::Deserialize;
use std::collections::HashMap;
use std::str::FromStr;

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

/// Pest parser for script
#[derive(Parser)]
#[grammar = "parser/grammar.pest"]
struct ScriptParser;

/// Parses a function
fn parse_fun(
    tag: Pair<Rule>,
    cli_args: &TaskArgs,
    env: &HashMap<String, String>,
) -> DynErrResult<FunResult> {
    let mut tag_inner = tag.into_inner();
    let fun_name = tag_inner.next().unwrap().as_str();
    let arguments = tag_inner.next();
    let fun = match DEFAULT_FUNCTIONS.functions.get(fun_name) {
        None => return Err(format!("There is no function named {fun_name}").into()),
        Some(fun) => fun,
    };

    let arguments: Vec<FunResult> = match arguments {
        None => {
            vec![]
        }
        Some(arguments) => {
            let mut arguments_list: Vec<FunResult> = vec![];
            for param in arguments.into_inner() {
                // Param takes a single value that can be of different types
                let param_type = param.into_inner().next().unwrap();
                match param_type.as_rule() {
                    Rule::fun => {
                        let fun_result = parse_fun(param_type, cli_args, env)?;
                        arguments_list.push(fun_result);
                    }
                    Rule::arg => arguments_list.push(parse_arg(param_type, cli_args)?),
                    Rule::kwarg => arguments_list.push(parse_kwargs(param_type, cli_args)?),
                    Rule::star => arguments_list.push(parse_star(param_type, cli_args)?),
                    Rule::env_var => arguments_list.push(parse_env_var(param_type, env)?),
                    Rule::string => arguments_list.push(parse_string(param_type)?),
                    v => panic!("Unexpected rule {v:?}"),
                }
            }
            arguments_list
        }
    };
    Ok(fun(&arguments.iter().map(|v| v.as_val()).collect())?)
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
                    v => {
                        panic!("Unexpected escaped value {}", v)
                    }
                }
                if let Some(other) = inner.next() {
                    panic!("Unexpected pair {other:?}")
                }
            }
            Rule::escape_dq => result.push('"'),
            Rule::escape_sq => result.push('\''),
            v => panic!("Unexpected rule {v:?}"),
        }
    }
    Ok(FunResult::String(result))
}

/// Parses an argument
fn parse_arg(tag: Pair<Rule>, cli_args: &TaskArgs) -> DynErrResult<FunResult> {
    let mut tag_inner = tag.into_inner();
    let arg_index = tag_inner.next().unwrap().as_str();
    let real_index: usize = usize::from_str(arg_index).unwrap() - 1;
    let modifier = tag_inner.next();
    let val = cli_args.get("*").unwrap().get(real_index);
    match val {
        None => {
            if modifier.is_none() {
                Err(format!("Mandatory argument at position {arg_index} not set.").into())
            } else {
                Ok(FunResult::Vec(vec![]))
            }
        }
        Some(val) => Ok(FunResult::String(val.clone())),
    }
}

/// Parses named arguments
fn parse_kwargs(tag: Pair<Rule>, cli_args: &TaskArgs) -> DynErrResult<FunResult> {
    let mut tag_inner = tag.into_inner();
    let arg_name = tag_inner.next().unwrap().as_str();
    let modifier = tag_inner.next();
    let values = cli_args.get(arg_name);
    match values {
        None => {
            if modifier.is_none() {
                return Err(format!("Mandatory argument `{arg_name}` not set.").into());
            } else {
                Ok(FunResult::Vec(vec![]))
            }
        }
        Some(values) => Ok(FunResult::Vec(values.clone())),
    }
}

/// Parses environment variables
fn parse_env_var(tag: Pair<Rule>, env: &HashMap<String, String>) -> DynErrResult<FunResult> {
    let mut tag_inner = tag.into_inner();
    let env_var_name = tag_inner.next().unwrap();
    // for now modifier can only be '?'
    let required = tag_inner.next().is_none();
    let env_var = env.get(env_var_name.as_str());
    match env_var {
        None => {
            if required {
                Err(format!("Mandatory environment variable `{env_var_name}` not set.").into())
            } else {
                Ok(FunResult::Vec(vec![]))
            }
        }
        Some(val) => Ok(FunResult::String(val.clone())),
    }
}

/// Parses the star variable
fn parse_star(tag: Pair<Rule>, cli_args: &TaskArgs) -> DynErrResult<FunResult> {
    // * is assumed to exist
    let values = cli_args.get("*").unwrap();
    let modifier = tag.into_inner().next();
    // for now modifier can only be '?'
    if modifier.is_some() && values.len() == 0 {
        return Err("Arguments are required".into());
    }
    Ok(FunResult::Vec(values.clone()))
}

/// Parses a tag
fn parse_tag(
    tag: Pair<Rule>,
    cli_args: &TaskArgs,
    env: &HashMap<String, String>,
) -> DynErrResult<FunResult> {
    if let Some(tag) = tag.into_inner().next() {
        return match tag.as_rule() {
            Rule::star => parse_star(tag, cli_args),
            Rule::env_var => parse_env_var(tag, env),
            Rule::kwarg => parse_kwargs(tag, cli_args),
            Rule::arg => parse_arg(tag, cli_args),
            Rule::fun => parse_fun(tag, cli_args, env),
            v => {
                panic!("Unexpected rule {v:?}");
            }
        };
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
        Err(e) => return Err(e.to_string().into()),
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
                            panic!("Unexpected rule {v:?}");
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
                        if values.len() > 0 {
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
        Err(e) => return Err(e.to_string().into()),
    };

    match pairs.peek().unwrap().as_rule() {
        Rule::tag => {
            let tag = pairs.next().unwrap();
            let next = pairs.next().unwrap();
            match next.as_rule() {
                Rule::EOI => (), // expected
                v => {
                    panic!("Unexpected rule {v:?}");
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
                                v => panic!("Unexpected rule {v:?}"),
                            }
                        }
                    }
                    v => panic!("Unexpected rule {v:?}"),
                }
            }
            Ok(FunResult::String(buffer))
        }
        Rule::EOI => Ok(FunResult::String(String::new())),
        v => panic!("Unexpected rule {v:?}"),
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
        "Echo {{Hello}} {*} {key} {1} {2} {5?} {$TEST_ENV_VARIABLE} {$TEST_ENV_VARIABLE2?}";
    let result = parse_script(script, &vars, &env, &EscapeMode::Never).unwrap();
    assert_eq!(
        result,
        "Echo {Hello} positional --key=val1 --key=val2 val1 val2 positional --key=val1  sample_val "
    );

    let script = r#"Echo {{map(Hello)}} {map("--f=\"{}.txt\"",key)}"#;

    let result = parse_script(script, &vars, &env, &EscapeMode::Never).unwrap();
    assert_eq!(
        result,
        "Echo {map(Hello)} --f=\"val1.txt\" --f=\"val2.txt\""
    );

    let script = r#"
print("hello world")
a = [{map("{}\n",flat("\n      '\\{}\\',",*))}]
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
        "{*}",
        "{key}",
        "{1}",
        "{2}",
        "{5?}",
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

    let params = vec!["Echo", "{{map(Hello)}}", r#"{map("--f=\"{}.txt\"",key)}"#];

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
        "{{flat(Hello)}}",
        r#"{flat("--f=\"{}.txt\" ",key)}"#,
    ];

    let result =
        parse_params(&params.iter().map(|v| v.to_string()).collect(), &vars, &env).unwrap();
    assert_eq!(
        result,
        vec![
            "Echo",
            "{flat(Hello)}",
            "--f=\"val1.txt\" --f=\"val2.txt\" "
        ]
    );
}
