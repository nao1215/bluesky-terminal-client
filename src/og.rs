//! What a page offers for a link card: its Open Graph title, description
//! and picture, read from the `<head>` of its HTML.
//!
//! The page comes from the web, so this reads any text without panicking
//! and in one pass: a tag, comment, or script left open ends the reading
//! rather than being searched for again from every later `<`. It uses only
//! the standard library, so the fuzz target can include it by path.

/// The card a page describes. Empty strings where the page says nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Page {
    pub title: String,
    pub description: String,
    /// An absolute `http://` or `https://` URL.
    pub image: Option<String>,
    /// The character encoding the page says it is in (`<meta charset>` or
    /// its `http-equiv` form), lower case.
    pub charset: Option<String>,
}

impl Page {
    /// Whether the page gave nothing to show.
    pub fn is_empty(&self) -> bool {
        self.title.is_empty() && self.description.is_empty() && self.image.is_none()
    }
}

/// Read the card of the page at `url` from its HTML. `og:` properties come
/// first, then `twitter:` ones, then the plain `description` and `<title>`.
pub fn parse(html: &str, url: &str) -> Page {
    let head = head_of(html);
    let mut metas: Vec<(String, String)> = Vec::new();
    let mut title = None;
    let mut charset = None;
    let mut pos = 0;
    while let Some(lt) = head[pos..].find('<') {
        let at = pos + lt + 1;
        let rest = &head[at..];
        if rest.starts_with("!--") {
            let Some(end) = rest.find("-->") else { break };
            pos = at + end + 3;
        } else if let Some(after) = tag_named(rest, "meta") {
            let Some((attrs, used)) = attributes(&rest[after..]) else {
                break;
            };
            pos = at + after + used;
            let content = attrs.iter().find(|(k, _)| k == "content");
            if charset.is_none() {
                charset = attrs
                    .iter()
                    .find(|(k, _)| k == "charset")
                    .map(|(_, v)| v.clone())
                    .or_else(|| {
                        let equiv = attrs.iter().any(|(k, v)| {
                            k == "http-equiv" && v.trim().eq_ignore_ascii_case("content-type")
                        });
                        equiv
                            .then(|| content.and_then(|(_, v)| charset_of(v)))
                            .flatten()
                    })
                    .map(|c| c.trim().trim_matches(['"', '\'']).to_ascii_lowercase())
                    .filter(|c| !c.is_empty());
            }
            // `<meta name="title" property="og:title">` counts under both.
            if let Some((_, content)) = content {
                for (_, key) in attrs.iter().filter(|(k, _)| k == "property" || k == "name") {
                    metas.push((key.trim().to_ascii_lowercase(), content.clone()));
                }
            }
        } else if let Some(after) = tag_named(rest, "title") {
            let Some(open) = rest[after..].find('>') else {
                break;
            };
            let body = &rest[after + open + 1..];
            let Some(close) = find_ci(body, "</title") else {
                break;
            };
            if title.is_none() {
                title = Some(body[..close].to_string());
            }
            pos = at + after + open + 1 + close;
        } else if let Some(after) = ["script", "style"]
            .into_iter()
            .find_map(|name| tag_named(rest, name).map(|a| (name, a)))
        {
            let (name, after) = after;
            let Some(close) = find_ci(&rest[after..], &format!("</{name}")) else {
                break;
            };
            pos = at + after + close;
        } else {
            pos = at;
        }
    }
    // The first usable value of the first key that has one; for a picture,
    // usable means a web address, so a `data:` og:image falls to the next.
    let pick = |keys: &[&str], usable: &dyn Fn(String) -> Option<String>| {
        keys.iter().find_map(|key| {
            metas
                .iter()
                .filter(|(k, _)| k == key)
                .find_map(|(_, v)| Some(clean(v)).filter(|v| !v.is_empty()).and_then(usable))
        })
    };
    let meta = |keys: &[&str]| pick(keys, &Some);
    Page {
        title: meta(&["og:title", "twitter:title"])
            .or_else(|| title.map(|t| clean(&t)))
            .unwrap_or_default(),
        description: meta(&["og:description", "twitter:description", "description"])
            .unwrap_or_default(),
        image: pick(
            &[
                "og:image",
                "og:image:url",
                "og:image:secure_url",
                "twitter:image",
                "twitter:image:src",
            ],
            &|src| resolve(&src, url),
        ),
        charset,
    }
}

/// The `charset=` parameter of a media type such as `text/html; charset=UTF-8`.
pub fn charset_of(content_type: &str) -> Option<String> {
    content_type.split(';').skip(1).find_map(|param| {
        let (name, value) = param.split_once('=')?;
        name.trim()
            .eq_ignore_ascii_case("charset")
            .then(|| value.trim().trim_matches(['"', '\'']).to_ascii_lowercase())
    })
}

/// The part of the document before its body: the card is in the head, and a
/// `<meta>` in the body is someone's text, not the page's.
fn head_of(html: &str) -> &str {
    let end = [find_ci(html, "</head"), find_ci(html, "<body")]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(html.len());
    &html[..end]
}

/// Where `needle` (ASCII, lower case) first appears in `hay`, in any case.
/// The match starts on an ASCII byte, so on a character boundary.
fn find_ci(hay: &str, needle: &str) -> Option<usize> {
    let (hay, needle) = (hay.as_bytes(), needle.as_bytes());
    hay.windows(needle.len())
        .position(|w| w.eq_ignore_ascii_case(needle))
}

/// When `rest` (the text after a `<`) opens the tag `name`, the byte offset
/// just past the name.
fn tag_named(rest: &str, name: &str) -> Option<usize> {
    let head = rest.as_bytes().get(..name.len())?;
    let next = rest.as_bytes().get(name.len()).copied();
    (head.eq_ignore_ascii_case(name.as_bytes())
        && matches!(
            next,
            Some(b'>' | b'/') | Some(b' ' | b'\t' | b'\n' | b'\r' | b'\x0c')
        ))
    .then_some(name.len())
}

/// The attributes of a tag, names in lower case, and the bytes up to and
/// including its `>`; `None` when the tag never closes.
fn attributes(s: &str) -> Option<(Vec<(String, String)>, usize)> {
    let bytes = s.as_bytes();
    let mut attrs = Vec::new();
    let mut i = 0;
    loop {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
            i += 1;
        }
        if *bytes.get(i)? == b'>' {
            return Some((attrs, i + 1));
        }
        let start = i;
        while i < bytes.len()
            && !matches!(bytes[i], b'=' | b'>' | b'/')
            && !bytes[i].is_ascii_whitespace()
        {
            i += 1;
        }
        let name = s[start..i].to_ascii_lowercase();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if bytes.get(i) != Some(&b'=') {
            attrs.push((name, String::new()));
            continue;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let value = match bytes.get(i)? {
            &q @ (b'"' | b'\'') => {
                let end = s[i + 1..].find(q as char)? + i + 1;
                let v = &s[i + 1..end];
                i = end + 1;
                v
            }
            _ => {
                let start = i;
                while i < bytes.len() && bytes[i] != b'>' && !bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                &s[start..i]
            }
        };
        attrs.push((name, value.to_string()));
    }
}

/// Text as it reads: character references decoded, runs of white space made
/// one space, the ends trimmed.
fn clean(text: &str) -> String {
    decode(text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Decode the character references a page title uses: the named ones of
/// HTML's markup characters and numeric ones. Others stay as written.
fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        // A reference is short: look for its `;` only that far, so a page of
        // `&` without one is read in one pass.
        let reference = rest.as_bytes()[1..]
            .iter()
            .take(11)
            .position(|&b| b == b';')
            .and_then(|end| Some((character(&rest[1..end + 2])?, end + 2)));
        match reference {
            Some((c, used)) => {
                out.push(c);
                rest = &rest[used..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The character `name;` refers to, `name` being what is between `&` and `;`.
fn character(name: &str) -> Option<char> {
    let name = name.strip_suffix(';')?;
    let code = match name.strip_prefix('#') {
        Some(n) => match n.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => n.parse().ok()?,
        },
        None => {
            return match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => None,
            };
        }
    };
    char::from_u32(code).filter(|c| *c != '\0')
}

/// `src` made absolute against the page at `base`, if it is a web address.
fn resolve(src: &str, base: &str) -> Option<String> {
    let src = src.trim();
    let lower = src.get(..8).unwrap_or(src).to_ascii_lowercase();
    let absolute = if lower.starts_with("http://") || lower.starts_with("https://") {
        src.to_string()
    } else {
        let (scheme, after) = base.split_once("://")?;
        let authority_end = after.find(['/', '?', '#']).unwrap_or(after.len());
        let origin = &base[..scheme.len() + 3 + authority_end];
        if let Some(rest) = src.strip_prefix("//") {
            format!("{scheme}://{rest}")
        } else if src.starts_with('/') {
            format!("{origin}{src}")
        } else if src.is_empty() || src.split(['/', '?', '#']).next()?.contains(':') {
            // `data:`, `javascript:` and other schemes are not pictures to fetch.
            return None;
        } else {
            let path = &after[authority_end..];
            let path = &path[..path.find(['?', '#']).unwrap_or(path.len())];
            let dir = &path[..path.rfind('/').map_or(0, |i| i + 1)];
            let dir = if dir.is_empty() { "/" } else { dir };
            format!("{origin}{dir}{src}")
        }
    };
    let ok = {
        let lower = absolute.get(..8).unwrap_or(&absolute).to_ascii_lowercase();
        (lower.starts_with("http://") && absolute.len() > 7)
            || (lower.starts_with("https://") && absolute.len() > 8)
    };
    (ok && !absolute.chars().any(char::is_control)).then(|| absolute.replace(' ', "%20"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "https://example.com/notes/a/?q=1#top";

    fn page(head: &str) -> Page {
        parse(
            &format!("<!doctype html><html><head>{head}</head><body></body></html>"),
            URL,
        )
    }

    #[test]
    fn open_graph_properties_make_the_card() {
        let p = page(
            r#"<meta property="og:title" content="A &amp; B">
               <meta content='Two
                 lines' property=og:description>
               <META PROPERTY="OG:IMAGE" CONTENT="/img/a.png">"#,
        );
        assert_eq!(
            p,
            Page {
                title: "A & B".into(),
                description: "Two lines".into(),
                image: Some("https://example.com/img/a.png".into()),
                charset: None,
            }
        );
    }

    #[test]
    fn twitter_then_plain_tags_stand_in_for_missing_open_graph() {
        let p = page(
            r#"<title>
              The &lt;title&gt; &#x1F600;&#33;
            </title>
            <meta name="description" content="plain">
            <meta name="twitter:image" content="https://cdn.example.net/t.jpg">"#,
        );
        assert_eq!(p.title, "The <title> 😀!");
        assert_eq!(p.description, "plain");
        assert_eq!(p.image.as_deref(), Some("https://cdn.example.net/t.jpg"));

        let p = page(
            r#"<meta name="twitter:title" content="tw"><title>t</title>
               <meta property="og:description" content=""><meta name="description" content="d">"#,
        );
        assert_eq!((p.title.as_str(), p.description.as_str()), ("tw", "d"));
    }

    #[test]
    fn the_first_of_a_property_given_twice_wins() {
        let p = page(
            r#"<meta property="og:title" content="one"><meta property="og:title" content="two">"#,
        );
        assert_eq!(p.title, "one");
    }

    #[test]
    fn tags_in_the_body_comments_and_scripts_are_not_the_card() {
        let p = parse(
            r#"<head><!-- <meta property="og:title" content="comment"> -->
               <script>let s = '<meta property="og:title" content="script">';</script>
               <style>/* <title>style</title> */</style>
               <meta property="og:description" content="head"></head>
               <body><meta property="og:title" content="body"></body>"#,
            URL,
        );
        assert_eq!(p.title, "");
        assert_eq!(p.description, "head");
    }

    #[test]
    fn a_page_without_a_head_is_read_to_its_body() {
        let p = parse(
            r#"<meta property=og:title content=bare><body><title>no</title>"#,
            URL,
        );
        assert_eq!(p.title, "bare");
        assert!(parse("just text", URL).is_empty());
    }

    #[test]
    fn a_tag_left_open_ends_the_reading() {
        for html in [
            r#"<meta property="og:title" content="cut"#,
            r#"<meta property="og:title" content="cut">"#.trim_end_matches('>'),
            "<title>never closed",
            "<script>never closed <meta property=og:title content=x>",
            "<!-- never closed <meta property=og:title content=x>",
        ] {
            assert_eq!(parse(html, URL).title, "", "{html}");
        }
    }

    #[test]
    fn pictures_are_made_absolute_against_the_page() {
        let cases = [
            ("https://x.test/a.png", Some("https://x.test/a.png")),
            ("HTTP://x.test/a.png", Some("HTTP://x.test/a.png")),
            ("//cdn.test/a.png", Some("https://cdn.test/a.png")),
            ("/a b.png", Some("https://example.com/a%20b.png")),
            ("a.png", Some("https://example.com/notes/a/a.png")),
            ("data:image/png;base64,AAAA", None),
            ("javascript:alert(1)", None),
            ("", None),
            ("https://", None),
        ];
        for (src, want) in cases {
            assert_eq!(resolve(src, URL).as_deref(), want, "{src}");
        }
        assert_eq!(
            resolve("a.png", "http://127.0.0.1:8080").as_deref(),
            Some("http://127.0.0.1:8080/a.png")
        );
        assert_eq!(resolve("a.png", "not a url"), None);
    }

    #[test]
    fn unknown_or_broken_references_stay_as_written() {
        assert_eq!(
            clean("a &copy; b &#xZZ; c & d &#0; &#1114112;"),
            "a &copy; b &#xZZ; c & d &#0; &#1114112;"
        );
        assert_eq!(clean("&quot;q&quot;&apos;&nbsp;x"), "\"q\"' x");
    }

    #[test]
    fn the_page_says_its_encoding_either_way() {
        assert_eq!(
            page(r#"<meta charset="Shift_JIS">"#).charset.as_deref(),
            Some("shift_jis")
        );
        assert_eq!(
            page(r#"<meta http-equiv="Content-Type" content="text/html; charset=EUC-JP">"#)
                .charset
                .as_deref(),
            Some("euc-jp")
        );
        assert_eq!(
            page(r#"<meta name="x" content="text/html; charset=EUC-JP">"#).charset,
            None
        );
        assert_eq!(
            charset_of("text/html; charset=\"utf-8\""),
            Some("utf-8".into())
        );
        assert_eq!(charset_of("text/html"), None);
    }

    #[test]
    fn markup_as_pages_write_it_is_read() {
        let p = page(
            r#"<meta property="og:title" content="a > b" />
               <meta itemprop property = 'og:description' content = x>
               <meta name="title" property="og:image:secure_url" content="https://x.test/s.png">"#,
        );
        assert_eq!(p.title, "a > b");
        assert_eq!(p.description, "x");
        assert_eq!(p.image.as_deref(), Some("https://x.test/s.png"));
        assert_eq!(page(r#"<meta charset="">"#).charset, None);
        assert_eq!(parse("<title", URL).title, "");
        assert_eq!(parse(r#"<meta content="#, URL).title, "");
    }

    #[test]
    fn a_picture_that_is_no_web_address_falls_to_the_next_one() {
        let p = page(
            r#"<meta property="og:image" content="data:image/png;base64,AA">
               <meta name="twitter:image" content="/t.png">"#,
        );
        assert_eq!(p.image.as_deref(), Some("https://example.com/t.png"));
        let p = page("<meta property=\"og:image\" content=\"/a\u{1}b.png\">");
        assert_eq!(p.image, None);
        assert_eq!(
            resolve("日本.png", URL).as_deref(),
            Some("https://example.com/notes/a/日本.png")
        );
    }

    // A page of `&` with no `;` after them is read in one pass, not one
    // search to its end per `&`.
    #[test]
    fn many_ampersands_are_read_in_one_pass() {
        let amps = "&".repeat(1 << 20);
        let started = std::time::Instant::now();
        let p = page(&format!(r#"<meta property="og:title" content="{amps}">"#));
        assert_eq!(p.title.len(), 1 << 20);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn text_in_any_script_survives() {
        let p = page(r#"<meta property="og:title" content="Rust 入門 — 日本語 🇯🇵">"#);
        assert_eq!(p.title, "Rust 入門 — 日本語 🇯🇵");
    }
}
