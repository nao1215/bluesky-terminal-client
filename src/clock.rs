//! Times as bsky shows them, in the client and on the command line.

use chrono::{DateTime, Local};

/// A timestamp as local time, `2026-09-20 10:00`; one that does not parse
/// is shown as it is.
pub fn local_time(ts: &str) -> String {
    match DateTime::parse_from_rfc3339(ts) {
        Ok(t) => t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string(),
        Err(_) => ts.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_is_local_minutes_and_anything_else_is_kept() {
        assert_eq!(local_time("yesterday"), "yesterday");
        assert_eq!(local_time("2026-09-22T01:02:03.000Z").len(), 16);
    }
}
