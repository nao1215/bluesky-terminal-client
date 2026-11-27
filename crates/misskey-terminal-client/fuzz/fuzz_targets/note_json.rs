//! A server's answer is read leniently, whatever it holds: a note, an
//! account or a notification that parses from the bytes parses the same from
//! the JSON value they hold, and a note that is read can be shown, counted
//! and have its files listed without panicking, however large its counts or
//! deep its renotes.
//!
//! msky reads every answer as a JSON value first and the note from that, so
//! a key written twice takes its last value, as a browser reads it; the
//! bytes alone refuse such a note (serde's derive reports the duplicate),
//! which is why the relation goes from the bytes to the value only.
//!
//! The input is the JSON text of a note.

#![no_main]

use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../src/api/types.rs"]
mod types;

use types::{Note, Notification, User};

/// What parses from the bytes parses the same from the value.
fn same<T>(data: &[u8], value: &serde_json::Value) -> Option<T>
where
    T: serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let from_value = serde_json::from_value::<T>(value.clone()).ok();
    if let Ok(from_bytes) = serde_json::from_slice::<T>(data) {
        assert_eq!(Some(&from_bytes), from_value.as_ref());
    }
    from_value
}

fuzz_target!(|data: &[u8]| {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) else {
        return;
    };

    let note = same::<Note>(data, &value);
    let _ = same::<User>(data, &value);
    let _ = same::<Notification>(data, &value);

    let Some(note) = note else {
        return;
    };
    let shown = note.shown();
    let _ = shown.author_id();
    let _ = shown.reaction_total();
    let _ = note.reaction_total();
    let files = note.attached();
    // Its own files come first, each counted from 1.
    assert!(files.len() >= note.files.len());
    for (i, (owner, n, _)) in files.iter().take(note.files.len()).enumerate() {
        assert!(std::ptr::eq(*owner, &note));
        assert_eq!(*n, i + 1);
    }
    for file in &note.files {
        let _ = file.is_picture();
        let _ = file.aspect();
    }
});
