use crate::args::ArgsMap;
use lazy_static::lazy_static;
use regex::Regex;
use std::str::{Chars, FromStr};
use std::{error, fmt, mem};

// Symbols used to identify the state on the stack
const OPEN_TAG_SYMBOL: char = '{';
const CLOSE_TAG_SYMBOL: char = '}';
const INSIDE_TAG_SYMBOL: char = '_';
const EMPTY_STACK_SYMBOL: char = '\0';

/// Iterator over tokens.
struct Tokens<'a> {
    /// Iterator over the chars of the string to extract the tokens from
    chars: Chars<'a>,
    /// Holds the next token to return as as it is build
    token: String,
    /// Used maintain a state
    // Could probably use an single variable,
    // but could be useful if we ever implement something more complex
    stack: Vec<char>,
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

/// Represent string format errors.
#[derive(Debug, PartialEq)]
pub enum FormatError {
    /// Raised when an invalid format string is given
    Invalid(String), // Invalid format string
    /// Raised when a required argument was not given
    KeyError(String), // Missing mandatory argument
}

/// Modes to escape (add quotes) the arguments passed to the script
pub enum EscapeMode {
    /// Always quote the arguments
    Always,
    /// Only add quotes if the argument has spaces
    OnSpace,
    /// Never quote the argument
    Never,
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

impl<'a> Tokens<'a> {
    /// Constructs a new Tokens iterator
    fn new(string: &'a str) -> Self {
        return Self {
            chars: string.chars(),
            stack: vec![EMPTY_STACK_SYMBOL],
            token: String::new(),
        };
    }
}

impl<'a> Iterator for Tokens<'a> {
    type Item = Result<(bool, String), FormatError>;

    /// Returns the next token
    fn next(&mut self) -> Option<Self::Item> {
        for char in self.chars.by_ref() {
            let last_special_char = *self.stack.last().unwrap();
            let is_tag = last_special_char == '_';
            match last_special_char {
                INSIDE_TAG_SYMBOL => match char {
                    '}' => {
                        // Pop INSIDE_TAG_SYMBOL
                        self.stack.pop();
                        // Pop OPEN_TAG_SYMBOL
                        self.stack.pop();
                        let result = self.token.clone();
                        self.token.clear();
                        return Some(Ok((is_tag, result)));
                    }
                    '{' | '\n' => {
                        return Some(Err(FormatError::Invalid("Unclosed tag.".to_string())))
                    }
                    c => {
                        self.token.push(c);
                    }
                },
                OPEN_TAG_SYMBOL => {
                    match char {
                        '{' => {
                            // Escaped
                            self.stack.pop();
                            self.token.push(OPEN_TAG_SYMBOL);
                        }
                        '}' => {
                            return Some(Err(FormatError::Invalid(
                                "Empty argument tag.".to_string(),
                            )));
                        }
                        '\n' => {
                            return Some(Err(FormatError::Invalid("Unclosed tag.".to_string())))
                        }
                        c => {
                            self.stack.push('_');
                            let result = self.token.clone();
                            self.token.clear();
                            self.token.push(c);
                            return Some(Ok((is_tag, result)));
                        }
                    }
                }
                CLOSE_TAG_SYMBOL => {
                    match char {
                        '}' => {
                            // Escaped
                            self.stack.pop();
                            self.token.push('}');
                        }
                        _ => return Some(Err(FormatError::Invalid("Unescaped '}'.".to_string()))),
                    }
                }
                _ => match char {
                    '}' => {
                        self.stack.push(CLOSE_TAG_SYMBOL);
                    }
                    // If not escaped, we should return the token, but we don't know
                    // yet if it is escaped
                    '{' => self.stack.push(OPEN_TAG_SYMBOL),
                    c => self.token.push(c),
                },
            }
        }
        // Reached the end of the string.
        return match *self.stack.last().unwrap() {
            OPEN_TAG_SYMBOL => Some(Err(FormatError::Invalid("Unescaped '{'.".to_string()))),
            INSIDE_TAG_SYMBOL => Some(Err(FormatError::Invalid("Unclosed tag.".to_string()))),
            CLOSE_TAG_SYMBOL => Some(Err(FormatError::Invalid("Unescaped '}'.".to_string()))),
            _ => {
                if self.token.is_empty() {
                    None
                } else {
                    // Replaces token with an string with 0 capacity since it
                    // will no longer be used, to avoid cloning
                    let old_v = mem::replace(&mut self.token, String::with_capacity(0));
                    Some(Ok((false, old_v)))
                }
            }
        };
    }
}

/// Returns the regex used to parse argument tags
fn get_argument_tag_regex() -> Regex {
    Regex::new(
        r"^(?:\((?P<prefix>.*?)\))?(?P<arg>([a-zA-Z]+[a-zA-Z\d_\-]*)|\d+|\*)(?P<optional>\?)?(?:\((?P<suffix>.*?)\))?$",
    )
        .unwrap()
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
    Some(ArgumentTag {
        required,
        arg,
        prefix,
        suffix,
    })
}

/// Formats a script string.
///
/// # Arguments
///
/// * `fmtstr`: Script string
/// * `args`: Values to format the script with
/// * `escape_mode`: How the passed values will be escaped
///
/// returns: Result<String, FormatError>
pub fn format_script(
    fmtstr: &str,
    args: &ArgsMap,
    escape_mode: EscapeMode,
) -> Result<String, FormatError> {
    let tokens = Tokens::new(fmtstr);
    let mut out = String::with_capacity(fmtstr.len() * 2);
    for token in tokens {
        let (is_tag, token) = token?;
        if is_tag {
            match get_argument_tag(&token) {
                None => {
                    return Err(FormatError::Invalid(format!(
                        "Invalid argument tag `{{{}}}`.",
                        token
                    )))
                }
                Some(arg) => {
                    let index_arg = usize::from_str(&arg.arg).unwrap_or(0);
                    let key = if index_arg > 0 {
                        String::from("*")
                    } else {
                        arg.arg
                    };
                    match args.get(&key) {
                        None => {
                            let arg_name = if index_arg > 0 {
                                index_arg.to_string()
                            } else {
                                key
                            };
                            return Err(FormatError::KeyError(arg_name));
                        }
                        Some(values) => {
                            if index_arg > 0 {
                                match values.get(index_arg - 1) {
                                    None => {
                                        if arg.required {
                                            return Err(FormatError::KeyError(
                                                index_arg.to_string(),
                                            ));
                                        }
                                    }
                                    Some(val) => {
                                        let escape = match escape_mode {
                                            EscapeMode::Always => true,
                                            EscapeMode::OnSpace => val.contains(' '),
                                            EscapeMode::Never => false,
                                        };
                                        if escape {
                                            out.push('"');
                                        }
                                        out.push_str(&arg.prefix);
                                        out.push_str(val);
                                        out.push_str(&arg.suffix);
                                        if escape {
                                            out.push('"');
                                        }
                                    }
                                }
                            } else {
                                let last_val_index = values.len() - 1;

                                for (i, val) in values.iter().enumerate() {
                                    let escape = match escape_mode {
                                        EscapeMode::Always => true,
                                        EscapeMode::OnSpace => val.contains(' '),
                                        EscapeMode::Never => false,
                                    };

                                    if escape {
                                        out.push('"');
                                    }
                                    out.push_str(&arg.prefix);
                                    out.push_str(val);
                                    out.push_str(&arg.suffix);
                                    if escape {
                                        out.push('"');
                                    }

                                    // Values are separated by spaces but the
                                    // last value should not be
                                    if i != last_val_index {
                                        out.push(' ');
                                    }
                                }
                            };
                        }
                    }
                }
            }
        } else {
            out.push_str(&token);
        }
    }
    Ok(out)
}
