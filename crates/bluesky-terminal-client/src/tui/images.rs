//! Shared image renderer, with Bluesky avatar addresses.
pub(crate) use terminal_kit::media::images::{CACHE_BYTES, DiskCache, Images};

/// The small version of a Bluesky avatar: a list draws avatars 4 cells
/// wide, and the full-size one is many times the bytes. Other URLs are kept.
pub(crate) fn small_avatar(url: &str) -> std::borrow::Cow<'_, str> {
    if url.contains("/img/avatar/") {
        url.replacen("/img/avatar/", "/img/avatar_thumbnail/", 1)
            .into()
    } else {
        url.into()
    }
}
