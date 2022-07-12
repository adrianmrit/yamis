use std::collections::HashMap;
use std::env::Args;
use std::fmt::Debug;
use std::{error, fmt};

use lazy_static::lazy_static;
use regex::Regex;

pub const OPEN_TOKEN: char = '{';
pub const CLOSE_TOKEN: char = '}';
pub const OPTIONAL_TOKEN: char = '?';
pub const UNESCAPED_OPEN_TOKEN_ERROR: &str = "Unescaped '{'.";
pub const UNESCAPED_CLOSE_TOKEN_ERROR: &str = "Unescaped '}'.";
pub const UNCLOSED_TAG_ERROR: &str = "Unclosed argument tag.";
pub const EMPTY_TAG_ERROR: &str = "Empty argument tag.";
pub const INVALID_ARG_CHAR_ERROR: &str = "Invalid argument tag.";

/// Represent string format errors.
#[derive(Debug, PartialEq)]
pub enum FormatError {
    /// Raised when an invalid format string is given
    Invalid(String), // Invalid format string
    /// Raised when a required argument was not given
    KeyError(String), // Missing mandatory argument
}

/// Extra args passed that will be mapped to the task.
pub type ArgsMap = HashMap<String, Vec<String>>;

/// Holds the data for running the given task.
pub struct CommandArgs {
    /// Manually set config file
    pub file: Option<String>,
    /// Task to run, if given
    pub task: Option<String>,
    /// Args to run the command with
    pub args: ArgsMap,
}

/// Represents an argument tag
struct ArgumentTag {
    required: bool,
    /// Argument name that would be replaced with the value
    arg: String,
    /// Prefix to be added before the replaced value.
    prefix: String,
    /// Suffix to be added before the replaced value.
    suffix: String,
}

// TODO: Implement second mode (still undefined)
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

/// Returns the regex used to parse argument tags
fn get_argument_tag_regex() -> Regex {
    return Regex::new(
        r"^(?:\((?P<prefix>.*?)\))?(?P<arg>([a-zA-Z]+[a-zA-Z\d_\-]*)|\d+|\*)(?P<optional>\?)?(?:\((?P<suffix>.*?)\))?$",
    )
    .unwrap();
}

/// Given the content of an argument tag, returns a representation of it
fn get_argument_tag(arg: &str) -> Option<ArgumentTag> {
    lazy_static! {
        static ref VALID_ARG_RE: Regex = get_argument_tag_regex();
    }
    let capture = VALID_ARG_RE.captures(arg)?;
    let arg = String::from(capture.name("arg").unwrap().as_str());
    let prefix = match capture.name("prefix") {
        None => String::from(""),
        Some(val) => String::from(val.as_str()),
    };
    let suffix = match capture.name("suffix") {
        None => String::from(""),
        Some(val) => String::from(val.as_str()),
    };
    let required = match capture.name("optional") {
        None => true,
        Some(_) => false,
    };
    return Some(ArgumentTag {
        required,
        arg,
        prefix,
        suffix,
    });
}

/// Formats the given format string with the given args. This differs a bit to the classical string
/// formats.
///
/// Format arguments have to be surrounded by `{}`, i.e. `{1}`, `{a}`, `{a}`. By adding the
/// `?` char at the end of the arg name, i.e. `{a?}`, these can be made optional. "Positional
/// argument variables can only contain digits. Keyword arguments must start with english alphabetic
/// characters, and can only contain english alphabetic and digit characters. Additionally, `{*}` is
/// allowed. If you want to add a prefix or a suffix only if the value is given, you might put them
/// surrounded by parenthesis after or before an argument, i.e. `{(-f=)out?(.txt)}`. Also, if the same
/// named value is passed multiple time in the arguments, the argument tag will be used to parse
/// each value and will add them separated by spaces.
///
/// # Arguments
///
/// * `fmtstr` - String to format
/// * `args` - HashMap containing the arguments
pub fn format_string(fmtstr: &str, args: &ArgsMap, quote: bool) -> Result<String, FormatError> {
    let mut out = String::with_capacity(fmtstr.len() * 2);
    let mut arg = String::with_capacity(10);
    let mut reading_arg = false;
    let mut found_open_token = false;
    let mut found_close_token = false;
    for c in fmtstr.chars() {
        // unescaped close token that doesn't close a tag
        if c != CLOSE_TOKEN && found_close_token {
            return Err(FormatError::Invalid(String::from(
                UNESCAPED_CLOSE_TOKEN_ERROR,
            )));
        }
        // Found OPTIONAL_TOKEN, still waiting for tag closure
        if c == CLOSE_TOKEN {
            if reading_arg {
                match get_argument_tag(&arg) {
                    None => {
                        return Err(FormatError::Invalid(String::from(INVALID_ARG_CHAR_ERROR)));
                    }
                    Some(arg) => match args.get(&arg.arg) {
                        None => {
                            if arg.required {
                                return Err(FormatError::KeyError(arg.arg));
                            }
                        }
                        Some(values) => {
                            let last_val_index = values.len() - 1;
                            for (i, val) in values.iter().enumerate() {
                                let escape = quote && val.contains(' ');
                                out.push_str(&arg.prefix);
                                if escape {
                                    out.push('"');
                                }
                                out.push_str(val);
                                out.push_str(&arg.suffix);
                                if escape {
                                    out.push('"');
                                }
                                /// Values are separated by spaces but the
                                /// last value should not be
                                if i != last_val_index {
                                    out.push(' ');
                                }
                            }
                        }
                    },
                }
                found_close_token = false;
                reading_arg = false;
                arg.clear();
            } else if found_close_token {
                // escaped token
                found_close_token = false;
                out.push(CLOSE_TOKEN);
            } else if found_open_token {
                return Err(FormatError::Invalid(String::from(EMPTY_TAG_ERROR)));
            } else {
                found_close_token = true; // waiting to see if it is escaped
            }
        } else if c == OPEN_TOKEN {
            if found_open_token {
                // escaped token
                found_open_token = false;
                out.push(OPEN_TOKEN);
            } else if reading_arg {
                return Err(FormatError::Invalid(String::from(UNCLOSED_TAG_ERROR)));
            } else {
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
        return Err(FormatError::Invalid(String::from(
            UNESCAPED_OPEN_TOKEN_ERROR,
        )));
    }
    if reading_arg {
        return Err(FormatError::Invalid(String::from(UNCLOSED_TAG_ERROR)));
    }
    if found_close_token {
        return Err(FormatError::Invalid(String::from(
            UNESCAPED_CLOSE_TOKEN_ERROR,
        )));
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
    fn new(mut args: Vec<String>) -> CommandArgs {
        let arg_regex: Regex =
            // TODO: Check best way to implement
            Regex::new(r"-*(?P<key>[a-zA-Z]+\w*)=(?P<val>[\s\S]*)")
                .unwrap();
        let mut kwargs = ArgsMap::new();
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

        for arg in &args {
            let arg_match = arg_regex.captures(arg);
            if let Some(arg_match) = arg_match {
                let key = String::from(arg_match.name("key").unwrap().as_str());
                let val = String::from(arg_match.name("val").unwrap().as_str());
                if kwargs.contains_key(&key) {
                    kwargs.get_mut(&key).unwrap().push(val);
                } else {
                    let args_vec: Vec<String> = vec![val];
                    kwargs.insert(key, args_vec);
                }
            }
        }

        kwargs.insert(String::from("*"), args);

        return CommandArgs {
            file,
            task: command,
            args: kwargs,
        };
    }
}
