//! Failures use the workspace's usage, local I/O, network and unsupported statuses.

use serde::Serialize;
use terminal_kit::error::Kind;

#[derive(Debug, Serialize)]
pub(crate) struct Error {
    pub(crate) error: String,
    pub(crate) hint: Option<String>,
    pub(crate) status: u8,
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub(crate) fn unavailable(message: impl ToString) -> Self {
        Self {
            error: message.to_string(),
            hint: None,
            status: 5,
        }
    }
    pub(crate) fn usage(message: impl ToString) -> Self {
        Self {
            error: message.to_string(),
            hint: Some("run `nstr --help` for usage".into()),
            status: Kind::Usage.exit_code(),
        }
    }

    pub(crate) fn file(message: impl ToString) -> Self {
        Self {
            error: message.to_string(),
            hint: None,
            status: Kind::Io.exit_code(),
        }
    }

    pub(crate) fn unsupported(message: impl ToString) -> Self {
        Self::unavailable(message)
    }

    pub(crate) fn network(message: impl ToString) -> Self {
        Self {
            error: message.to_string(),
            hint: None,
            status: Kind::Api.exit_code(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        terminal_kit::error::write_error(f, &self.error, self.hint.as_deref())
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_json_names_the_feature_and_uses_shared_status_five() {
        let error = Error::unsupported("unsupported: NIP-50 search");
        assert_eq!(error.to_string(), "error: unsupported: NIP-50 search");
        let json = serde_json::to_value(error).unwrap();
        assert_eq!(json["status"], 5);
        assert_eq!(json["hint"], serde_json::Value::Null);
    }
}
