//! Reading input events.
//!
//! Unix uses crossterm's `use-dev-tty` reader (level-triggered poll). Its
//! default mio reader can return a resize and discard a simultaneously
//! ready key's edge, leaving that key unread until another key arrives.
//!
//! Everywhere but Windows this is crossterm's `poll` and `read`. On Windows,
//! crossterm 0.29 drops every character outside the Basic Multilingual Plane
//! (emoji, most of all): a terminal sends such a character as two UTF-16
//! surrogates, each as a key-down and a key-up record, and crossterm pairs a
//! surrogate's key-up with its own key-down, fails to decode the pair, and
//! throws both away. So on Windows this module takes the records of
//! surrogates off the console input before crossterm reads them, assembles
//! the characters from the key-down records, and leaves every other record
//! to crossterm.

use std::io;
use std::time::Duration;

#[cfg(not(windows))]
use crossterm::event;
use crossterm::event::Event;

#[cfg(all(test, target_os = "linux"))]
mod unix_tests;

/// The input events of the terminal the program runs in.
pub struct Input {
    #[cfg(windows)]
    console: windows::Console,
}

impl Input {
    /// Starts reading the terminal's input; on Windows it opens the console.
    pub fn new() -> io::Result<Input> {
        // Install crossterm's lazy SIGWINCH handler before the first frame: a resize between that frame and the first next() would otherwise be lost. Polling leaves any queued input for next().
        #[cfg(not(windows))]
        event::poll(Duration::ZERO)?;
        Ok(Input {
            #[cfg(windows)]
            console: windows::Console::new()?,
        })
    }

    /// Handle at most `limit` events, waiting only for the first. A false
    /// callback result stops immediately; the return value is the number handled.
    pub fn drain(
        &mut self,
        limit: usize,
        wait: Duration,
        handle: impl FnMut(Event) -> bool,
    ) -> io::Result<usize> {
        drain(limit, wait, |wait| self.next(wait), handle)
    }

    /// The next event, waiting at most `wait` for one. Unix rounds the timeout
    /// up to 1 ms; already available events return immediately.
    pub fn next(&mut self, wait: Duration) -> io::Result<Option<Event>> {
        #[cfg(windows)]
        return self.console.next(wait);
        #[cfg(not(windows))]
        // The use-dev-tty reader skips polling altogether for a zero timeout,
        // including its buffered events. Give it a positive millisecond (the
        // underlying poll's resolution); ready input still returns immediately.
        return if event::poll(wait.max(Duration::from_millis(1)))? {
            event::read().map(Some)
        } else {
            Ok(None)
        };
    }
}

fn drain<T>(
    limit: usize,
    mut wait: Duration,
    mut next: impl FnMut(Duration) -> io::Result<Option<T>>,
    mut handle: impl FnMut(T) -> bool,
) -> io::Result<usize> {
    let mut count = 0;
    for _ in 0..limit {
        let Some(event) = next(wait)? else {
            break;
        };
        count += 1;
        if !handle(event) {
            break;
        }
        wait = Duration::ZERO;
    }
    Ok(count)
}

#[cfg(test)]
mod drain_tests {
    use super::*;
    #[test]
    fn waits_once_limits_work_and_stops_on_quit() {
        for (limit, stop, expected) in [(0, 99, 0), (3, 99, 3), (20, 2, 2)] {
            let mut waits = Vec::new();
            let mut handled = 0;
            let count = drain(
                limit,
                Duration::from_millis(50),
                |wait| {
                    waits.push(wait);
                    Ok(Some(()))
                },
                |_| {
                    handled += 1;
                    handled < stop
                },
            )
            .unwrap();
            assert_eq!(count, expected);
            assert_eq!(handled, expected);
            if let Some(first) = waits.first() {
                assert_eq!(*first, Duration::from_millis(50));
                assert!(waits.iter().skip(1).all(Duration::is_zero));
            }
        }
    }
}

/// Whether a UTF-16 code unit is half of a surrogate pair.
#[cfg_attr(
    not(windows),
    allow(
        dead_code,
        reason = "only the Windows console reader uses it; tests on every system do"
    )
)]
fn is_surrogate(unit: u16) -> bool {
    (0xD800..=0xDFFF).contains(&unit)
}

/// Pairs the surrogates of key records into characters. Only key-down
/// records count: a key-up repeats the unit its key-down carried, so the
/// order high-down, high-up, low-down, low-up that Windows terminals send
/// and the order high-down, low-down, high-up, low-up both give the
/// character once.
#[derive(Debug, Default)]
#[cfg_attr(
    not(windows),
    allow(
        dead_code,
        reason = "only the Windows console reader uses it; tests on every system do"
    )
)]
struct Surrogates {
    high: Option<u16>,
}

#[cfg_attr(
    not(windows),
    allow(
        dead_code,
        reason = "only the Windows console reader uses it; tests on every system do"
    )
)]
impl Surrogates {
    /// The character a record completes, if any. A low surrogate without a
    /// high one before it is dropped, as is a high one another high one
    /// replaces: neither is a character.
    fn feed(&mut self, key_down: bool, unit: u16) -> Option<char> {
        if !key_down {
            return None;
        }
        if (0xD800..=0xDBFF).contains(&unit) {
            self.high = Some(unit);
            return None;
        }
        let high = self.high.take()?;
        char::decode_utf16([high, unit]).next()?.ok()
    }
}

#[cfg(windows)]
mod windows {
    use std::collections::VecDeque;
    use std::io;
    use std::time::{Duration, Instant};

    use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
    use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE, WAIT_FAILED};
    use windows_sys::Win32::System::Console::{
        GetNumberOfConsoleInputEvents, GetStdHandle, INPUT_RECORD, KEY_EVENT, PeekConsoleInputW,
        ReadConsoleInputW, STD_INPUT_HANDLE,
    };
    use windows_sys::Win32::System::Threading::WaitForSingleObject;

    use super::{Surrogates, is_surrogate};

    pub(crate) struct Console {
        handle: HANDLE,
        surrogates: Surrogates,
        /// Characters assembled and not yet handed out.
        ready: VecDeque<char>,
    }

    impl Console {
        pub(crate) fn new() -> io::Result<Console> {
            // SAFETY: GetStdHandle has no preconditions.
            let handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
            if handle.is_null() || handle == INVALID_HANDLE_VALUE {
                return Err(io::Error::last_os_error());
            }
            Ok(Console {
                handle,
                surrogates: Surrogates::default(),
                ready: VecDeque::new(),
            })
        }

        pub(crate) fn next(&mut self, wait: Duration) -> io::Result<Option<Event>> {
            let deadline = Instant::now() + wait;
            loop {
                self.take_surrogates()?;
                if let Some(c) = self.ready.pop_front() {
                    return Ok(Some(Event::Key(KeyEvent::new(
                        KeyCode::Char(c),
                        KeyModifiers::NONE,
                    ))));
                }
                if self.pending()? > 0 {
                    // The first record is not a surrogate's, and crossterm
                    // reads one record per zero-timeout poll, so it never
                    // reaches the surrogates behind it.
                    if event::poll(Duration::ZERO)? {
                        return event::read().map(Some);
                    }
                    continue;
                }
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Ok(None);
                }
                let ms = u32::try_from(left.as_millis().max(1)).unwrap_or(u32::MAX);
                // SAFETY: the handle is the console input, open for the
                // life of the process.
                if unsafe { WaitForSingleObject(self.handle, ms) } == WAIT_FAILED {
                    return Err(io::Error::last_os_error());
                }
            }
        }

        /// Read the key records of surrogates at the head of the input, as
        /// long as there are some, into `ready`.
        fn take_surrogates(&mut self) -> io::Result<()> {
            loop {
                // SAFETY: INPUT_RECORD is plain data; all zeros is valid.
                let mut record: INPUT_RECORD = unsafe { std::mem::zeroed() };
                let mut n = 0u32;
                // SAFETY: one record's room, and `n` says how many came.
                if unsafe { PeekConsoleInputW(self.handle, &mut record, 1, &mut n) } == 0 {
                    return Err(io::Error::last_os_error());
                }
                if n == 0 || u32::from(record.EventType) != KEY_EVENT {
                    return Ok(());
                }
                // SAFETY: EventType says the union holds a key record.
                let key = unsafe { record.Event.KeyEvent };
                // SAFETY: both members of the union are a u16.
                let unit = unsafe { key.uChar.UnicodeChar };
                if !is_surrogate(unit) {
                    return Ok(());
                }
                // SAFETY: as for the peek; this removes the same record.
                if unsafe { ReadConsoleInputW(self.handle, &mut record, 1, &mut n) } == 0 {
                    return Err(io::Error::last_os_error());
                }
                if let Some(c) = self.surrogates.feed(key.bKeyDown != 0, unit) {
                    self.ready.push_back(c);
                }
            }
        }

        fn pending(&self) -> io::Result<u32> {
            let mut n = 0u32;
            // SAFETY: `n` is a valid place for the count.
            if unsafe { GetNumberOfConsoleInputEvents(self.handle, &mut n) } == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(n)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The records of `text` in the order Windows terminals send them: a
    /// key-down and a key-up per UTF-16 code unit.
    fn records(text: &str) -> Vec<(bool, u16)> {
        text.encode_utf16()
            .flat_map(|u| [(true, u), (false, u)])
            .collect()
    }

    fn assemble(records: &[(bool, u16)]) -> String {
        let mut s = Surrogates::default();
        records
            .iter()
            .filter(|(_, u)| is_surrogate(*u))
            .filter_map(|&(down, u)| s.feed(down, u))
            .collect()
    }

    #[test]
    fn down_and_up_records_of_each_half_give_the_character_once() {
        assert_eq!(assemble(&records("😀")), "😀");
        assert_eq!(assemble(&records("🇯🇵👍🏽")), "🇯🇵👍🏽");
    }

    #[test]
    fn both_downs_before_both_ups_give_the_character_once() {
        let [high, low] = [0xD83D, 0xDE00];
        let got = assemble(&[(true, high), (true, low), (false, high), (false, low)]);
        assert_eq!(got, "😀");
    }

    #[test]
    fn a_family_keeps_its_people_and_the_joiners_are_left_to_crossterm() {
        // The joiners are in the BMP: they stay in the input for crossterm,
        // and only the people outside it are assembled here.
        let family = "👨\u{200d}👩\u{200d}👧";
        assert_eq!(assemble(&records(family)), "👨👩👧");
    }

    #[test]
    fn a_half_without_its_other_half_is_dropped() {
        assert_eq!(assemble(&[(true, 0xDE00), (false, 0xDE00)]), "");
        // A high surrogate another replaces is lost; the new pair is whole.
        let got = assemble(&[(true, 0xD83D), (true, 0xD83D), (true, 0xDE00)]);
        assert_eq!(got, "😀");
    }
}
