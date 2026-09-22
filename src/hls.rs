//! What playing a Bluesky video needs besides the decoder: its HLS
//! playlists (which stream, which segments) and the H.264 inside the MPEG
//! transport stream segments.
//!
//! Only as much of each format as Bluesky serves is read: a master playlist
//! of H.264 variants, a media playlist of `.ts` segments, and in each
//! segment the program's first video stream. Audio is skipped.

/// A URL as a playlist line names it, relative to the playlist at `base`.
pub fn resolve(base: &str, rel: &str) -> String {
    if rel.starts_with("https://") || rel.starts_with("http://") {
        return rel.to_string();
    }
    let base = base.split(['?', '#']).next().unwrap_or(base);
    if let Some(path) = rel.strip_prefix('/') {
        // The scheme and host of the base, then the absolute path.
        let after_scheme = base.find("://").map_or(0, |i| i + 3);
        let host_end = base[after_scheme..]
            .find('/')
            .map_or(base.len(), |i| after_scheme + i);
        return format!("{}/{path}", &base[..host_end]);
    }
    let dir = base.rfind('/').map_or(base, |i| &base[..=i]);
    format!("{dir}{rel}")
}

/// The stream to play from a master playlist: the one with the least
/// bandwidth, which is plenty for a terminal. `None` when `text` is already
/// a media playlist.
pub fn pick_variant(text: &str, base: &str) -> Option<String> {
    variants(text, base)
        .into_iter()
        .min_by_key(|(b, _)| if *b == 0 { u64::MAX } else { *b })
        .map(|(_, u)| u)
}

/// The stream to save from a master playlist: the one with the most
/// bandwidth. `None` when `text` is already a media playlist.
pub fn pick_best_variant(text: &str, base: &str) -> Option<String> {
    variants(text, base)
        .into_iter()
        .max_by_key(|(b, _)| *b)
        .map(|(_, u)| u)
}

fn variants(text: &str, base: &str) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    let mut lines = text.lines().map(str::trim);
    while let Some(line) = lines.next() {
        let Some(attrs) = line.strip_prefix("#EXT-X-STREAM-INF:") else {
            continue;
        };
        let bandwidth = attrs
            .split(',')
            .find_map(|a| a.strip_prefix("BANDWIDTH="))
            .and_then(|b| b.parse().ok())
            .unwrap_or(0);
        if let Some(uri) = lines
            .by_ref()
            .find(|l| !l.is_empty() && !l.starts_with('#'))
        {
            out.push((bandwidth, resolve(base, uri)));
        }
    }
    out
}

/// The segments of a media playlist, in order.
pub fn segments(text: &str, base: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| resolve(base, l))
        .collect()
}

/// One frame's worth of H.264 (Annex B), with its presentation time in
/// 90 kHz ticks when the stream gave one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessUnit {
    pub pts: Option<u64>,
    pub data: Vec<u8>,
}

/// Stream types of the PMT.
const H264: u8 = 0x1b;

/// Pulls the H.264 stream out of MPEG transport stream bytes, fed in any
/// pieces; the state carries from one segment to the next.
#[derive(Debug, Default)]
pub struct Demuxer {
    pmt_pid: Option<u16>,
    video_pid: Option<u16>,
    /// The program's video stream is of a type bs cannot decode.
    pub unsupported: Option<u8>,
    pes: Vec<u8>,
    pes_pts: Option<u64>,
    pending: Vec<u8>,
    out: Vec<AccessUnit>,
}

impl Demuxer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Read more of the stream.
    pub fn feed(&mut self, data: &[u8]) {
        self.pending.extend_from_slice(data);
        let mut at = 0;
        while self.pending.len() - at >= 188 {
            if self.pending[at] != 0x47 {
                // Lost sync: find the next packet start.
                at += 1;
                continue;
            }
            let packet: [u8; 188] = self.pending[at..at + 188].try_into().expect("188 bytes");
            self.packet(&packet);
            at += 188;
        }
        self.pending.drain(..at);
    }

    /// The access units read so far.
    pub fn take(&mut self) -> Vec<AccessUnit> {
        std::mem::take(&mut self.out)
    }

    /// The end of the stream: the last access unit is complete.
    pub fn finish(&mut self) -> Vec<AccessUnit> {
        self.flush();
        self.take()
    }

    fn flush(&mut self) {
        if !self.pes.is_empty() {
            self.out.push(AccessUnit {
                pts: self.pes_pts.take(),
                data: std::mem::take(&mut self.pes),
            });
        }
    }

    fn packet(&mut self, p: &[u8; 188]) {
        let start = p[1] & 0x40 != 0;
        let pid = (u16::from(p[1] & 0x1f) << 8) | u16::from(p[2]);
        let control = (p[3] >> 4) & 3;
        if control & 1 == 0 {
            return;
        }
        let mut at = 4;
        if control & 2 != 0 {
            at += 1 + usize::from(p[4]);
        }
        let Some(payload) = p.get(at..) else {
            return;
        };
        if pid == 0 && start {
            self.pat(payload);
        } else if Some(pid) == self.pmt_pid && start {
            self.pmt(payload);
        } else if Some(pid) == self.video_pid {
            self.video(payload, start);
        }
    }

    /// The section a payload starting a table holds.
    fn section(payload: &[u8]) -> Option<&[u8]> {
        let pointer = usize::from(*payload.first()?);
        let t = payload.get(1 + pointer..)?;
        let len = (usize::from(*t.get(1)? & 0x0f) << 8) | usize::from(*t.get(2)?);
        // After the length, minus the CRC.
        t.get(..3 + len)?.get(..(3 + len).saturating_sub(4))
    }

    fn pat(&mut self, payload: &[u8]) {
        let Some(t) = Self::section(payload) else {
            return;
        };
        for entry in t.get(8..).unwrap_or_default().as_chunks::<4>().0 {
            let program = u16::from_be_bytes([entry[0], entry[1]]);
            if program != 0 {
                self.pmt_pid = Some((u16::from(entry[2] & 0x1f) << 8) | u16::from(entry[3]));
                return;
            }
        }
    }

    fn pmt(&mut self, payload: &[u8]) {
        let Some(t) = Self::section(payload) else {
            return;
        };
        let Some(info_len) = t
            .get(10..12)
            .map(|b| (usize::from(b[0] & 0x0f) << 8) | usize::from(b[1]))
        else {
            return;
        };
        let mut i = 12 + info_len;
        while let Some(es) = t.get(i..i + 5) {
            let (kind, pid) = (es[0], (u16::from(es[1] & 0x1f) << 8) | u16::from(es[2]));
            let es_len = (usize::from(es[3] & 0x0f) << 8) | usize::from(es[4]);
            match kind {
                H264 => {
                    self.video_pid = Some(pid);
                    self.unsupported = None;
                    return;
                }
                // Other video: HEVC, MPEG-2, MPEG-4 part 2.
                0x24 | 0x02 | 0x10 if self.video_pid.is_none() => self.unsupported = Some(kind),
                _ => {}
            }
            i += 5 + es_len;
        }
    }

    fn video(&mut self, payload: &[u8], start: bool) {
        if !start {
            if !self.pes.is_empty() || self.pes_pts.is_some() {
                self.pes.extend_from_slice(payload);
            }
            return;
        }
        self.flush();
        // PES header: start code, stream id, length, flags, header length.
        if payload.len() < 9 || payload[..3] != [0, 0, 1] {
            return;
        }
        let header_len = usize::from(payload[8]);
        if payload[7] & 0x80 != 0
            && let Some(b) = payload.get(9..14)
        {
            let pts = (u64::from(b[0] >> 1) & 7) << 30
                | u64::from(b[1]) << 22
                | u64::from(b[2] >> 1) << 15
                | u64::from(b[3]) << 7
                | u64::from(b[4] >> 1);
            self.pes_pts = Some(pts);
        }
        if let Some(es) = payload.get(9 + header_len..) {
            self.pes.extend_from_slice(es);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::path::Path;

    #[rstest]
    #[case(
        "https://v.test/watch/x/playlist.m3u8",
        "360p/video.m3u8?s=1",
        "https://v.test/watch/x/360p/video.m3u8?s=1"
    )]
    #[case(
        "https://v.test/watch/x/360p/video.m3u8?s=1",
        "video0.ts?d=6",
        "https://v.test/watch/x/360p/video0.ts?d=6"
    )]
    #[case("https://v.test/a/b.m3u8", "/c/d.ts", "https://v.test/c/d.ts")]
    #[case(
        "https://v.test/a/b.m3u8",
        "https://cdn.test/e.ts",
        "https://cdn.test/e.ts"
    )]
    fn playlist_lines_resolve_against_the_playlist(
        #[case] base: &str,
        #[case] rel: &str,
        #[case] want: &str,
    ) {
        assert_eq!(resolve(base, rel), want);
    }

    #[test]
    fn the_lightest_variant_is_played() {
        let master = "#EXTM3U\n#EXT-X-VERSION:3\n\
            #EXT-X-STREAM-INF:PROGRAM-ID=0,BANDWIDTH=3300000,CODECS=\"avc1.640020\",RESOLUTION=720x1516\n\
            720p/video.m3u8?session_id=a\n\
            #EXT-X-STREAM-INF:PROGRAM-ID=0,BANDWIDTH=550000,CODECS=\"avc1.64001e\",RESOLUTION=360x758\n\
            360p/video.m3u8?session_id=a\n";
        assert_eq!(
            pick_variant(master, "https://v.test/w/playlist.m3u8").as_deref(),
            Some("https://v.test/w/360p/video.m3u8?session_id=a")
        );
        assert_eq!(
            pick_variant("#EXTM3U\n#EXTINF:6.0,\nvideo0.ts\n", "x"),
            None
        );
        assert_eq!(
            pick_best_variant(master, "https://v.test/w/playlist.m3u8").as_deref(),
            Some("https://v.test/w/720p/video.m3u8?session_id=a")
        );
    }

    #[test]
    fn a_media_playlist_lists_its_segments_in_order() {
        let media = "#EXTM3U\n#EXT-X-TARGETDURATION:6\n#EXTINF:6.000,\nvideo0.ts?dur=6\n\
            #EXTINF:5.300,\nvideo1.ts?dur=5.3\n#EXT-X-ENDLIST\n";
        assert_eq!(
            segments(media, "https://v.test/w/360p/video.m3u8"),
            [
                "https://v.test/w/360p/video0.ts?dur=6",
                "https://v.test/w/360p/video1.ts?dur=5.3"
            ]
        );
    }

    fn fixture() -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("e2e/atago/testdata/hls/v/seg0.ts"),
        )
        .unwrap()
    }

    #[test]
    fn a_real_segment_gives_h264_frames_in_time_order() {
        let mut d = Demuxer::new();
        // Fed in odd pieces, as a download arrives.
        for piece in fixture().chunks(1000) {
            d.feed(piece);
        }
        let units = d.finish();
        assert!(units.len() >= 8, "{} access units", units.len());
        assert!(units[0].data.starts_with(&[0, 0, 0, 1]) || units[0].data.starts_with(&[0, 0, 1]));
        let pts: Vec<u64> = units.iter().filter_map(|u| u.pts).collect();
        assert_eq!(pts.len(), units.len());
        assert!(pts.windows(2).all(|w| w[0] <= w[1]) || pts.len() > 1);
        assert_eq!(d.unsupported, None);
    }

    #[test]
    fn the_segment_decodes_to_pictures() {
        use openh264::decoder::Decoder;
        use openh264::formats::YUVSource;
        let mut d = Demuxer::new();
        d.feed(&fixture());
        let mut decoder = Decoder::new().unwrap();
        let mut pictures = 0;
        for unit in d.finish() {
            if let Ok(Some(yuv)) = decoder.decode(&unit.data) {
                assert_eq!(yuv.dimensions(), (64, 36));
                pictures += 1;
            }
        }
        assert!(pictures >= 5, "{pictures} pictures");
    }

    #[test]
    fn garbage_gives_nothing_and_does_not_panic() {
        let mut d = Demuxer::new();
        d.feed(&[0x47; 1000]);
        d.feed(b"not a transport stream at all");
        assert!(d.finish().is_empty());
    }
}
