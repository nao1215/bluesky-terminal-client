//! Inline custom emoji cells, independent of the markup of a client.
use crate::media::images::Images;
use ratatui::{Frame, layout::Rect};

/// The two cells a custom emoji's picture is drawn over: what is laid out
/// in its place, and found again on screen to draw it.
pub const EMOJI_CELLS: &str = "\u{E000}\u{E001}";

/// `text` with each `:name:` that `picture` has a picture for put as
/// [`EMOJI_CELLS`], the pictures' addresses pushed in order.
pub fn with_emoji_cells(
    text: &str,
    picture: &mut dyn FnMut(&str) -> Option<String>,
    urls: &mut Vec<String>,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(':') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let end = after.find(|c: char| {
            !(c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-' | '@' | '.'))
        });
        match end {
            Some(e) if e > 0 && after[e..].starts_with(':') => {
                let name = &after[..e];
                match picture(name) {
                    Some(url) => {
                        urls.push(url);
                        out.push_str(EMOJI_CELLS);
                        rest = &after[e + 1..];
                    }
                    None => {
                        out.push(':');
                        rest = after;
                    }
                }
            }
            _ => {
                out.push(':');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Draw `urls` over the [`EMOJI_CELLS`] of `area`, in order, row by row.
pub fn draw_emoji_cells(frame: &mut Frame, area: Rect, urls: &[String], images: &mut Images) {
    if urls.is_empty() {
        return;
    }
    let area = area.intersection(frame.area());
    let mut found = Vec::new();
    {
        let buf = frame.buffer_mut();
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if buf[(x, y)].symbol() == "\u{E000}" {
                    found.push((x, y));
                }
            }
        }
    }
    for ((x, y), url) in found.into_iter().zip(urls) {
        // Blank under the picture: one still loading draws a mark in the
        // first cell only.
        {
            let buf = frame.buffer_mut();
            buf[(x, y)].set_symbol(" ");
            if x + 1 < area.right() {
                buf[(x + 1, y)].set_symbol(" ");
            }
        }
        images.draw(
            frame,
            Rect {
                x,
                y,
                width: 2,
                height: 1,
            },
            url,
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_names_and_unicode_stay_text() {
        let mut urls = Vec::new();
        let text = with_emoji_cells(
            "猫 :cat: :unknown: 👨‍👩‍👧",
            &mut |name| (name == "cat").then(|| "https://example.test/cat.png".into()),
            &mut urls,
        );
        assert_eq!(text, format!("猫 {EMOJI_CELLS} :unknown: 👨‍👩‍👧"));
        assert_eq!(urls, ["https://example.test/cat.png"]);
    }
}
