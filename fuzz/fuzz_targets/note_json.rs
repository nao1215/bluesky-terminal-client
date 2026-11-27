//! A server's answer is read leniently, whatever it holds: a note, an
//! account or a notification that parses from the bytes parses the same from
//! the JSON value they hold, and a note that is read can be shown, counted
//! and have its files listed without panicking, however large its counts or
//! deep its renotes.
//!
//! The input is the JSON text of a note.

#![no_main]

use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../src/api/types.rs"]
mod types;

use types::{Note, Notification, User};

fuzz_target!(|data: &[u8]| {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) else {
        return;
    };

    let note = serde_json::from_slice::<Note>(data).ok();
    assert_eq!(note, serde_json::from_value::<Note>(value.clone()).ok());
    let user = serde_json::from_slice::<User>(data).ok();
    assert_eq!(user, serde_json::from_value::<User>(value.clone()).ok());
    let notification = serde_json::from_slice::<Notification>(data).ok();
    assert_eq!(
        notification,
        serde_json::from_value::<Notification>(value).ok()
    );

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
