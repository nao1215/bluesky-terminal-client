//! A server's answer is read leniently, whatever it holds: a status, an
//! account or a notification that parses from the bytes parses the same
//! from the JSON value they hold, and a status that is read can have the
//! status it shows and its files listed without a panic, however deep its
//! boosts.
//!
//! mstdn reads every answer as a JSON value first and the status from
//! that, so a key written twice takes its last value; the bytes alone
//! refuse such a status (serde's derive reports the duplicate), which is
//! why the relation goes from the bytes to the value only.
//!
//! The input is the JSON text of a status.

#![no_main]

use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../src/api/types.rs"]
mod types;

use types::{Account, Limits, Notification, Status};

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

    let status = same::<Status>(data, &value);
    let _ = same::<Account>(data, &value);
    let _ = same::<Notification>(data, &value);
    // The limits of a server's configuration are read from any value.
    let _ = Limits::from_instance(&value);

    let Some(status) = status else {
        return;
    };
    let shown = status.shown();
    match &status.reblog {
        Some(boosted) => assert!(std::ptr::eq(shown, &**boosted)),
        None => assert!(std::ptr::eq(shown, &status)),
    }
    let _ = shown.account.host("mastodon.social");
    // The files are those of the status shown, each counted from 1.
    let files = status.attached();
    assert_eq!(files.len(), shown.media_attachments.len());
    for (i, (owner, n, media)) in files.iter().enumerate() {
        assert!(std::ptr::eq(*owner, shown));
        assert_eq!(*n, i + 1);
        let _ = media.source();
    }
});
