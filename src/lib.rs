//! A deliberately small Ruby-to-GW-BASIC transpiler.
//!
//! See the README for the supported Ruby subset and target limitations.

mod compiler;
mod lexer;
mod parser;

use std::fmt;

/// A source error. Lines and columns are one-based.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl Error {
    pub(crate) fn new(line: usize, column: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            column,
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for Error {}

impl lexer::Token {
    pub(crate) fn error(&self, message: impl Into<String>) -> Error {
        Error::new(self.line, self.column, message)
    }
}

/// Convert Ruby source into a complete, numbered GW-BASIC listing.
///
/// Unsupported syntax and unsafe type/variable usage return a source error.
pub fn convert(source: &str) -> Result<String, Error> {
    compiler::compile(&parser::parse(lexer::lex(source)?)?)
}
