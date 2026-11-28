//! Shared metadata removal; the fuzz harness includes the same source directly.
use crate::error::{Error, Result};
use std::borrow::Cow;
#[cfg(fuzzing)]
#[path = "../../terminal-kit/src/media/metadata.rs"]
mod shared;
#[cfg(fuzzing)]
pub(crate) use shared::{Format, format, mp4};

pub(crate) fn strip<'a>(name: &str, data: &'a [u8]) -> Result<Cow<'a, [u8]>> {
    #[cfg(fuzzing)]
    let result = shared::strip(name, data);
    #[cfg(not(fuzzing))]
    let result =
        terminal_kit::media::metadata::strip_detected(name, data, image::guess_format(data).ok());
    result.map_err(|message| {
        Error::usage(message)
            .with_hint("save it again with another program, or upload it from the web")
    })
}
