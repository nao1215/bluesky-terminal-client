//! Shared command errors, exit statuses and error layout.

use std::fmt;

/// The class of a failure; it decides the process exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The command line cannot be acted on.
    Usage,
    /// The terminal cannot host the full-screen client.
    Terminal,
    /// A local file could not be read or written.
    Io,
    /// The server could not be reached or refused the request.
    Api,
    /// The server does not provide the requested feature.
    Unsupported,
}

impl Kind {
    /// Process exit code for this class.
    pub fn exit_code(self) -> u8 {
        match self {
            Kind::Usage => 1,
            Kind::Terminal => 2,
            Kind::Io => 3,
            Kind::Api => 4,
            Kind::Unsupported => 5,
        }
    }
}

/// A failure with a user-facing message and an optional hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    kind: Kind,
    message: String,
    hint: Option<String>,
    api_code: Option<String>,
}

impl Error {
    /// Build an error of the given class.
    pub fn new(kind: Kind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            hint: None,
            api_code: None,
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

    /// Preserve the server's structured code for capability decisions.
    pub fn with_api_code(mut self, code: String) -> Self {
        self.api_code = Some(code);
        self
    }

    /// The structured error code, when the server supplied one.
    pub fn api_code(&self) -> Option<&str> {
        self.api_code.as_deref()
    }

    /// Shorthand for [`Kind::Api`].
    pub fn api(message: impl Into<String>) -> Self {
        Self::new(Kind::Api, message)
    }

    /// Shorthand for [`Kind::Io`].
    pub fn io(message: impl Into<String>) -> Self {
        Self::new(Kind::Io, message)
    }

    /// Shorthand for [`Kind::Usage`].
    pub fn usage(message: impl Into<String>) -> Self {
        Self::new(Kind::Usage, message)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_error(f, &self.message, self.hint.as_deref())
    }
}

impl std::error::Error for Error {}

/// Write the common error layout without requiring a client to change its JSON schema.
pub fn write_error(f: &mut fmt::Formatter<'_>, message: &str, hint: Option<&str>) -> fmt::Result {
    write!(f, "error: {message}")?;
    if let Some(hint) = hint {
        write!(f, "\nhint: {hint}")?;
    }
    Ok(())
}

/// Result alias used across the crate.
pub type Result<T> = std::result::Result<T, Error>;

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
        let codes = [
            Kind::Usage,
            Kind::Terminal,
            Kind::Io,
            Kind::Api,
            Kind::Unsupported,
        ]
        .map(Kind::exit_code);
        assert_eq!(codes, [1, 2, 3, 4, 5]);
    }
}
