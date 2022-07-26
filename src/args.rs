use regex::Regex;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::env::Args;

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

// TODO: Implement second mode (still undefined)
/// We can run the program in two different modes, one is to run a command with args
/// amd the other mode is to run other things like help, list commands etc
pub enum YamisArgs {
    CommandArgs(CommandArgs),
}

impl YamisArgs {
    pub fn new(mut args: Args) -> YamisArgs {
        args.next(); // ignore the program name arg
        let args: Vec<String> = args.collect();
        if !args.is_empty() && args[0].starts_with('-') {
            panic!("Not implemented yet");
        }
        YamisArgs::CommandArgs(CommandArgs::new(args))
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
            if args.get(0).is_some() {
                command = Some(args.remove(0));
            }
        }

        for arg in &args {
            let arg_match = arg_regex.captures(arg);
            if let Some(arg_match) = arg_match {
                let key = String::from(arg_match.name("key").unwrap().as_str());
                let val = String::from(arg_match.name("val").unwrap().as_str());
                match kwargs.entry(key) {
                    Entry::Occupied(mut e) => {
                        e.get_mut().push(val);
                    }
                    Entry::Vacant(e) => {
                        let args_vec: Vec<String> = vec![val];
                        e.insert(args_vec);
                    }
                }
            }
        }

        kwargs.insert(String::from("*"), args);

        CommandArgs {
            file,
            task: command,
            args: kwargs,
        }
    }
}
