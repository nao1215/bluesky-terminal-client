//! The MPEG-TS demuxer never panics, and how a download is cut into pieces
//! does not change what it reads: the whole stream fed at once and the same
//! bytes fed in two pieces give the same access units and the same verdict
//! on the video codec.
//!
//! The first two input bytes pick the split point; the rest is the stream.

#![no_main]

use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../src/hls.rs"]
mod hls;

use hls::{AccessUnit, Demuxer};

/// Everything the demuxer gives for `pieces`, fed in order, with a `take`
/// after each as the player does, and whether it found unsupported video.
fn demux(pieces: &[&[u8]]) -> (Vec<AccessUnit>, Option<u8>) {
    let mut d = Demuxer::new();
    let mut units = Vec::new();
    for piece in pieces {
        d.feed(piece);
        units.extend(d.take());
    }
    units.extend(d.finish());
    // Once finished, nothing is left over.
    assert!(d.finish().is_empty());
    (units, d.unsupported)
}

fuzz_target!(|data: &[u8]| {
    let Some((selector, stream)) = data.split_first_chunk::<2>() else {
        return;
    };
    let at = usize::from(u16::from_le_bytes(*selector)) % (stream.len() + 1);
    let (head, tail) = stream.split_at(at);

    let whole = demux(&[stream]);
    let split = demux(&[head, tail]);
    assert_eq!(whole, split, "split at {at} of {}", stream.len());

    // An access unit is never empty: an empty PES is not a frame.
    assert!(whole.0.iter().all(|u| !u.data.is_empty()));
});
