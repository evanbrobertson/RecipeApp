//! Invalid input, as the pure logic reports it. The server maps it to a 400.

use std::fmt;

/// Invalid input from a client. The server turns it into a 400.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError(pub String);

pub type ValidationResult<T> = Result<T, ValidationError>;

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ValidationError {}
