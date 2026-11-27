//! Reading input events: crossterm's `poll` and `read`.
//!
//! bsky reads the Windows console itself before crossterm does, because
//! crossterm 0.29 drops every character outside the Basic Multilingual
//! Plane there (an emoji typed into a field). The client of this version
//! has no field to type into, so the keys it reads are all in that plane;
//! bsky's reader comes with the first one.

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event};

/// The next event, waiting at most `wait` for one.
pub fn next(wait: Duration) -> io::Result<Option<Event>> {
    if event::poll(wait)? {
        event::read().map(Some)
    } else {
        Ok(None)
    }
}
