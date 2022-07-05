use std::collections::HashMap;
use std::{env, error, fmt};
use std::env::Args;
use std::fmt::Debug;
use std::fs::read;
use std::ops::Add;
use lazy_static::lazy_static;
use regex::{Regex, Replacer};

pub const OPEN_TOKEN: char = '{';
pub const CLOSE_TOKEN: char = '}';
pub const OPTIONAL_TOKEN: char = '?';
pub const UNESCAPED_OPEN_TOKEN_ERROR: &str = "Unescaped '{'.";
pub const UNESCAPED_CLOSE_TOKEN_ERROR: &str = "Unescaped '}'.";
pub const BAD_OPTIONAL_TOKEN_ERROR: &str = "'?' may only be added at the end of the argument.";
pub const UNCLOSED_TAG_ERROR: &str = "Unclosed argument tag.";
pub const EMPTY_TAG_ERROR: &str = "Empty argument tag.";
pub const INVALID_ARG_CHAR_ERROR: &str = "Positional argument variables can only contain digits. \
Keyword arguments my be prepended with '-', must start with english alphabetic characters, \
and can only contain english alphabetic and digit characters, '-' and '_'. \
Additionally, you can use '{*}' to pass all arguments as given.";


#[derive(Debug, PartialEq)]
pub enum FormatError {
    Invalid(String),  // Invalid format string
    KeyError(String), // Missing mandatory argument
}

pub struct CommandArgs {
    /// Manually set config file
    pub(crate) file: Option<String>,
    /// Command to run, if given
    pub(crate) task: Option<String>,
    /// Args to run the command with
    pub(crate) args: HashMap<String, String>
}

// TODO: Implement second mode
/// We can run the program in two different modes, one is to run a command with args
/// amd the other mode is to run other things like help, list commands etc
pub enum YamisArgs {
    CommandArgs(CommandArgs),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            FormatError::Invalid(ref s) => write!(f, "Invalid format string. {}", s),
            FormatError::KeyError(ref s) => write!(f, "Missing mandatory argument: {}", s),
        }
    }
}

impl error::Error for FormatError {
    fn description(&self) -> &str {
        match *self {
            FormatError::Invalid(_) => "invalid format string",
            FormatError::KeyError(_) => "missing mandatory argument",
        }
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        None
    }
}

fn is_valid_arg(arg: &str) -> bool {
    lazy_static! {
        static ref VALID_ARG_RE: Regex = Regex::new(r"^(?:(?:-*[a-zA-Z]+[a-zA-Z0-9_\-]*)|[0-9]+|\*)$").unwrap();
    }
    return VALID_ARG_RE.is_match(arg);
}

/// Formats the given format string with the given args. This differs a bit to the classical string
/// formats.
///
/// Format arguments have to be surrounded by `{}`, i.e. `{1}`, `{a}`, `{-a}`. By adding the
/// `?` char at the end of the arg name, i.e. `{a?}`, these can be made optional. "Positional
/// argument variables can only contain digits. Keyword arguments my be prepended with
/// '-', must start with english alphabetic characters, and can only contain english alphabetic
/// and digit characters, '-' and '_'. Additionally, '{*}' is allowed.
///
/// # Arguments
/// * `fmtstr` - String to format
/// * `args` - HashMap containing the arguments
pub fn format_string(fmtstr: &str, args: &HashMap<String, String>) -> Result<String, FormatError>{
    let mut out = String::with_capacity(fmtstr.len() * 2);
    let mut arg = String::with_capacity(10);
    let mut reading_arg = false;
    let mut found_open_token = false;
    let mut found_close_token = false;
    let mut optional_arg = false;
    for c in fmtstr.chars() {
        // unescaped close token that doesn't close a tag
        if c != CLOSE_TOKEN && found_close_token {
            return Err(FormatError::Invalid(String::from(UNESCAPED_CLOSE_TOKEN_ERROR)));
        }
        // OPTIONAL_TOKEN not added at the end of parameter
        if c != CLOSE_TOKEN && optional_arg {
            return Err(FormatError::Invalid(String::from(BAD_OPTIONAL_TOKEN_ERROR)))
        }
        // Found OPTIONAL_TOKEN, still waiting for tag closure
        if c == OPTIONAL_TOKEN && reading_arg {
            optional_arg = true;
        }
        else if c == CLOSE_TOKEN {
            if reading_arg {
                if !is_valid_arg(&arg) {
                    return Err(FormatError::Invalid(String::from(INVALID_ARG_CHAR_ERROR)))
                }
                match args.get(&arg) {
                    None => {
                        if !optional_arg {
                            return Err(FormatError::KeyError(arg));
                        }
                    }
                    Some(val) => {
                        out.push_str(val);
                    }
                }
                found_close_token = false;
                optional_arg = false;
                reading_arg = false;
                arg.clear();
            }
            else if found_close_token {  // escaped token
                found_close_token = false;
                out.push(CLOSE_TOKEN);
            }
            else if found_open_token{
                return Err(FormatError::Invalid(String::from(EMPTY_TAG_ERROR)));
            }
            else {
                found_close_token = true;  // waiting to see if it is escaped
            }
        } else if c == OPEN_TOKEN {
            if found_open_token {  // escaped token
                found_open_token = false;
                out.push(OPEN_TOKEN);
            }
            else if reading_arg {
                return Err(FormatError::Invalid(String::from(UNCLOSED_TAG_ERROR)));
            }
            else {
                found_open_token = true;
            }
        } else if reading_arg {
            arg.push(c);
        } else {
            if found_open_token {
                arg.push(c);
                reading_arg = true;
                found_open_token = false;
            } else {
                out.push(c);
            }
        }
    }
    if found_open_token {
        return Err(FormatError::Invalid(String::from(UNESCAPED_OPEN_TOKEN_ERROR)));
    }
    if reading_arg {
        return Err(FormatError::Invalid(String::from(UNCLOSED_TAG_ERROR)))
    }
    if found_close_token {
        return Err(FormatError::Invalid(String::from(UNESCAPED_CLOSE_TOKEN_ERROR)))
    }
    return Ok(out);
}

impl YamisArgs {
    pub fn new(mut args: Args) -> YamisArgs {
        args.next(); // ignore the program name arg
        let args: Vec<String> = args.collect();
        if args.len() > 0 && args[0].starts_with("-") {
            panic!("Not implemented yet");
        }
        return YamisArgs::CommandArgs(CommandArgs::new(args));
    }
}

impl CommandArgs {
    fn new(mut args: Vec<String>) -> CommandArgs{
        let arg_regex: Regex = Regex::new(r"(?P<key>[a-zA-Z\-]{1,2}[a-zA-Z_\-])=(?P<val>[\s\S]*)").unwrap();
        let mut kwargs: HashMap<String, String> = HashMap::new();
        let mut file: Option<String> = None;
        let mut command: Option<String> = None;

        if let Some(first_arg) = args.get(0) {
            if first_arg.to_lowercase().ends_with(".toml") {
                file = Some(args.remove(0));
            }
            if let Some(_) = args.get(0) {
                command = Some(args.remove(0));
            }
        }

        for arg in args.iter().enumerate() {
            kwargs.insert(arg.0.to_string(), arg.1.clone());
        }

        kwargs.insert(String::from("*"), args.join(" "));

        for arg in args {
            let arg_match = arg_regex.captures(&arg);
            if let Some(arg_match) = arg_match {
                // TODO: Handle unwraps
                kwargs.insert(String::from(arg_match.name("key").unwrap().as_str()), String::from(arg_match.name("val").unwrap().as_str()));
            }
        }

        return CommandArgs {file, task: command, args: kwargs };
    }
}