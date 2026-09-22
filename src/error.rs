//! Error type shared by every layer, and the exit code each failure class maps to.
//!
//! bs prints failures in one shape, `error: <message>` followed by an optional
//! `hint: <next step>`, and exits with a code that tells scripts which class of
//! failure happened:
//!
//! | code | class     | examples                                                       |
//! |------|-----------|----------------------------------------------------------------|
//! | 0    | success   |                                                                |
//! | 1    | usage     | unknown flag, malformed `--service` URL, unknown `BS_GRAPHICS` |
//! | 2    | terminal  | stdin or stdout is not a terminal, no image protocol           |
//! | 3    | local I/O | session file unreadable, corrupt, or unwritable                |
//!
//! [`Kind::Api`] classifies network and server failures inside the client,
//! which shows them and keeps running; no command exits with them today, so
//! their code (4) is reserved rather than documented.

use std::fmt;

/// The class of a failure; it decides the process exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The command line cannot be acted on.
    Usage,
    /// The terminal cannot host the client.
    Terminal,
    /// A local file could not be read or written.
    Io,
    /// The Bluesky server could not be reached or refused the request.
    Api,
}

impl Kind {
    /// Process exit code for this class.
    pub fn exit_code(self) -> u8 {
        match self {
            Kind::Usage => 1,
            Kind::Terminal => 2,
            Kind::Io => 3,
            Kind::Api => 4,
        }
    }
}

/// A failure with a user-facing message and an optional hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    kind: Kind,
    message: String,
    hint: Option<String>,
}

impl Error {
    /// Build an error of the given class.
    pub fn new(kind: Kind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            hint: None,
        }
    }

    /// Attach the next step the user can take.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// The failure class.
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// The message without the `error:` prefix.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The next step the user can take, if there is one.
    pub fn hint(&self) -> Option<&str> {
        self.hint.as_deref()
    }

    /// Shorthand for [`Kind::Api`].
    pub fn api(message: impl Into<String>) -> Self {
        Self::new(Kind::Api, message)
    }

    /// Shorthand for [`Kind::Io`].
    pub fn io(message: impl Into<String>) -> Self {
        Self::new(Kind::Io, message)
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
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_puts_the_hint_on_its_own_line() {
        let err = Error::new(Kind::Terminal, "no images").with_hint("use kitty");
        assert_eq!(err.to_string(), "error: no images\nhint: use kitty");
    }

    #[test]
    fn display_without_hint_is_one_line() {
        assert_eq!(Error::io("disk").to_string(), "error: disk");
    }

    #[test]
    fn exit_codes_are_distinct_and_nonzero() {
        let codes = [Kind::Usage, Kind::Terminal, Kind::Io, Kind::Api].map(Kind::exit_code);
        assert_eq!(codes, [1, 2, 3, 4]);
    }
}
