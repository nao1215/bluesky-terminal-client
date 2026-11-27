//! Taking the metadata off a file before it is uploaded never panics,
//! whatever the file holds: it gives back a file of the same kind or refuses
//! it. A video keeps its length, since every offset in it must stay where it
//! was, and taking the metadata off a second time changes nothing more.
//!
//! The input is the file.

#![no_main]

use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../src/error.rs"]
mod error;

#[allow(dead_code)]
#[path = "../../src/metadata.rs"]
mod metadata;

use metadata::Format;

fuzz_target!(|data: &[u8]| {
    let kind = metadata::format(data);
    let Ok(out) = metadata::strip("fuzz", data) else {
        // Only a file that says it is of a kind msky reads is refused.
        assert!(kind.is_some(), "a file of no known kind was refused");
        return;
    };
    let Some(kind) = kind else {
        assert_eq!(&*out, data, "a file of no known kind was changed");
        return;
    };
    assert_eq!(metadata::format(&out), Some(kind), "the file changed kind");
    if matches!(kind, Format::Mp4 | Format::WebM) {
        assert_eq!(out.len(), data.len(), "a {kind:?} file changed length");
    }
    // A JPEG stored sideways is turned upright and encoded again, so the
    // second pass reads another file; every other kind is only blanked.
    if kind != Format::Jpeg {
        let again = metadata::strip("fuzz", &out).expect("a stripped file is read again");
        assert_eq!(&*again, &*out, "a second pass changed a {kind:?} file");
    }

    // The MP4 pass works in place on any bytes, and is idempotent too.
    let mut mp4 = data.to_vec();
    metadata::mp4(&mut mp4);
    assert_eq!(mp4.len(), data.len());
    let mut twice = mp4.clone();
    metadata::mp4(&mut twice);
    assert_eq!(twice, mp4);
});
