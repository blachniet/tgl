//! Defines the error type for top-level CLI commands.

/// Error type for top-level CLI commands.
#[derive(Debug)]
pub struct Error {
    pub message: String,
}

impl Error {
    /// Creates a new entry with the given message.
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Self { message }
    }
}

impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}
