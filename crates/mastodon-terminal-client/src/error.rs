//! Error type shared by every layer, and the exit code each failure class maps to.
//!
//! mstdn prints failures in one shape, `error: <message>` followed by an optional
//! `hint: <next step>`, and exits with a code that tells scripts which class of
//! failure happened:
//!
//! | code | class     | examples                                                         |
//! |------|-----------|------------------------------------------------------------------|
//! | 0    | success   |                                                                  |
//! | 1    | usage     | unknown flag, malformed `--host`, a post or account not written as one |
//! | 3    | local I/O | the output or a download cannot be written                       |
//! | 4    | API       | a request failed on the network or the server said no            |
//!
//! The codes are bsky's and msky's. Their 2, a terminal that cannot host
//! their full-screen client, is not used: mstdn has no such client.

use std::fmt;

/// The class of a failure; it decides the process exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// The command line cannot be acted on.
    Usage,
    /// A local file could not be read or written.
    Io,
    /// The Mastodon server could not be reached or refused the request.
    Api,
}

impl Kind {
    /// Process exit code for this class.
    pub(crate) fn exit_code(self) -> u8 {
        match self {
            Kind::Usage => 1,
            Kind::Io => 3,
            Kind::Api => 4,
        }
    }
}

/// A failure with a user-facing message and an optional hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Error {
    kind: Kind,
    message: String,
    hint: Option<String>,
}

impl Error {
    /// Build an error of the given class.
    pub(crate) fn new(kind: Kind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            hint: None,
        }
    }

    /// Attach the next step the user can take.
    #[must_use]
    pub(crate) fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// The failure class.
    pub(crate) fn kind(&self) -> Kind {
        self.kind
    }

    /// The message without the `error:` prefix.
    pub(crate) fn message(&self) -> &str {
        &self.message
    }

    /// The next step the user can take, if there is one.
    pub(crate) fn hint(&self) -> Option<&str> {
        self.hint.as_deref()
    }

    /// Shorthand for [`Kind::Api`].
    pub(crate) fn api(message: impl Into<String>) -> Self {
        Self::new(Kind::Api, message)
    }

    /// Shorthand for [`Kind::Io`].
    pub(crate) fn io(message: impl Into<String>) -> Self {
        Self::new(Kind::Io, message)
    }

    /// Shorthand for [`Kind::Usage`].
    pub(crate) fn usage(message: impl Into<String>) -> Self {
        Self::new(Kind::Usage, message)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error: {}", self.message)?;
        if let Some(hint) = &self.hint {
            write!(f, "\nhint: {hint}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

/// Result alias used across the crate.
pub(crate) type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_puts_the_hint_on_its_own_line() {
        let err = Error::api("no such user").with_hint("check --host");
        assert_eq!(err.to_string(), "error: no such user\nhint: check --host");
    }

    #[test]
    fn display_without_hint_is_one_line() {
        assert_eq!(Error::io("disk").to_string(), "error: disk");
    }

    #[test]
    fn exit_codes_are_distinct_and_nonzero() {
        let codes = [Kind::Usage, Kind::Io, Kind::Api].map(Kind::exit_code);
        assert_eq!(codes, [1, 3, 4]);
    }
}
