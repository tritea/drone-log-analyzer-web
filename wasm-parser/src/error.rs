//! Parse errors. Kept as a plain message type so it crosses the wasm
//! boundary trivially and matches the Go error-string surface.

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
}

impl ParseError {
    pub fn new(msg: impl Into<String>) -> ParseError {
        ParseError {
            message: msg.into(),
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

impl From<String> for ParseError {
    fn from(s: String) -> ParseError {
        ParseError { message: s }
    }
}

impl From<&str> for ParseError {
    fn from(s: &str) -> ParseError {
        ParseError {
            message: s.to_string(),
        }
    }
}

pub type Result<T> = std::result::Result<T, ParseError>;
