//! Reading the video track of an MP4 a note carries never panics and never
//! spins, whatever the boxes say about their sizes and counts. A track it
//! reads has a picture, and its SPS and PPS and every sample are Annex B
//! (each NAL unit after a start code), the form OpenH264 takes.
//!
//! The input is the file.

#![no_main]

use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../../terminal-kit/src/media/mp4.rs"]
mod mp4;

const START: [u8; 4] = [0, 0, 0, 1];

fuzz_target!(|data: &[u8]| {
    let Ok(track) = mp4::video_track(data) else {
        return;
    };
    assert!(!track.samples.is_empty(), "a track without a picture");
    assert!(
        track.config.is_empty() || track.config.starts_with(&START),
        "the SPS and PPS are not Annex B"
    );
    for sample in &track.samples {
        assert!(
            sample.data.is_empty() || sample.data.starts_with(&START),
            "a sample is not Annex B"
        );
    }
});
