//! A link card for a post: the page of its first link read for an Open
//! Graph title, description and picture, as Bluesky's app makes one.
//!
//! The card is an extra. A page that cannot be read, is not HTML in UTF-8,
//! or gives nothing to show leaves the post without a card rather than
//! stopping it, and a picture that cannot be fetched or uploaded leaves the
//! card without one. Posts are sent on the thread every write shares, so
//! each request has a short time limit.

use std::borrow::Cow;
use std::io::Read;
use std::net::{IpAddr, ToSocketAddrs};
use std::time::Duration;

use unicode_segmentation::UnicodeSegmentation;
use ureq::ResponseExt;

use crate::api::facets::{self, Target};
use crate::api::{Client, PostExternal};
use crate::media::Prepared;

/// How long the page, and then its picture, may take.
const TIMEOUT: Duration = Duration::from_secs(5);
/// How much of a page is read; the head, which holds the card, comes first.
const MAX_PAGE_BYTES: u64 = 1024 * 1024;
/// Largest picture fetched for a card.
const MAX_PICTURE_BYTES: u64 = 10 * 1024 * 1024;
/// Longest side of a card's picture once shrunk.
const THUMB_SIDE: u32 = 1000;
/// Longest title and description kept; the rest is cut and marked with an
/// ellipsis, so a page cannot make the post too large to send.
const MAX_TITLE_GRAPHEMES: usize = 300;
const MAX_DESCRIPTION_GRAPHEMES: usize = 1000;

/// The first link in `text`, as its facet links to it: a bare domain such
/// as `example.com` counts, with `https://` put in front.
fn first_link(text: &str) -> Option<String> {
    facets::detect(text)
        .into_iter()
        .filter_map(|span| match span.target {
            Target::Link(uri) => Some((span.start, uri)),
            Target::Mention(_) | Target::Tag(_) => None,
        })
        .min_by_key(|(start, _)| *start)
        .map(|(_, uri)| uri)
}

/// What the page of the first link in `text` gives a card: its address,
/// title and description, and the address of its picture when one may be
/// fetched. Nothing is uploaded; `post --dry-run` shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Page {
    pub uri: String,
    pub title: String,
    pub description: String,
    pub image: Option<String>,
    /// Whether the page is on the user's own network, so that its picture
    /// may be too.
    local: bool,
}

/// The card the first link in `text` would get, read from its page; `None`
/// when there is no link or its page gives no card.
pub(crate) fn read(text: &str) -> Option<Page> {
    let uri = first_link(text)?;
    let agent = crate::api::agent();
    let (html, base) = page(&agent, &uri)?;
    let found = crate::og::parse(&html, &base);
    if found.charset.as_deref().is_some_and(|c| !is_utf8(c)) || found.is_empty() {
        return None;
    }
    // A page on the web may name a picture on the user's own network; that
    // is not fetched and published, unless the page itself is there.
    let local = is_local(&uri) || is_local(&base);
    let image = found.image.filter(|src| local || !is_local(src));
    Some(Page {
        title: shorten(&found.title, MAX_TITLE_GRAPHEMES),
        description: shorten(&found.description, MAX_DESCRIPTION_GRAPHEMES),
        uri,
        image,
        local,
    })
}

/// The card for the first link in `text`, its picture uploaded as the
/// account `client` is logged in to; `None` when there is no link or its
/// page gives no card.
pub(crate) fn fetch(client: &Client, text: &str) -> Option<PostExternal> {
    let p = read(text)?;
    let agent = crate::api::agent();
    let thumb = p
        .image
        .as_deref()
        .and_then(|src| picture(&agent, src, p.local))
        .and_then(|pic| client.upload_blob(&pic.bytes, pic.mime).ok());
    Some(PostExternal {
        uri: p.uri,
        title: p.title,
        description: p.description,
        thumb,
    })
}

/// The start of the HTML page at `uri` and the address it came from after
/// any redirect, which its relative picture address is read against.
fn page(agent: &ureq::Agent, uri: &str) -> Option<(String, String)> {
    let mut resp = agent
        .get(request_uri(uri).as_ref())
        .config()
        .timeout_global(Some(TIMEOUT))
        .build()
        .header("Accept", "text/html,application/xhtml+xml;q=0.9,*/*;q=0.1")
        .call()
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    // A link straight to a picture or a download is not read at all.
    if let Some(kind) = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
    {
        let kind = kind.to_ascii_lowercase();
        if !kind.contains("html") || crate::og::charset_of(&kind).is_some_and(|c| !is_utf8(&c)) {
            return None;
        }
    }
    let base = resp.get_uri().to_string();
    let mut bytes = Vec::new();
    resp.body_mut()
        .as_reader()
        .take(MAX_PAGE_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    Some((utf8_start(&bytes)?, base))
}

/// The page as text when its head, which holds the card, is UTF-8; a stray
/// byte in the body does not matter. Reading stopped at the limit may have
/// cut the last character, which is dropped. Text in another encoding would
/// show garbled on the card, so it gives none.
fn utf8_start(bytes: &[u8]) -> Option<String> {
    let head = ["</head", "<body"]
        .iter()
        .filter_map(|tag| {
            bytes
                .windows(tag.len())
                .position(|w| w.eq_ignore_ascii_case(tag.as_bytes()))
        })
        .min()
        .unwrap_or(bytes.len());
    match std::str::from_utf8(&bytes[..head]) {
        Ok(_) => Some(String::from_utf8_lossy(bytes).into_owned()),
        Err(e) if e.error_len().is_none() && head == bytes.len() => {
            Some(String::from_utf8_lossy(&bytes[..e.valid_up_to()]).into_owned())
        }
        Err(_) => None,
    }
}

/// `uri` as it goes on the request line. A link may hold text in any
/// script (a Japanese page name), and the HTTP client sends it as raw UTF-8,
/// which servers answer with 400; each byte outside printable ASCII, and
/// the few printable ones an address may not hold, is percent-encoded. An
/// escape already there stays as it is.
fn request_uri(uri: &str) -> Cow<'_, str> {
    let escaped = |b: u8| !b.is_ascii_graphic() || b"\"<>\\^`{|}".contains(&b);
    if !uri.bytes().any(escaped) {
        return Cow::Borrowed(uri);
    }
    let mut out = String::with_capacity(uri.len() * 3);
    for b in uri.bytes() {
        if escaped(b) {
            out.push_str(&format!("%{b:02X}"));
        } else {
            out.push(char::from(b));
        }
    }
    Cow::Owned(out)
}

/// Whether text in `charset` reads as UTF-8: ASCII is a part of it.
fn is_utf8(charset: &str) -> bool {
    matches!(charset, "utf-8" | "utf8" | "us-ascii" | "ascii")
}

/// Whether `url` names a host on this machine or its own network: a
/// loopback, private, link-local or unspecified address, or a name that
/// resolves to one.
fn is_local(url: &str) -> bool {
    let Ok(uri) = url.parse::<ureq::http::Uri>() else {
        return false;
    };
    let Some(host) = uri.host() else {
        return false;
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = host.parse::<IpAddr>() {
        return is_local_ip(ip);
    }
    let port = uri.port_u16().unwrap_or(80);
    (host, port)
        .to_socket_addrs()
        .is_ok_and(|mut addrs| addrs.any(|a| is_local_ip(a.ip())))
}

fn is_local_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                // 100.64.0.0/10, shared by carriers and VPNs such as Tailscale.
                || (a == 100 && (b & 0xc0) == 64)
        }
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_local_ip(IpAddr::V4(v4)),
            None => {
                let first = v6.segments()[0];
                v6.is_loopback()
                    || v6.is_unspecified()
                    || (first & 0xfe00) == 0xfc00
                    || (first & 0xffc0) == 0xfe80
            }
        },
    }
}

/// The picture at `src`, shrunk for a card; not one a redirect took onto
/// the user's own network when the page is not there.
fn picture(agent: &ureq::Agent, src: &str, page_local: bool) -> Option<Prepared> {
    let mut resp = agent
        .get(request_uri(src).as_ref())
        .config()
        .timeout_global(Some(TIMEOUT))
        .build()
        .call()
        .ok()?;
    if !resp.status().is_success() || (!page_local && is_local(&resp.get_uri().to_string())) {
        return None;
    }
    let bytes = resp
        .body_mut()
        .with_config()
        .limit(MAX_PICTURE_BYTES)
        .read_to_vec()
        .ok()?;
    crate::media::prepare_bytes(&bytes, THUMB_SIDE)
}

/// `text` cut to `max` characters as people count them, an ellipsis
/// marking the cut.
fn shorten(text: &str, max: usize) -> String {
    if text.graphemes(true).nth(max).is_none() {
        return text.to_string();
    }
    let mut cut: String = text.graphemes(true).take(max.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_link_is_the_one_written_first() {
        assert_eq!(first_link("no link #tag @alice.test"), None);
        assert_eq!(
            first_link("見て example.com/a と https://b.test/x"),
            Some("https://example.com/a".into())
        );
        // A link glued after a tag is found after the tag, yet still first.
        assert_eq!(
            first_link("#tag(https://a.test) https://b.test"),
            Some("https://a.test".into())
        );
        assert_eq!(
            first_link("Title\nhttps://debimate.jp/notes/x/"),
            Some("https://debimate.jp/notes/x/".into())
        );
    }

    #[test]
    fn only_a_utf8_head_is_read_and_a_cut_last_character_dropped() {
        assert_eq!(utf8_start("日本".as_bytes()).as_deref(), Some("日本"));
        assert_eq!(utf8_start(&"日本".as_bytes()[..5]).as_deref(), Some("日"));
        // Shift_JIS for 日本: in the head it gives no text; in the body only,
        // the head is still read.
        let sjis = [0x93, 0xfa, 0x96, 0x7b];
        assert_eq!(utf8_start(&sjis), None);
        let body_only = [b"<head><title>t</title></HEAD><body>".as_slice(), &sjis].concat();
        assert!(utf8_start(&body_only).is_some_and(|t| t.starts_with("<head><title>t</title>")));
        let cut_in_head = [&"日".as_bytes()[..2], b"</head>"].concat();
        assert_eq!(utf8_start(&cut_in_head), None);
        assert!(is_utf8("utf-8") && is_utf8("us-ascii") && !is_utf8("shift_jis"));
    }

    // A page may make the card point at the user's own network; those
    // addresses are known by number and by name.
    #[test]
    fn addresses_on_the_users_own_network_are_local() {
        for url in [
            "http://127.0.0.1:8080/a.png",
            "http://10.0.0.1/a",
            "http://192.168.11.60/a",
            "http://172.16.0.1/a",
            "http://169.254.169.254/latest",
            "http://100.100.1.1/a",
            "http://0.0.0.0/a",
            "http://[::1]/a",
            "http://[fd00::1]/a",
            "http://[fe80::1]/a",
            "http://[::ffff:192.168.0.1]/a",
            "http://localhost/a",
        ] {
            assert!(is_local(url), "{url}");
        }
        for url in [
            "https://8.8.8.8/a",
            "https://[2001:4860:4860::8888]/a",
            "https://100.128.0.1/a",
            "not a url",
        ] {
            assert!(!is_local(url), "{url}");
        }
    }

    // A link as people write it may hold text in any script; the request
    // line holds only ASCII, and a server answers raw UTF-8 there with 400.
    #[rstest::rstest]
    #[case("https://debimate.jp/notes/x/", "https://debimate.jp/notes/x/")]
    #[case(
        "https://debimate.jp/post/ja/2026-11-17-golangアプリ層/",
        "https://debimate.jp/post/ja/2026-11-17-golang%E3%82%A2%E3%83%97%E3%83%AA%E5%B1%A4/"
    )]
    #[case(
        "https://a.test/%E3%82%A2?q=日本#見出し",
        "https://a.test/%E3%82%A2?q=%E6%97%A5%E6%9C%AC#%E8%A6%8B%E5%87%BA%E3%81%97"
    )]
    #[case(
        "https://a.test/👨‍👩‍👧‍👦",
        "https://a.test/%F0%9F%91%A8%E2%80%8D%F0%9F%91%A9%E2%80%8D%F0%9F%91%A7%E2%80%8D%F0%9F%91%A6"
    )]
    #[case(
        "https://a.test/a|b{c}^d`e\\f\"g<h>",
        "https://a.test/a%7Cb%7Bc%7D%5Ed%60e%5Cf%22g%3Ch%3E"
    )]
    #[case("https://a.test/(a)?b=c&d=e", "https://a.test/(a)?b=c&d=e")]
    fn a_link_is_requested_with_its_text_percent_encoded(#[case] link: &str, #[case] want: &str) {
        assert_eq!(request_uri(link), want);
        assert!(want.parse::<ureq::http::Uri>().is_ok(), "{want}");
    }

    /// A server on loopback that answers like a strict one: 400 to a request
    /// line holding anything but ASCII, else `body` as `kind`. It answers
    /// one request and gives back its request line.
    fn strict_server(
        kind: &'static str,
        body: Vec<u8>,
    ) -> (String, std::thread::JoinHandle<Vec<u8>>) {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let served = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = Vec::new();
            reader.read_until(b'\n', &mut line).unwrap();
            let mut header = Vec::new();
            while reader.read_until(b'\n', &mut header).unwrap() > 2 {
                header.clear();
            }
            if line.is_ascii() {
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(head.as_bytes()).unwrap();
                stream.write_all(&body).unwrap();
            } else {
                stream
                    .write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .unwrap();
            }
            line
        });
        (base, served)
    }

    #[test]
    fn a_page_and_a_picture_named_in_japanese_are_asked_for_percent_encoded() {
        let agent = crate::api::agent();
        let html = r#"<head><meta property="og:title" content="アプリ層"></head>"#;
        let (base, served) = strict_server("text/html; charset=utf-8", html.into());
        let got = page(&agent, &format!("{base}/ja/アプリ層👍🏽/"));
        let line = String::from_utf8_lossy(&served.join().unwrap()).into_owned();
        assert!(
            line.starts_with(
                "GET /ja/%E3%82%A2%E3%83%97%E3%83%AA%E5%B1%A4%F0%9F%91%8D%F0%9F%8F%BD/ "
            ),
            "{line}"
        );
        let (got, _) = got.expect("the page is read");
        assert_eq!(crate::og::parse(&got, &base).title, "アプリ層");

        let photo = include_bytes!("../e2e/atago/testdata/photo.png").to_vec();
        let (base, served) = strict_server("image/png", photo);
        let got = picture(&agent, &format!("{base}/写真.png"), true);
        let line = String::from_utf8_lossy(&served.join().unwrap()).into_owned();
        assert!(line.starts_with("GET /%E5%86%99%E7%9C%9F.png "), "{line}");
        assert!(got.is_some());
    }

    #[test]
    fn a_long_title_or_description_is_cut_at_a_whole_character() {
        let family = "👨‍👩‍👧‍👦";
        assert_eq!(shorten("short", 5), "short");
        assert_eq!(shorten("short", 3), "sh…");
        let exact = family.repeat(MAX_TITLE_GRAPHEMES);
        assert_eq!(shorten(&exact, MAX_TITLE_GRAPHEMES), exact);
        let long = shorten(&family.repeat(MAX_TITLE_GRAPHEMES + 1), MAX_TITLE_GRAPHEMES);
        assert_eq!(long.graphemes(true).count(), MAX_TITLE_GRAPHEMES);
        assert!(long.ends_with(&format!("{family}…")));
    }
}
