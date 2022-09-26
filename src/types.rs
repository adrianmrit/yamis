use std::collections::HashMap;
use std::error;

/// Alias the result type for convenience. We simply return a dynamic error as these should
/// be displayed to the user as they are.
pub(crate) type DynErrResult<T> = Result<T, Box<dyn error::Error>>;

/// Extra args passed that will be mapped to the task.
pub(crate) type TaskArgs = HashMap<String, Vec<String>>;
