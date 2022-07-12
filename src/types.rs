use std::{error, result};

/// Alias the result type for convenience. We simply return a dynamic error as these should
/// be displayed to the user as they are.
pub type DynErrResult<T> = Result<T, Box<dyn error::Error>>;