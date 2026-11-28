//! Reading an HLS playlist never panics, the variant chosen is one the
//! master playlist lists (the lightest to play, the heaviest to save), every
//! segment is its playlist line resolved against the playlist's URL, and an
//! absolute URL resolves to itself.
//!
//! The first input line is the playlist's URL; the rest is the playlist.

#![no_main]

use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../src/hls.rs"]
mod hls;

/// The variants of a master playlist as the HLS spec lists them: each
/// `#EXT-X-STREAM-INF` with its bandwidth (0 when missing or unreadable) and
/// the URI on the next line that is neither blank nor a tag.
fn variants(text: &str, base: &str) -> Vec<(u64, String)> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        i += 1;
        let Some(attrs) = line.strip_prefix("#EXT-X-STREAM-INF:") else {
            continue;
        };
        let bandwidth = attrs
            .split(',')
            .find_map(|a| a.strip_prefix("BANDWIDTH="))
            .and_then(|b| b.parse().ok())
            .unwrap_or(0);
        while i < lines.len() {
            let uri = lines[i];
            i += 1;
            if !uri.is_empty() && !uri.starts_with('#') {
                out.push((bandwidth, hls::resolve(base, uri)));
                break;
            }
        }
    }
    out
}

/// RFC 3986 section 3.1: a letter, then letters, digits, `+`, `-` or `.`,
/// then a colon.
fn has_scheme(url: &str) -> bool {
    let Some((scheme, _)) = url.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);
    let (base, text) = input.split_once('\n').unwrap_or((&input, ""));

    let listed = variants(text, base);
    let play = hls::pick_variant(text, base);
    let save = hls::pick_best_variant(text, base);
    assert_eq!(play.is_some(), !listed.is_empty());
    assert_eq!(save.is_some(), !listed.is_empty());
    if let Some(play) = &play {
        // The lightest stream that states its bandwidth, or any when none does.
        let key = |b: u64| if b == 0 { u64::MAX } else { b };
        let least = listed.iter().map(|(b, _)| key(*b)).min().unwrap();
        assert!(
            listed.iter().any(|(b, u)| key(*b) == least && u == play),
            "{play:?} is not a lightest variant of {listed:?}"
        );
    }
    if let Some(save) = &save {
        let most = listed.iter().map(|(b, _)| *b).max().unwrap();
        assert!(
            listed.iter().any(|(b, u)| *b == most && u == save),
            "{save:?} is not a heaviest variant of {listed:?}"
        );
    }

    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let segments = hls::segments(text, base);
    assert_eq!(segments.len(), lines.len());
    for (segment, line) in segments.iter().zip(&lines) {
        assert_eq!(*segment, hls::resolve(base, line));
        if has_scheme(line) {
            assert_eq!(segment, line, "an absolute URL changed");
        }
    }

    // Whatever follows the scheme, an absolute URL is taken as it is.
    for line in text.lines() {
        let absolute = format!("https://{line}");
        assert_eq!(hls::resolve(base, &absolute), absolute);
    }
});
