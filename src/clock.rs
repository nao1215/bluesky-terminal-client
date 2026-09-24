//! Times as bsky shows them, in the client and on the command line.

use chrono::{DateTime, Local};

/// A timestamp as local time, `2026-09-20 10:00`; one that does not parse
/// is shown as it is, less any control character: it is whatever the
/// record's author wrote, and an escape sequence in it must not reach the
/// terminal.
pub fn local_time(ts: &str) -> String {
    match DateTime::parse_from_rfc3339(ts) {
        Ok(t) => t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string(),
        Err(_) => ts.chars().filter(|c| !c.is_control()).collect(),
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

    #[test]
    fn a_time_that_does_not_parse_loses_its_control_characters() {
        assert_eq!(
            local_time("\u{1b}]52;c;cHduZWQ=\u{7}\u{1b}[2J\nnext 👨‍👩‍👧\u{9b}"),
            "]52;c;cHduZWQ=[2Jnext 👨‍👩‍👧"
        );
    }
}
