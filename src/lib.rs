extern crate core;

use std::env;
use std::env::Args;
use std::error::Error;

use crate::args::YamisArgs;
use crate::tasks::ConfigFiles;

pub mod args;
pub mod tasks;
