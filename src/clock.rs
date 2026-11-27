//! Times as msky shows them.

use jiff::Timestamp;
use jiff::tz::TimeZone;

/// A timestamp as local time, `2026-09-20 10:00`; one that does not parse
/// is shown as it is, less any control character: it is whatever the
/// server wrote, and an escape sequence in it must not reach the terminal.
pub fn local_time(ts: &str) -> String {
    local_time_in(ts, &TimeZone::system())
}

/// [`local_time`] in `tz`.
fn local_time_in(ts: &str, tz: &TimeZone) -> String {
    match ts.parse::<Timestamp>() {
        Ok(t) => {
            let t = t.to_zoned_datetime(tz.clone());
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}",
                t.year(),
                t.month(),
                t.day(),
                t.hour(),
                t.minute()
            )
        }
        Err(_) => ts.chars().filter(|c| !c.is_control()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::tz::offset;
    use rstest::rstest;

    #[rstest]
    #[case("2026-09-22T01:02:03.000Z", 0, "2026-09-22 01:02")]
    #[case("2026-09-22T01:02:03.000Z", 9, "2026-09-22 10:02")]
    #[case("2026-12-31T20:00:00Z", 9, "2027-01-01 05:00")]
    #[case("0999-01-02T23:59:59+09:00", 0, "0999-01-02 14:59")]
    #[case("2024-02-29T12:00:00-11:30", 0, "2024-02-29 23:30")]
    fn a_time_is_put_in_the_time_zone_to_the_minute(
        #[case] ts: &str,
        #[case] hours: i8,
        #[case] want: &str,
    ) {
        assert_eq!(local_time_in(ts, &TimeZone::fixed(offset(hours))), want);
    }

    #[test]
    fn a_time_is_local_minutes() {
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
