//! What a file says about where, when and with what it was made, taken
//! off before it is uploaded, as bsky does: a photo's EXIF (the place it
//! was taken among it), XMP, IPTC and comments, and a video's user data,
//! metadata and location boxes. The picture itself is left as it is: a
//! JPEG keeps its bytes but for those segments, except that one the
//! camera stored sideways is turned upright first (its orientation is in
//! the EXIF that goes), and an MP4's boxes become `free` boxes of the same
//! size, so every offset in it stays where it was.
//!
//! A file of another kind is uploaded as it is; one that says it is a
//! JPEG, PNG, WebP, GIF, MP4 or WebM but cannot be read as one is refused,
//! since what it carries cannot be known.

use std::borrow::Cow;

use crate::error::{Error, Result};

/// The kinds of file whose metadata is taken off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Format {
    Jpeg,
    Png,
    WebP,
    Gif,
    /// MP4, QuickTime, 3GP: ISO base media boxes.
    Mp4,
    /// WebM and Matroska: EBML.
    WebM,
}

/// What `data` is, by its first bytes.
pub(crate) fn format(data: &[u8]) -> Option<Format> {
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(Format::Jpeg)
    } else if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(Format::Png)
    } else if data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some(Format::WebP)
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Some(Format::Gif)
    } else if data.len() >= 8 && &data[4..8] == b"ftyp" {
        Some(Format::Mp4)
    } else if data.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        Some(Format::WebM)
    } else {
        None
    }
}

/// `data` (the file `name`) without its metadata.
pub(crate) fn strip<'a>(name: &str, data: &'a [u8]) -> Result<Cow<'a, [u8]>> {
    let Some(f) = format(data) else {
        return Ok(Cow::Borrowed(data));
    };
    let out = match f {
        Format::Jpeg => jpeg(data),
        Format::Png => png(data),
        Format::WebP => webp(data),
        Format::Gif => gif(data),
        Format::Mp4 => {
            let mut v = data.to_vec();
            mp4(&mut v).then_some(v)
        }
        Format::WebM => webm(data),
    };
    out.map(Cow::Owned).ok_or_else(|| {
        Error::usage(format!(
            "cannot take the metadata (the place a photo was taken among it) off {name}: it is not a {f:?} file mstdn can read"
        ))
        .with_hint("save it again with another program, or upload it from the web")
    })
}

// --- JPEG ---------------------------------------------------------------

/// Whether a JPEG's EXIF says it is to be turned or flipped to be seen.
fn jpeg_turned(data: &[u8]) -> bool {
    use image::ImageDecoder;
    let Ok(mut d) = image::codecs::jpeg::JpegDecoder::new(std::io::Cursor::new(data)) else {
        return false;
    };
    !matches!(
        d.orientation(),
        Ok(image::metadata::Orientation::NoTransforms) | Err(_)
    )
}

/// A JPEG without APP1 (EXIF, XMP), APP3 to APP13 (IPTC, Photoshop and the
/// like), APP15 and comments. APP0 (JFIF), APP2 (the ICC profile) and
/// APP14 (Adobe's colour transform) say how to show it, and stay. A
/// picture to be turned is turned and encoded again.
fn jpeg(data: &[u8]) -> Option<Vec<u8>> {
    if jpeg_turned(data) {
        return jpeg_upright(data);
    }
    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[..2]);
    let mut at = 2;
    loop {
        // Fill bytes before a marker.
        while data.get(at) == Some(&0xFF) && data.get(at + 1) == Some(&0xFF) {
            at += 1;
        }
        if data.get(at) != Some(&0xFF) {
            return None;
        }
        let marker = *data.get(at + 1)?;
        match marker {
            // Start of scan: the rest is the picture, as it is.
            0xDA => {
                out.extend_from_slice(&data[at..]);
                return Some(out);
            }
            0xD9 => {
                out.extend_from_slice(&data[at..at + 2]);
                return Some(out);
            }
            // Markers without a length.
            0x01 | 0xD0..=0xD7 => {
                out.extend_from_slice(&data[at..at + 2]);
                at += 2;
                continue;
            }
            _ => {}
        }
        let len = usize::from(u16::from_be_bytes([*data.get(at + 2)?, *data.get(at + 3)?]));
        if len < 2 || at + 2 + len > data.len() {
            return None;
        }
        let end = at + 2 + len;
        let private = matches!(marker, 0xE1 | 0xE3..=0xED | 0xEF | 0xFE);
        if !private {
            out.extend_from_slice(&data[at..end]);
        }
        at = end;
    }
}

/// A JPEG turned the way its EXIF says and encoded again, without it.
fn jpeg_upright(data: &[u8]) -> Option<Vec<u8>> {
    use image::ImageDecoder;
    let mut d = image::codecs::jpeg::JpegDecoder::new(std::io::Cursor::new(data)).ok()?;
    let o = d.orientation().ok()?;
    let mut img = image::DynamicImage::from_decoder(d).ok()?;
    img.apply_orientation(o);
    let mut out = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 92);
    img.to_rgb8().write_with_encoder(enc).ok()?;
    Some(out)
}

// --- PNG ----------------------------------------------------------------

/// A PNG without its text (`tEXt`, `zTXt`, `iTXt`, where XMP is kept),
/// its EXIF (`eXIf`) and its time (`tIME`).
fn png(data: &[u8]) -> Option<Vec<u8>> {
    let mut out = data[..8].to_vec();
    let mut at = 8;
    while at < data.len() {
        let len = u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?) as usize;
        let end = at.checked_add(12)?.checked_add(len)?;
        if end > data.len() {
            return None;
        }
        let kind = &data[at + 4..at + 8];
        if !matches!(kind, b"tEXt" | b"zTXt" | b"iTXt" | b"eXIf" | b"tIME") {
            out.extend_from_slice(&data[at..end]);
        }
        at = end;
        if kind == b"IEND" {
            return Some(out);
        }
    }
    None
}

// --- WebP ---------------------------------------------------------------

/// A WebP without its `EXIF` and `XMP ` chunks, the extended header's flags
/// and the RIFF size saying so.
fn webp(data: &[u8]) -> Option<Vec<u8>> {
    let mut out = b"RIFF\0\0\0\0WEBP".to_vec();
    let riff_end = 8usize.checked_add(u32::from_le_bytes(data[4..8].try_into().ok()?) as usize)?;
    let end_all = riff_end.min(data.len());
    let mut at = 12;
    let mut vp8x_at = None;
    while at + 8 <= end_all {
        let len = u32::from_le_bytes(data[at + 4..at + 8].try_into().ok()?) as usize;
        let padded = len + (len & 1);
        let end = at.checked_add(8)?.checked_add(padded)?;
        if end > end_all {
            // The last chunk may lack its padding byte.
            if at + 8 + len != end_all {
                return None;
            }
        }
        let end = end.min(end_all);
        let kind = &data[at..at + 4];
        match kind {
            b"EXIF" | b"XMP " => {}
            _ => {
                if kind == b"VP8X" {
                    vp8x_at = Some(out.len());
                }
                out.extend_from_slice(&data[at..end]);
            }
        }
        at = end;
    }
    if at != end_all {
        return None;
    }
    if let Some(v) = vp8x_at {
        // Flags: the EXIF (0x08) and XMP (0x04) bits off.
        let flags = out.get_mut(v + 8)?;
        *flags &= !0x0C;
    }
    let size = u32::try_from(out.len() - 8).ok()?;
    out[4..8].copy_from_slice(&size.to_le_bytes());
    Some(out)
}

// --- GIF ----------------------------------------------------------------

/// A GIF without its comments and application extensions (XMP among
/// them), but for the ones that make it loop (`NETSCAPE2.0`, `ANIMEXTS1.0`).
fn gif(data: &[u8]) -> Option<Vec<u8>> {
    fn sub_blocks(data: &[u8], mut at: usize) -> Option<usize> {
        loop {
            let n = usize::from(*data.get(at)?);
            at += 1 + n;
            if n == 0 {
                return Some(at);
            }
            if at > data.len() {
                return None;
            }
        }
    }
    if data.len() < 13 {
        return None;
    }
    let mut out = Vec::with_capacity(data.len());
    let flags = data[10];
    let mut at = 13;
    if flags & 0x80 != 0 {
        at += 3 * (1 << ((flags & 7) + 1));
    }
    if at > data.len() {
        return None;
    }
    out.extend_from_slice(&data[..at]);
    loop {
        match *data.get(at)? {
            0x3B => {
                out.push(0x3B);
                return Some(out);
            }
            0x21 => {
                let label = *data.get(at + 1)?;
                let end = sub_blocks(data, at + 2)?;
                let keep = match label {
                    0xFE => false,
                    0xFF => {
                        let id = data.get(at + 3..at + 14)?;
                        id == b"NETSCAPE2.0" || id == b"ANIMEXTS1.0"
                    }
                    _ => true,
                };
                if keep {
                    out.extend_from_slice(data.get(at..end)?);
                }
                at = end;
            }
            0x2C => {
                let desc = data.get(at..at + 10)?;
                let mut end = at + 10;
                if desc[9] & 0x80 != 0 {
                    end += 3 * (1 << ((desc[9] & 7) + 1));
                }
                // The LZW code size, then the picture's blocks.
                end = sub_blocks(data, end + 1)?;
                out.extend_from_slice(data.get(at..end)?);
                at = end;
            }
            _ => return None,
        }
    }
}

// --- MP4 ----------------------------------------------------------------

/// The boxes that say where, when, and with what a video was made: user
/// data (where a phone writes the place, `©xyz`), metadata (`meta`, with
/// Apple's location keys), `loci`, and `uuid` boxes (XMP), as bsky's.
const PRIVATE_BOXES: &[&[u8; 4]] = &[b"udta", b"meta", b"loci", b"uuid", b"\xA9xyz"];

/// The boxes that hold other boxes, where the private ones are looked for.
const CONTAINERS: &[&[u8; 4]] = &[b"moov", b"trak", b"mdia", b"minf", b"edts"];

/// Blank the private boxes of an MP4 or QuickTime file in place: each
/// becomes a `free` box of the same size, its content zeroed, so every
/// offset in the file (the media data's included) stays where it was. A
/// track's handler name is blanked too.
///
/// False when a box cannot be read (smaller than its header, or longer
/// than what is left of the file or of the box it is in): what comes after
/// it is not known, and a private box there would go up as it is.
pub(crate) fn mp4(file: &mut [u8]) -> bool {
    fn walk(buf: &mut [u8]) -> bool {
        let mut at = 0;
        while at + 8 <= buf.len() {
            let size =
                u32::from_be_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]]) as usize;
            let kind: [u8; 4] = [buf[at + 4], buf[at + 5], buf[at + 6], buf[at + 7]];
            let (size, header) = match size {
                0 => (buf.len() - at, 8),
                1 => {
                    let Some(large) = buf.get(at + 8..at + 16) else {
                        return false;
                    };
                    let large = u64::from_be_bytes(large.try_into().unwrap_or([0xFF; 8]));
                    (usize::try_from(large).unwrap_or(usize::MAX), 16)
                }
                n => (n, 8),
            };
            // Compared with what is left, not added to `at`: a 64-bit size
            // near the largest overflows the sum.
            if size < header || size > buf.len() - at {
                return false;
            }
            let body = at + header..at + size;
            if PRIVATE_BOXES.contains(&&kind) {
                buf[at + 4..at + 8].copy_from_slice(b"free");
                buf[body].fill(0);
            } else if kind == *b"hdlr" && body.len() > 24 {
                // The handler's name, which may name the device or the
                // program: blanked, its length kept.
                buf[body.start + 24..body.end].fill(0);
            } else if CONTAINERS.contains(&&kind) && !walk(&mut buf[body]) {
                return false;
            }
            at += size;
        }
        true
    }
    walk(file)
}

// --- WebM ---------------------------------------------------------------

/// An EBML variable-length integer at `at`: its value (the length marker
/// taken off, for a size) and how many bytes it takes.
fn vint(data: &[u8], at: usize, keep_marker: bool) -> Option<(u64, usize)> {
    let first = *data.get(at)?;
    let n = first.leading_zeros() as usize + 1;
    if n > 8 {
        return None;
    }
    let bytes = data.get(at..at + n)?;
    let mut v = if keep_marker {
        u64::from(first)
    } else {
        u64::from(first & 0xFFu8.checked_shr(n as u32).unwrap_or(0))
    };
    for b in &bytes[1..] {
        v = (v << 8) | u64::from(*b);
    }
    Some((v, n))
}

/// `Tags` (titles, dates, places, encoders) and the segment info's
/// `Title`, `MuxingApp`, `WritingApp` and `DateUTC`, as Void elements of
/// the same size, so the cues' offsets stay right.
fn webm(data: &[u8]) -> Option<Vec<u8>> {
    const SEGMENT: u64 = 0x1853_8067;
    const INFO: u64 = 0x1549_A966;
    const TAGS: u64 = 0x1254_C367;
    const PRIVATE_INFO: [u64; 4] = [0x7BA9, 0x4D80, 0x5741, 0x4461];
    /// Turn the element at `at`, `total` bytes long, into a Void one:
    /// ID 0xEC, a size of one byte or eight, and zeros.
    fn void(buf: &mut [u8], at: usize, total: usize) -> Option<()> {
        if total < 2 {
            return None;
        }
        buf[at] = 0xEC;
        if total - 2 <= 126 {
            buf[at + 1] = 0x80 | u8::try_from(total - 2).ok()?;
            buf[at + 2..at + total].fill(0);
        } else {
            let body = (total - 9) as u64;
            buf[at + 1] = 0x01;
            buf[at + 2..at + 9].copy_from_slice(&body.to_be_bytes()[1..]);
            buf[at + 9..at + total].fill(0);
        }
        Some(())
    }
    /// The elements of `buf[from..to]`: (id, start, header length, end).
    fn children(buf: &[u8], from: usize, to: usize) -> Option<Vec<(u64, usize, usize, usize)>> {
        let mut v = Vec::new();
        let mut at = from;
        while at < to {
            let (id, idl) = vint(buf, at, true)?;
            let (size, sl) = vint(buf, at + idl, false)?;
            let header = idl + sl;
            // All ones: unknown size, to the end of the parent.
            let unknown = size == (1u64 << (7 * sl)) - 1;
            let end = if unknown {
                to
            } else {
                at.checked_add(header)?
                    .checked_add(usize::try_from(size).ok()?)?
            };
            if end > to {
                return None;
            }
            v.push((id, at, header, end));
            at = end;
        }
        Some(v)
    }
    let mut out = data.to_vec();
    for (id, at, header, end) in children(data, 0, data.len())? {
        if id != SEGMENT {
            continue;
        }
        for (cid, cat, cheader, cend) in children(data, at + header, end)? {
            if cid == TAGS {
                void(&mut out, cat, cend - cat)?;
            } else if cid == INFO {
                for (iid, iat, _, iend) in children(data, cat + cheader, cend)? {
                    if PRIVATE_INFO.contains(&iid) {
                        void(&mut out, iat, iend - iat)?;
                    }
                }
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests;
