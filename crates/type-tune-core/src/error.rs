//! Error types returned by native type-tune operations.

use std::fmt::{Display, Formatter};

/// Error raised when native analysis cannot safely continue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeTuneError {
    message: String,
}

impl TypeTuneError {
    /// Creates an error with a user-facing message.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for TypeTuneError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for TypeTuneError {}

/// Convenience result type for native type-tune operations.
pub type Result<T> = std::result::Result<T, TypeTuneError>;

#[cfg(test)]
mod tests {
    use super::TypeTuneError;

    #[test]
    fn exposes_the_original_message() {
        assert_eq!(
            TypeTuneError::new("invalid plan").to_string(),
            "invalid plan"
        );
    }
}
