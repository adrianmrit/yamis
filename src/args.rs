use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::ffi::OsString;

/// Represents the context of the arguments passed to task.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArgsContext {
    ///Holds a list of positional arguments
    pub(crate) args: Vec<String>,
    ///Holds keyword argument, the value is the last passed value
    pub(crate) kwargs: HashMap<String, String>,
    ///Holds a list of keyword arguments, the value is a list of all passed values
    pub(crate) pkwargs: HashMap<String, Vec<String>>,
}

impl ArgsContext {
    pub(crate) fn new() -> Self {
        Self {
            args: Vec::new(),
            kwargs: HashMap::new(),
            pkwargs: HashMap::new(),
        }
    }
    pub(crate) fn from(arg_matches: clap::ArgMatches) -> Self {
        if let Some(args_matched) = arg_matches.get_many::<OsString>("") {
            // All args are pushed into a vector as they are
            let args = args_matched
                .map(|s| s.to_string_lossy().to_string())
                .collect::<Vec<String>>();

            let mut kwargs: HashMap<String, String> = HashMap::new();
            let mut pkwargs: HashMap<String, Vec<String>> = HashMap::new();

            // kwarg found that could be a key
            let mut possible_kwarg_key: Option<String> = None;

            // looping over the args to find kwargs
            for arg in args.iter() {
                // if a kwarg key was previously found, assume this is the value, even if
                // it starts with - or --
                if let Some(possible_kwarg) = possible_kwarg_key {
                    // replace in kwargs if exists, otherwise insert
                    kwargs.insert(possible_kwarg.clone(), arg.clone());

                    match pkwargs.entry(possible_kwarg) {
                        Entry::Occupied(mut e) => {
                            e.get_mut().push(arg.clone());
                        }
                        Entry::Vacant(e) => {
                            let args_vec: Vec<String> = vec![arg.clone()];
                            e.insert(args_vec);
                        }
                    }
                    possible_kwarg_key = None;
                    continue;
                }

                // Quick check to see if the arg is a kwarg key or key-value pair
                // if it is a positional value, we just continue
                if !arg.starts_with('-') {
                    continue;
                }

                // Check if this is a kwarg key-value pair
                if let Some((key, val)) = Self::get_kwarg(arg) {
                    kwargs.insert(key.clone(), val.clone());
                    match pkwargs.entry(key) {
                        Entry::Occupied(mut e) => {
                            e.get_mut().push(val.clone());
                        }
                        Entry::Vacant(e) => {
                            let args_vec: Vec<String> = vec![val.clone()];
                            e.insert(args_vec);
                        }
                    }
                    continue;
                }

                // Otherwise it could be a kwarg key, for which we need to check the next arg
                if let Some(key) = Self::get_kwarg_key(arg) {
                    possible_kwarg_key = Some(key);
                    continue;
                }

                // Finally if it is not a kwarg key or key-value pair, it is a positional arg,
                // i.e. -0
            }
            ArgsContext {
                args,
                kwargs,
                pkwargs,
            }
        } else {
            ArgsContext::new()
        }
    }

    /// Returns the key if the arg represents a kwarg key, otherwise None
    fn get_kwarg_key(arg: &str) -> Option<String> {
        lazy_static! {
            static ref KWARG_KEY_REGEX: Regex = Regex::new(r"-{1,2}(?P<key>[a-zA-Z]+\w*)").unwrap();
        }
        let kwarg_match = KWARG_KEY_REGEX.captures(arg);
        if let Some(arg_match) = kwarg_match {
            let key = String::from(arg_match.name("key").unwrap().as_str());
            Some(key)
        } else {
            None
        }
    }

    /// Returns the key and value if the arg represents a kwarg key-value pair, otherwise None
    fn get_kwarg(arg: &str) -> Option<(String, String)> {
        lazy_static! {
            static ref KWARG_REGEX: Regex =
                Regex::new(r"-{1,2}(?P<key>[a-zA-Z]+\w*)=(?P<val>[\s\S]*)").unwrap();
        }
        let kwarg_match = KWARG_REGEX.captures(arg);
        if let Some(arg_match) = kwarg_match {
            let key = String::from(arg_match.name("key").unwrap().as_str());
            let val = String::from(arg_match.name("val").unwrap().as_str());
            Some((key, val))
        } else {
            None
        }
    }
}
