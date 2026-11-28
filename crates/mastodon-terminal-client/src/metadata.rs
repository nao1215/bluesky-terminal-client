//! Metadata removal shared with the other clients through terminal-kit.
use crate::error::{Error, Result};
use std::borrow::Cow;
pub(crate) use terminal_kit::media::metadata::{Format, format};
pub(crate) fn strip<'a>(name: &str, data: &'a [u8]) -> Result<Cow<'a, [u8]>> {
    terminal_kit::media::metadata::strip_detected(name, data, image::guess_format(data).ok())
        .map_err(|message| {
            Error::usage(message)
                .with_hint("save it again with another program, or upload it from the web")
        })
}
