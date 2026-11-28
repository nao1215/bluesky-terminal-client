//! Failures use the workspace's usage, local I/O and network exit statuses.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct Error {
    pub(crate) error: String,
    pub(crate) hint: Option<String>,
    pub(crate) status: u8,
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub(crate) fn usage(message: impl ToString) -> Self {
        Self {
            error: message.to_string(),
            hint: Some("run `nstr --help` for usage".into()),
            status: 1,
        }
    }

    pub(crate) fn file(message: impl ToString) -> Self {
        Self {
            error: message.to_string(),
            hint: None,
            status: 3,
        }
    }

    pub(crate) fn network(message: impl ToString) -> Self {
        Self {
            error: message.to_string(),
            hint: None,
            status: 4,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error: {}", self.error)?;
        if let Some(next) = &self.hint {
            write!(f, "\nhint: {next}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}
