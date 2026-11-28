//! The H.264 pictures of an MP4 file, in decode order, for the player.
//!
//! bsky plays Bluesky's HLS, whose segments are MPEG-TS; a file on Misskey
//! is the uploaded file itself, MP4 or WebM. This reads an MP4's `moov`
//! box: the video track, its `avcC` (the SPS and PPS, and how long the
//! length before each NAL unit is), and its sample table (sizes, chunk
//! offsets, chunks, durations, composition offsets), and hands each sample
//! to the decoder as Annex B (start codes before each NAL unit), the form
//! OpenH264 takes, with the time it is shown at. A fragmented MP4 (`moof`),
//! and a video that is not H.264 (HEVC, AV1, VP9), are refused with what
//! they are: OpenH264 decodes H.264 alone.

/// A sample of the video track: its NAL units as Annex B, and the time it
/// is shown at, in 90 kHz ticks as the player's clock counts them.
pub(crate) struct Sample {
    pub data: Vec<u8>,
    pub pts: u64,
}

/// The video track of an MP4.
pub(crate) struct Track {
    /// The SPS and PPS as Annex B, to go before the first sample.
    pub config: Vec<u8>,
    pub samples: Vec<Sample>,
}

const START: [u8; 4] = [0, 0, 0, 1];

fn be16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn be64(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

/// The boxes in `buf`, each its type and its body. A box whose size runs
/// past `buf` ends the list.
fn boxes(buf: &[u8]) -> Boxes<'_> {
    boxes_checked(buf).0
}

/// Each box's type and body.
type Boxes<'a> = Vec<([u8; 4], &'a [u8])>;

/// [`boxes`], and whether a box's size ran past `buf` (or below its own
/// header) and ended the list early. A 64-bit size can be anything, so the
/// end of a box is added with a check: wrapped around, it would pass the
/// bound and slice backwards.
fn boxes_checked(buf: &[u8]) -> (Boxes<'_>, bool) {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at + 8 <= buf.len() {
        let Some(size32) = be32(buf, at) else {
            return (out, true);
        };
        let Ok(kind) = <[u8; 4]>::try_from(&buf[at + 4..at + 8]) else {
            return (out, true);
        };
        let (header, size) = match size32 {
            0 => (8, buf.len() - at),
            1 => match be64(buf, at + 8).and_then(|s| usize::try_from(s).ok()) {
                Some(s) => (16, s),
                None => return (out, true),
            },
            s => (8, s as usize),
        };
        match at.checked_add(size) {
            Some(end) if size >= header && end <= buf.len() => {
                out.push((kind, &buf[at + header..end]));
                at = end;
            }
            _ => return (out, true),
        }
    }
    (out, false)
}

fn child<'a>(buf: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(buf)
        .into_iter()
        .find(|(k, _)| k == kind)
        .map(|(_, b)| b)
}

/// The video track of the MP4 file `file`, or why it cannot be played.
pub(crate) fn video_track(file: &[u8]) -> Result<Track, String> {
    let (top, broken) = boxes_checked(file);
    if top.iter().any(|(k, _)| k == b"moof") {
        return Err("the video is a fragmented MP4, which msky cannot play yet".into());
    }
    let moov = top
        .iter()
        .find(|(k, _)| k == b"moov")
        .map(|(_, b)| *b)
        .ok_or(if broken && !top.is_empty() {
            "the MP4 file is broken: a box runs past the end of the file"
        } else {
            "the file has no MP4 header (moov)"
        })?;
    let trak = boxes(moov)
        .into_iter()
        .filter(|(k, _)| k == b"trak")
        .map(|(_, b)| b)
        .find(|t| {
            child(t, b"mdia")
                .and_then(|m| child(m, b"hdlr"))
                .is_some_and(|h| h.get(8..12) == Some(b"vide"))
        })
        .ok_or("the file has no video track")?;
    let mdia = child(trak, b"mdia").ok_or("the video track has no mdia")?;
    let mdhd = child(mdia, b"mdhd").ok_or("the video track has no mdhd")?;
    let timescale = match mdhd.first() {
        Some(1) => be32(mdhd, 20),
        _ => be32(mdhd, 12),
    }
    .filter(|t| *t > 0)
    .ok_or("the video track has no time scale")?;
    let stbl = child(mdia, b"minf")
        .and_then(|m| child(m, b"stbl"))
        .ok_or("the video track has no sample table")?;
    let stsd = child(stbl, b"stsd").ok_or("the video track has no stsd")?;
    let entries = boxes(stsd.get(8..).unwrap_or_default());
    let (codec, entry) = entries
        .first()
        .ok_or("the video track describes no format")?;
    if codec != b"avc1" && codec != b"avc3" {
        let name = String::from_utf8_lossy(codec).into_owned();
        return Err(format!(
            "the video is {name}, not H.264; msky decodes H.264 only (with OpenH264)"
        ));
    }
    // A visual sample entry's fields take 78 bytes before its boxes.
    let avcc =
        child(entry.get(78..).unwrap_or_default(), b"avcC").ok_or("the H.264 track has no avcC")?;
    let (length_size, config) = parse_avcc(avcc).ok_or("the H.264 track's avcC is broken")?;
    let samples = sample_table(stbl, timescale, file.len())
        .ok_or("the video track's sample table is broken")?;
    let data = samples
        .into_iter()
        .filter_map(|(offset, size, pts)| {
            let raw = file.get(offset..offset.checked_add(size)?)?;
            Some(Sample {
                data: annex_b(raw, length_size)?,
                pts,
            })
        })
        .collect::<Vec<_>>();
    if data.is_empty() {
        return Err("the video track has no picture".into());
    }
    Ok(Track {
        config,
        samples: data,
    })
}

/// The length of the size before each NAL unit, and the SPS and PPS as
/// Annex B.
fn parse_avcc(b: &[u8]) -> Option<(usize, Vec<u8>)> {
    let length_size = usize::from(b.get(4)? & 3) + 1;
    let mut at = 5;
    let mut config = Vec::new();
    let sps = usize::from(b.get(at)? & 0x1f);
    at += 1;
    for _ in 0..sps {
        let n = usize::from(be16(b, at)?);
        config.extend_from_slice(&START);
        config.extend_from_slice(b.get(at + 2..at + 2 + n)?);
        at += 2 + n;
    }
    let pps = usize::from(*b.get(at)?);
    at += 1;
    for _ in 0..pps {
        let n = usize::from(be16(b, at)?);
        config.extend_from_slice(&START);
        config.extend_from_slice(b.get(at + 2..at + 2 + n)?);
        at += 2 + n;
    }
    Some((length_size, config))
}

/// Length-prefixed NAL units as Annex B.
fn annex_b(raw: &[u8], length_size: usize) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(raw.len() + 16);
    let mut at = 0;
    while at < raw.len() {
        let mut n = 0usize;
        for i in 0..length_size {
            n = (n << 8) | usize::from(*raw.get(at + i)?);
        }
        at += length_size;
        out.extend_from_slice(&START);
        out.extend_from_slice(raw.get(at..at.checked_add(n)?)?);
        at += n;
    }
    Some(out)
}

/// A full box's entries after its version, flags and count.
fn entries(b: &[u8]) -> Option<(u8, usize, &[u8])> {
    let version = *b.first()?;
    let count = be32(b, 4)? as usize;
    let rest = b.get(8..)?;
    // Each entry takes at least four bytes of the box.
    (count <= rest.len() / 4).then_some((version, count, rest))
}

/// Each sample's offset in the file, its size, and the time it is shown,
/// in 90 kHz ticks, in decode order.
///
/// Every count is bounded by the file: a sample takes at least a byte of
/// it, so a broken or hostile count is refused rather than looped over or
/// allocated for.
fn sample_table(stbl: &[u8], timescale: u32, file_len: usize) -> Option<Vec<(usize, usize, u64)>> {
    // Sizes.
    let stsz = child(stbl, b"stsz")?;
    let fixed = be32(stsz, 4)? as usize;
    let count = be32(stsz, 8)? as usize;
    if count > file_len {
        return None;
    }
    let sizes: Vec<usize> = if fixed != 0 {
        vec![fixed; count]
    } else {
        (0..count)
            .map(|i| be32(stsz, 12 + i * 4).map(|s| s as usize))
            .collect::<Option<_>>()?
    };
    // Chunk offsets.
    let chunks: Vec<usize> = if let Some(stco) = child(stbl, b"stco") {
        let (_, n, e) = entries(stco)?;
        (0..n)
            .map(|i| be32(e, i * 4).map(|o| o as usize))
            .collect::<Option<_>>()?
    } else {
        let (_, n, e) = entries(child(stbl, b"co64")?)?;
        (0..n)
            .map(|i| be64(e, i * 8).and_then(|o| usize::try_from(o).ok()))
            .collect::<Option<_>>()?
    };
    // Samples per chunk, as runs starting at a chunk (from 1).
    let (_, n, e) = entries(child(stbl, b"stsc")?)?;
    let runs: Vec<(usize, usize)> = (0..n)
        .map(|i| Some((be32(e, i * 12)? as usize, be32(e, i * 12 + 4)? as usize)))
        .collect::<Option<_>>()?;
    let mut offsets = Vec::with_capacity(count);
    let mut sample = 0usize;
    for (c, &chunk_offset) in chunks.iter().enumerate() {
        let number = c + 1;
        let per = runs
            .iter()
            .rev()
            .find(|(first, _)| *first <= number)
            .map_or(0, |r| r.1);
        let mut at = chunk_offset;
        for _ in 0..per.min(count) {
            let Some(&size) = sizes.get(sample) else {
                break;
            };
            offsets.push((at, size));
            at = at.checked_add(size)?;
            sample += 1;
        }
    }
    // Decode times, and the offsets to the times shown.
    let (_, n, e) = entries(child(stbl, b"stts")?)?;
    let mut dts = Vec::with_capacity(count);
    let mut t: u64 = 0;
    'runs: for i in 0..n {
        let (k, delta) = (be32(e, i * 8)?, be32(e, i * 8 + 4)?);
        for _ in 0..k {
            if dts.len() >= count {
                break 'runs;
            }
            dts.push(t);
            t = t.saturating_add(u64::from(delta));
        }
    }
    let mut cts: Vec<i64> = vec![0; dts.len()];
    if let Some(ctts) = child(stbl, b"ctts") {
        let (version, n, e) = entries(ctts)?;
        let mut at = 0;
        for i in 0..n {
            let k = be32(e, i * 8)?;
            let raw = be32(e, i * 8 + 4)?;
            let off = if version == 1 {
                i64::from(raw as i32)
            } else {
                i64::from(raw)
            };
            for _ in 0..k {
                let Some(c) = cts.get_mut(at) else { break };
                *c = off;
                at += 1;
            }
        }
    }
    let to_ticks = |v: i64| -> u64 {
        (v.max(0) as u128 * 90_000 / u128::from(timescale)).min(u128::from(u64::MAX)) as u64
    };
    Some(
        offsets
            .into_iter()
            .enumerate()
            .map(|(i, (o, s))| {
                let d = dts.get(i).copied().unwrap_or(0).min(i64::MAX as u64 / 2) as i64;
                (
                    o,
                    s,
                    to_ticks(d.saturating_add(cts.get(i).copied().unwrap_or(0))),
                )
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip() -> Vec<u8> {
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/e2e/atago/testdata/clip.mp4"
        ))
        .unwrap()
    }

    // The fixture's ten pictures come out in decode order with their SPS
    // and PPS, at 10 per second, and OpenH264 decodes them at 64 x 36.
    #[test]
    fn an_h264_mp4_gives_its_pictures_and_they_decode() {
        let t = video_track(&clip()).unwrap();
        assert_eq!(t.samples.len(), 10);
        assert!(t.config.starts_with(&START));
        let mut pts: Vec<u64> = t.samples.iter().map(|s| s.pts).collect();
        pts.sort_unstable();
        assert_eq!(pts[1] - pts[0], 9_000, "a tenth of a second");
        let mut d = openh264::decoder::Decoder::new().unwrap();
        let mut decoded = 0;
        let mut first = t.config.clone();
        first.extend_from_slice(&t.samples[0].data);
        let mut feed: Vec<Vec<u8>> = vec![first];
        feed.extend(t.samples[1..].iter().map(|s| s.data.clone()));
        for unit in feed {
            if let Ok(Some(yuv)) = d.decode(&unit) {
                use openh264::formats::YUVSource;
                assert_eq!(yuv.dimensions(), (64, 36));
                decoded += 1;
            }
        }
        assert!(decoded > 0);
    }

    // What is not an MP4, a fragmented one, or a video that is not H.264
    // says what it is, and nothing panics on a file cut anywhere.
    #[test]
    fn what_cannot_be_played_says_why_and_a_cut_file_never_panics() {
        assert_eq!(
            video_track(b"\x1aE\xdf\xa3webm").err().unwrap(),
            "the file has no MP4 header (moov)"
        );
        let mut frag = vec![0, 0, 0, 8, b'm', b'o', b'o', b'f'];
        frag.extend_from_slice(&clip());
        assert!(video_track(&frag).err().unwrap().contains("fragmented"));
        let mut hevc = clip();
        // The sample entry, not the avc1 brand of ftyp.
        if let Some(i) = hevc.windows(4).rposition(|w| w == b"avc1") {
            hevc[i..i + 4].copy_from_slice(b"hvc1");
        }
        assert_eq!(
            video_track(&hevc).err().unwrap(),
            "the video is hvc1, not H.264; msky decodes H.264 only (with OpenH264)"
        );
        assert_eq!(
            video_track(&clip()[..100]).err().unwrap(),
            "the MP4 file is broken: a box runs past the end of the file"
        );
        let file = clip();
        for cut in 0..file.len() {
            let _ = video_track(&file[..cut]);
        }
        let mut x: u32 = 1;
        for _ in 0..2000 {
            let mut f = file.clone();
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let i = x as usize % f.len();
            f[i] ^= (x >> 8) as u8;
            let _ = video_track(&f);
        }
    }

    // A 64-bit box size near the top of the range does not wrap the end of
    // the box around to before its start: the file is refused as broken,
    // where it panicked on a backward slice and the video stayed loading.
    #[test]
    fn a_box_size_that_overflows_is_refused_without_panicking() {
        let file: &[u8] =
            b"\0\0\0\x20f\x01\0\0\0\0\0\0\0\0\x02\0is\0\x01\0\0\x08\0\0\0\0\x01\0\0\x14\0\
                            \0\0\0\x01\0\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xee";
        assert_eq!(file.len(), 48);
        assert_eq!(
            video_track(file).err().unwrap(),
            "the MP4 file is broken: a box runs past the end of the file"
        );
        // Inside `moov` too, where a box's end is counted from its parent.
        let mut nested = b"\0\0\0\x20moov".to_vec();
        nested.extend_from_slice(b"\0\0\0\x01trak\xff\xff\xff\xff\xff\xff\xff\xf8");
        nested.extend_from_slice(&[0; 8]);
        assert_eq!(
            video_track(&nested).err().unwrap(),
            "the file has no video track"
        );
    }

    fn mp4_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = u32::try_from(body.len() + 8)
            .unwrap()
            .to_be_bytes()
            .to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(body);
        v
    }

    /// A full box: its version, zero flags, and then `fields` as 32-bit
    /// big-endian numbers.
    fn full_box(kind: &[u8; 4], version: u8, fields: &[u32]) -> Vec<u8> {
        let mut body = vec![version, 0, 0, 0];
        for f in fields {
            body.extend_from_slice(&f.to_be_bytes());
        }
        mp4_box(kind, &body)
    }

    // A box after the first one whose size is 0 runs to the end of the
    // buffer, and one whose size is 1 takes its size from the 64 bits after
    // its type.
    #[test]
    fn a_box_sized_to_the_end_or_by_64_bits_after_another_is_read() {
        let mut buf = mp4_box(b"free", &[]);
        buf.extend_from_slice(&[0, 0, 0, 0]);
        buf.extend_from_slice(b"mdat");
        buf.extend_from_slice(&[1, 2, 3, 4]);
        let (b, broken) = boxes_checked(&buf);
        assert!(!broken);
        assert_eq!(b.len(), 2);
        assert_eq!(b[1], (*b"mdat", &[1u8, 2, 3, 4][..]));

        let mut buf = mp4_box(b"free", &[]);
        buf.extend_from_slice(&[0, 0, 0, 1]);
        buf.extend_from_slice(b"wide");
        buf.extend_from_slice(&20u64.to_be_bytes());
        buf.extend_from_slice(&[5, 6, 7, 8]);
        buf.extend_from_slice(&mp4_box(b"skip", &[9]));
        let (b, broken) = boxes_checked(&buf);
        assert!(!broken);
        assert_eq!(b.len(), 3);
        assert_eq!(b[1], (*b"wide", &[5u8, 6, 7, 8][..]));
        assert_eq!(b[2], (*b"skip", &[9u8][..]));
    }

    // Two SPS and two PPS of different lengths each come out after a
    // start code, in order, with the NAL length size from the low bits.
    #[test]
    fn an_avcc_with_two_sps_and_two_pps_gives_each_in_order() {
        let avcc = [
            1, 0x42, 0, 0x1e, 0xfd, 0xe2, // two-byte lengths, two SPS
            0, 3, 0x67, 0xa1, 0xa2, // SPS 1
            0, 1, 0x67, // SPS 2
            2,    // two PPS
            0, 3, 0x68, 0xb1, 0xb2, // PPS 1
            0, 2, 0x68, 0xc1, // PPS 2
        ];
        let (length_size, config) = parse_avcc(&avcc).unwrap();
        assert_eq!(length_size, 2);
        assert_eq!(
            config,
            [
                0, 0, 0, 1, 0x67, 0xa1, 0xa2, 0, 0, 0, 1, 0x67, 0, 0, 0, 1, 0x68, 0xb1, 0xb2, 0, 0,
                0, 1, 0x68, 0xc1,
            ]
        );
    }

    // A full box whose count claims more entries than its bytes can hold is
    // refused; one whose count fits is read.
    #[test]
    fn a_count_past_the_entries_is_refused() {
        let fits = [0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 2];
        assert_eq!(entries(&fits), Some((0, 2, &fits[8..])));
        let over = [0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 1, 0, 0, 0, 2];
        assert_eq!(entries(&over), None);
    }

    /// The boxes of a sample table, in order.
    fn stbl(parts: &[Vec<u8>]) -> Vec<u8> {
        parts.concat()
    }

    // Each chunk takes the samples of the run it falls in, at offsets that
    // follow on from the chunk's, and each sample is shown at its decode
    // time plus its composition offset, in 90 kHz ticks.
    #[test]
    fn the_sample_table_gives_each_sample_its_offset_size_and_time() {
        let t = stbl(&[
            full_box(b"stsz", 0, &[0, 5, 10, 20, 30, 40, 50]),
            full_box(b"stco", 0, &[3, 100, 1000, 2000]),
            full_box(b"stsc", 0, &[2, 1, 1, 1, 2, 2, 1]),
            full_box(b"stts", 0, &[2, 2, 3000, 3, 1500]),
            full_box(b"ctts", 0, &[2, 1, 6000, 4, 3000]),
        ]);
        assert_eq!(
            sample_table(&t, 3000, 10_000),
            Some(vec![
                (100, 10, 180_000),
                (1000, 20, 180_000),
                (1020, 30, 270_000),
                (2000, 40, 315_000),
                (2040, 50, 360_000),
            ])
        );
    }

    // 64-bit chunk offsets are read like 32-bit ones.
    #[test]
    fn co64_chunk_offsets_are_read() {
        let mut co64 = vec![0, 0, 0, 0, 0, 0, 0, 2];
        co64.extend_from_slice(&100u64.to_be_bytes());
        co64.extend_from_slice(&5000u64.to_be_bytes());
        let t = stbl(&[
            full_box(b"stsz", 0, &[7, 2]),
            mp4_box(b"co64", &co64),
            full_box(b"stsc", 0, &[1, 1, 1, 1]),
            full_box(b"stts", 0, &[1, 2, 1000]),
        ]);
        assert_eq!(
            sample_table(&t, 1000, 10_000),
            Some(vec![(100, 7, 0), (5000, 7, 90_000)])
        );
    }

    // A version 1 ctts holds signed offsets, so a picture can be shown
    // before its decode time (clamped at zero).
    #[test]
    fn a_version_1_ctts_offset_is_signed() {
        let t = stbl(&[
            full_box(b"stsz", 0, &[1, 2]),
            full_box(b"stco", 0, &[1, 0]),
            full_box(b"stsc", 0, &[1, 1, 2, 1]),
            full_box(b"stts", 0, &[1, 2, 3000]),
            full_box(b"ctts", 1, &[1, 2, (-1500i32) as u32]),
        ]);
        assert_eq!(
            sample_table(&t, 3000, 10_000),
            Some(vec![(0, 1, 0), (1, 1, 45_000)])
        );
    }

    // As many samples as the file has bytes is allowed; one more is not.
    #[test]
    fn a_sample_count_past_the_file_length_is_refused() {
        let table = |count| {
            stbl(&[
                full_box(b"stsz", 0, &[1, count]),
                full_box(b"stco", 0, &[1, 0]),
                full_box(b"stsc", 0, &[1, 1, count, 1]),
                full_box(b"stts", 0, &[1, count, 1]),
            ])
        };
        assert_eq!(
            sample_table(&table(2), 1, 2),
            Some(vec![(0, 1, 0), (1, 1, 90_000)])
        );
        assert_eq!(sample_table(&table(3), 1, 2), None);
    }

    /// A file of two pictures, each one NAL unit with a 4-byte length, its
    /// track timed by `mdhd`, a second apart at a time scale of 1000.
    fn two_picture_mp4(mdhd: Vec<u8>) -> Vec<u8> {
        let nal = [0, 0, 0, 2, 0x65, 0x88];
        let mut file = mp4_box(b"mdat", &[nal, nal].concat());
        let avcc = [
            1, 0x42, 0, 0x1e, 0xff, 0xe1, 0, 2, 0x67, 0x42, 1, 0, 2, 0x68, 0xce,
        ];
        let mut entry = vec![0u8; 78];
        entry.extend_from_slice(&mp4_box(b"avcC", &avcc));
        let mut stsd = vec![0, 0, 0, 0, 0, 0, 0, 1];
        stsd.extend_from_slice(&mp4_box(b"avc1", &entry));
        let stbl = mp4_box(
            b"stbl",
            &[
                mp4_box(b"stsd", &stsd),
                full_box(b"stsz", 0, &[6, 2]),
                full_box(b"stco", 0, &[1, 8]),
                full_box(b"stsc", 0, &[1, 1, 2, 1]),
                full_box(b"stts", 0, &[1, 2, 1000]),
            ]
            .concat(),
        );
        let mut hdlr = vec![0u8; 8];
        hdlr.extend_from_slice(b"vide");
        hdlr.extend_from_slice(&[0; 12]);
        let mdia = mp4_box(
            b"mdia",
            &[mp4_box(b"hdlr", &hdlr), mdhd, mp4_box(b"minf", &stbl)].concat(),
        );
        file.extend_from_slice(&mp4_box(b"moov", &mp4_box(b"trak", &mdia)));
        file
    }

    // A version 1 mdhd keeps its time scale after 64-bit times, and a time
    // scale of 0 is refused.
    #[test]
    fn the_time_scale_is_read_from_either_mdhd_version() {
        let pts = |file: &[u8]| -> Vec<u64> {
            video_track(file)
                .unwrap()
                .samples
                .iter()
                .map(|s| s.pts)
                .collect()
        };
        let v0 = two_picture_mp4(full_box(b"mdhd", 0, &[0, 0, 1000, 0]));
        assert_eq!(pts(&v0), [0, 90_000]);
        let v1 = two_picture_mp4(full_box(b"mdhd", 1, &[0, 0, 0, 0, 1000, 0, 0]));
        assert_eq!(pts(&v1), [0, 90_000]);
        assert_eq!(
            video_track(&v1).unwrap().samples[0].data,
            [0, 0, 0, 1, 0x65, 0x88]
        );
        let zero = two_picture_mp4(full_box(b"mdhd", 0, &[0, 0, 0, 0]));
        assert_eq!(
            video_track(&zero).err().unwrap(),
            "the video track has no time scale"
        );
    }
}
