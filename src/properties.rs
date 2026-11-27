//! Properties of the code that lays text out, edits it and reads MFM, checked
//! on generated text: emoji with ZWJ, skin tones, flags and VS16, CJK, half-
//! width kana, combining marks, controls, and the characters MFM gives a
//! meaning to. A run takes a few seconds with the default 256 cases; the
//! nightly workflow (`.github/workflows/properties.yml`) runs them with many
//! more by `PROPTEST_CASES`. A failure prints the smallest text it found,
//! which belongs in a unit test of its own once fixed.

use proptest::prelude::*;

use crate::tui::input::TextInput;
use crate::tui::text::{cells, truncate, wrap};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_segmentation::UnicodeSegmentation;

/// Pieces text is made of: each a grapheme or a short run a note may hold.
const PIECES: &[&str] = &[
    "a",
    "Z",
    "0",
    " ",
    "  ",
    "\n",
    "\t",
    "\r\n",
    "\u{7}",
    "\u{1b}[31m",
    "日",
    "本",
    "語",
    "ｶ",
    "ﾞ",
    "ﾟ",
    "한",
    "é",
    "e\u{301}",
    "\u{200b}",
    "👍",
    "👍🏽",
    "🇯🇵",
    "❤️",
    "🏛️",
    "👨\u{200d}👩\u{200d}👧\u{200d}👦",
    "🐈\u{200d}⬛",
    "*",
    "**",
    "_",
    "~~",
    "`",
    "```",
    "$[",
    "]",
    "[",
    "(",
    ")",
    "<",
    ">",
    "<plain>",
    "</plain>",
    "<small>",
    "</small>",
    ":iruka:",
    "@alice",
    "#tag",
    "https://",
    "http://",
    "?[",
    "https://e.example/a",
    "<https://e.example/b>",
    "[label](https://e.example/c)",
    "?[s](https://e.example/d)",
    "$[x2 ",
    "$[ruby ",
];

fn text() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(PIECES), 0..48).prop_map(|p| p.concat())
}

/// The characters of `s` that show, as the terminal is given them.
fn printable(s: &str) -> String {
    crate::text::drawable(s)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

/// Whether `line` fits `width`, or is one cluster too wide for any line
/// (`あ` with sound marks after it), which a line holds alone.
fn fits(line: &str, width: usize) -> bool {
    cells(line) <= width || line.graphemes(true).count() == 1
}

/// A key the text field takes.
#[derive(Debug, Clone)]
enum Op {
    Key(KeyCode),
    Paste(String),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        prop::sample::select(PIECES).prop_map(|p| Op::Paste(p.to_string())),
        prop::sample::select(&['a', 'あ', ' ', '👍'][..]).prop_map(|c| Op::Key(KeyCode::Char(c))),
        prop::sample::select(
            &[
                KeyCode::Left,
                KeyCode::Right,
                KeyCode::Up,
                KeyCode::Down,
                KeyCode::Home,
                KeyCode::End,
                KeyCode::Backspace,
                KeyCode::Delete,
                KeyCode::Enter,
            ][..]
        )
        .prop_map(Op::Key),
    ]
}

proptest! {
    // Every line fits the width, and wrapping drops only the spaces at the
    // breaks: the characters that show are all there, in order.
    #[test]
    fn wrap_fits_and_keeps_every_character(s in text(), width in 2usize..40) {
        let lines = wrap(&s, width);
        for l in &lines {
            prop_assert!(fits(l, width), "{l:?} is wider than {width}");
        }
        prop_assert_eq!(printable(&lines.concat()), printable(&s));
    }

    // A cut fits the width and is the start of the text, with … when cut.
    #[test]
    fn truncate_fits_and_is_a_prefix(s in text(), width in 1usize..40) {
        let shown = crate::text::drawable(&s).into_owned();
        let cut = truncate(&s, width);
        prop_assert!(cells(&cut) <= width, "{cut:?} is wider than {width}");
        if cells(&shown) <= width {
            prop_assert_eq!(cut, shown);
        } else {
            let kept = cut.strip_suffix('…').expect("a cut ends with …");
            prop_assert!(shown.starts_with(kept), "{kept:?} is not the start of {shown:?}");
        }
    }

    // Whatever is typed or pasted, the field never panics, its layout fits
    // the width and puts the cursor on a line it has, and text added at the
    // end is the end of the text.
    #[test]
    fn the_text_field_stays_whole(
        ops in prop::collection::vec(op(), 0..40),
        multiline in any::<bool>(),
        width in 2usize..30,
    ) {
        let mut field = if multiline { TextInput::multi("") } else { TextInput::single("") };
        for op in ops {
            match op {
                Op::Key(code) => {
                    field.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
                }
                Op::Paste(s) => field.insert_str(&s),
            }
            let layout = field.layout(width);
            for l in &layout.lines {
                prop_assert!(fits(l, width), "{l:?} is wider than {width}");
            }
            prop_assert!(layout.cursor.0 <= layout.lines.len(), "{layout:?}");
            prop_assert!(layout.cursor.1 <= width, "{layout:?}");
        }
        let before = field.text();
        if !multiline {
            field.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
            field.insert_str("xyz");
            prop_assert_eq!(field.text(), format!("{before}xyz"));
        }
    }

    // Silencing a note's links leaves none that gets a preview and the
    // same text to read; doing it twice changes nothing more to read.
    #[test]
    fn silenced_links_get_no_preview_and_read_the_same(s in text()) {
        let quiet = crate::mfm::silenced(&s);
        prop_assert!(crate::mfm::previewed(&quiet, &[]).is_empty(), "{quiet:?}");
        prop_assert_eq!(crate::mfm::plain_text(&quiet), crate::mfm::plain_text(&s));
        let twice = crate::mfm::silenced(&quiet);
        prop_assert_eq!(crate::mfm::plain_text(&twice), crate::mfm::plain_text(&s));
    }

    // The plain text of a note never has more to show than the note.
    #[test]
    fn plain_text_never_grows(s in text()) {
        let plain = crate::mfm::plain_text(&s);
        prop_assert!(printable(&plain).chars().count() <= printable(&s).chars().count(), "{plain:?}");
    }
}
