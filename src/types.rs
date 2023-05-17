use std::error;

/// Alias the result type for convenience. We simply return a dynamic error as these should
/// be displayed to the user as they are.
pub(crate) type DynErrResult<T> = Result<T, Box<dyn error::Error>>;
