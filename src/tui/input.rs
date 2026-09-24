//! A small text editor for the login form, the composer, and the profile editor.
//!
//! The buffer is a `Vec<char>` with a cursor index. The cursor only ever
//! stops between grapheme clusters (what a reader sees as one character), so
//! an emoji with a skin tone, a ZWJ family, a flag, or a letter with a
//! combining accent is stepped over, deleted, and wrapped as one. Layout wraps
//! by display width at those boundaries, so CJK text (which has no spaces)
//! wraps too.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::tui::text::drawable;

/// One editable text field.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextInput {
    chars: Vec<char>,
    cursor: usize,
    multiline: bool,
    masked: bool,
}

/// The visual layout of a field at a given width.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// Visual lines.
    pub lines: Vec<String>,
    /// Cursor position as (row, display column).
    pub cursor: (usize, usize),
}

impl TextInput {
    /// A single-line field.
    pub fn single(text: &str) -> Self {
        let chars: Vec<char> = drawable(text).chars().filter(|c| *c != '\n').collect();
        let cursor = chars.len();
        Self {
            chars,
            cursor,
            multiline: false,
            masked: false,
        }
    }

    /// A multi-line field; Enter inserts a newline.
    pub fn multi(text: &str) -> Self {
        let chars: Vec<char> = drawable(text).chars().collect();
        let cursor = chars.len();
        Self {
            chars,
            cursor,
            multiline: true,
            masked: false,
        }
    }

    /// A single-line field that renders every character as a bullet.
    pub fn password() -> Self {
        Self {
            masked: true,
            ..Self::single("")
        }
    }

    /// The current text.
    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    /// Whether the field holds no text.
    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    /// Insert text at the cursor (newlines are dropped in single-line fields).
    pub fn insert_str(&mut self, s: &str) {
        let mut after_break = false;
        let mut pasted = Vec::new();
        for c in drawable(s).chars() {
            // One line takes a pasted paragraph as one line: a run of line
            // breaks is a space, so the words on either side stay apart.
            if c == '\n' && !self.multiline {
                if !after_break {
                    pasted.push(' ');
                }
                after_break = true;
                continue;
            }
            after_break = false;
            pasted.push(c);
        }
        // In at once: a character at a time moves what follows once per
        // character, which a long paste into long text makes slow.
        let n = pasted.len();
        self.chars.splice(self.cursor..self.cursor, pasted);
        self.cursor += n;
        self.snap_to_stop();
    }

    /// Move the cursor on to the end of the cluster it is inside: what was
    /// typed can join the character after it (a regional indicator before a
    /// flag, a ZWJ before an emoji) into one.
    fn snap_to_stop(&mut self) {
        if let Some(&stop) = self.stops().iter().find(|&&s| s >= self.cursor) {
            self.cursor = stop;
        }
    }

    fn insert(&mut self, c: char) {
        self.chars.insert(self.cursor, c);
        self.cursor += 1;
    }

    /// Apply an editing key. Returns false when the key is not an edit, so
    /// the caller can treat it as a command (Tab, Esc, Ctrl+S, ...).
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char(c) if !ctrl => {
                self.insert(c);
                self.snap_to_stop();
            }
            KeyCode::Char('a') if ctrl => self.cursor = self.line_start(),
            KeyCode::Char('e') if ctrl => self.cursor = self.line_end(),
            KeyCode::Char('u') if ctrl => {
                let start = self.line_start();
                self.chars.drain(start..self.cursor);
                self.cursor = start;
            }
            KeyCode::Enter if self.multiline => self.insert('\n'),
            KeyCode::Backspace => {
                let prev = self.prev_stop();
                self.chars.drain(prev..self.cursor);
                self.cursor = prev;
            }
            KeyCode::Delete => {
                let next = self.next_stop();
                self.chars.drain(self.cursor..next);
            }
            KeyCode::Left => self.cursor = self.prev_stop(),
            KeyCode::Right => self.cursor = self.next_stop(),
            KeyCode::Home => self.cursor = self.line_start(),
            KeyCode::End => self.cursor = self.line_end(),
            KeyCode::Up if self.multiline => self.vertical(-1),
            KeyCode::Down if self.multiline => self.vertical(1),
            _ => return false,
        }
        true
    }

    /// The char indices the cursor may stop at: every grapheme cluster
    /// boundary, from 0 to the end of the text.
    fn stops(&self) -> Vec<usize> {
        let text = self.text();
        let mut stops = vec![0];
        let mut at = 0;
        for g in text.graphemes(true) {
            at += g.chars().count();
            stops.push(at);
        }
        stops
    }

    fn prev_stop(&self) -> usize {
        self.stops()
            .into_iter()
            .rev()
            .find(|&i| i < self.cursor)
            .unwrap_or(0)
    }

    fn next_stop(&self) -> usize {
        self.stops()
            .into_iter()
            .find(|&i| i > self.cursor)
            .unwrap_or(self.chars.len())
    }

    /// How many clusters lie between the char indices `from` and `to`.
    fn clusters_between(stops: &[usize], from: usize, to: usize) -> usize {
        stops.iter().filter(|&&i| i > from && i <= to).count()
    }

    /// The char index `n` clusters after `from`, but not past `limit`.
    fn advance(stops: &[usize], from: usize, n: usize, limit: usize) -> usize {
        if n == 0 {
            return from;
        }
        stops
            .iter()
            .copied()
            .filter(|&i| i > from && i <= limit)
            .nth(n - 1)
            .unwrap_or(limit)
    }

    fn line_start(&self) -> usize {
        self.chars[..self.cursor]
            .iter()
            .rposition(|c| *c == '\n')
            .map_or(0, |i| i + 1)
    }

    fn line_end(&self) -> usize {
        self.chars[self.cursor..]
            .iter()
            .position(|c| *c == '\n')
            .map_or(self.chars.len(), |i| self.cursor + i)
    }

    /// Move to the same column, counted in clusters, of the previous or
    /// next logical line.
    fn vertical(&mut self, dir: i32) {
        let stops = self.stops();
        let col = Self::clusters_between(&stops, self.line_start(), self.cursor);
        if dir < 0 {
            let start = self.line_start();
            if start == 0 {
                self.cursor = 0;
                return;
            }
            self.cursor = start - 1;
            let prev_start = self.line_start();
            self.cursor = Self::advance(&stops, prev_start, col, start - 1);
        } else {
            let end = self.line_end();
            if end == self.chars.len() {
                self.cursor = end;
                return;
            }
            self.cursor = end + 1;
            let next_end = self.line_end();
            self.cursor = Self::advance(&stops, end + 1, col, next_end);
        }
    }

    /// Lay the text out in `width` columns (at least 1).
    pub fn layout(&self, width: usize) -> Layout {
        let width = width.max(1);
        let mut lines = vec![String::new()];
        let mut col = 0usize;
        let mut cursor = (0, 0);
        let text = self.text();
        let mut i = 0;
        for g in text.graphemes(true) {
            // Drawn where the next character goes: in the room left on this
            // line, else at the start of the next.
            if i == self.cursor {
                cursor = if col < width {
                    (lines.len() - 1, col)
                } else {
                    (lines.len(), 0)
                };
            }
            i += g.chars().count();
            if g == "\n" {
                lines.push(String::new());
                col = 0;
                continue;
            }
            let shown = if self.masked { "•" } else { g };
            let w = shown.width();
            if col + w > width {
                lines.push(String::new());
                col = 0;
            }
            lines.last_mut().unwrap().push_str(shown);
            col += w;
        }
        if self.cursor == self.chars.len() {
            if col >= width {
                lines.push(String::new());
                col = 0;
            }
            cursor = (lines.len() - 1, col);
        }
        // Only zero-width characters after a full line: the line the cursor
        // is on has nothing on it yet.
        if cursor.0 == lines.len() {
            lines.push(String::new());
        }
        Layout { lines, cursor }
    }
}

#[cfg(test)]
mod tests {
    // A field filled from the server, where another client may have
    // written CRLF or tabs, is laid out as it is drawn.
    #[test]
    fn a_field_from_the_server_keeps_lines_and_cursor_in_step() {
        let multi = TextInput::multi("line one\r\nline two\rthree");
        assert_eq!(multi.text(), "line one\nline two\nthree");
        assert_eq!(multi.layout(40).lines, ["line one", "line two", "three"]);
        let single = TextInput::single("a\tb\u{7}");
        assert_eq!(single.text(), "a    b");
        assert_eq!(single.layout(20).cursor, (0, 6));
    }

    use super::*;

    // A field of one line (alt text, a display name) takes a pasted
    // paragraph as one line: its line breaks become spaces, rather than the
    // words on either side running together.
    #[rstest::rstest]
    #[case("A dog\nrunning", "A dog running")]
    #[case("A dog\r\n\r\nrunning", "A dog running")]
    #[case("犬が\n走る 🐕", "犬が 走る 🐕")]
    #[case("trailing\n", "trailing ")]
    fn a_paste_into_one_line_turns_line_breaks_into_spaces(
        #[case] paste: &str,
        #[case] want: &str,
    ) {
        let mut input = TextInput::single("");
        input.insert_str(paste);
        assert_eq!(input.text(), want);
        let mut multi = TextInput::multi("");
        multi.insert_str("a\nb");
        assert_eq!(multi.text(), "a\nb");
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn typed(input: &mut TextInput, s: &str) {
        for c in s.chars() {
            input.handle_key(key(KeyCode::Char(c)));
        }
    }

    #[test]
    fn typing_and_backspace_edit_at_the_cursor() {
        let mut t = TextInput::single("");
        typed(&mut t, "helo");
        t.handle_key(key(KeyCode::Left));
        typed(&mut t, "l");
        assert_eq!(t.text(), "hello");
        t.handle_key(key(KeyCode::End));
        t.handle_key(key(KeyCode::Backspace));
        assert_eq!(t.text(), "hell");
    }

    #[test]
    fn enter_is_a_newline_only_in_multiline_fields() {
        let mut single = TextInput::single("a");
        assert!(!single.handle_key(key(KeyCode::Enter)));
        let mut multi = TextInput::multi("a");
        assert!(multi.handle_key(key(KeyCode::Enter)));
        assert_eq!(multi.text(), "a\n");
    }

    #[test]
    fn paste_turns_newlines_into_a_space_in_single_line_fields() {
        let mut t = TextInput::single("");
        t.insert_str("abc\r\ndef");
        assert_eq!(t.text(), "abc def");
    }

    #[test]
    fn command_keys_are_not_consumed() {
        let mut t = TextInput::multi("");
        assert!(!t.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)));
        assert!(!t.handle_key(key(KeyCode::Tab)));
        assert!(!t.handle_key(key(KeyCode::Esc)));
    }

    #[test]
    fn vertical_moves_keep_the_column_when_possible() {
        let mut t = TextInput::multi("abcd\nxy\nlmnop");
        // cursor at end of "lmnop" (col 5) -> up to "xy" clamps to col 2
        t.handle_key(key(KeyCode::Up));
        typed(&mut t, "!");
        assert_eq!(t.text(), "abcd\nxy!\nlmnop");
        t.handle_key(key(KeyCode::Up));
        typed(&mut t, "?");
        assert_eq!(t.text(), "abc?d\nxy!\nlmnop");
    }

    #[test]
    fn layout_wraps_wide_characters_by_display_width() {
        let t = TextInput::multi("日本語です");
        let l = t.layout(4);
        assert_eq!(l.lines, ["日本", "語で", "す"]);
        assert_eq!(l.cursor, (2, 2));
    }

    #[test]
    fn cursor_at_a_full_line_moves_to_the_next_row() {
        let t = TextInput::single("abcd");
        assert_eq!(t.layout(4).cursor, (1, 0));
    }

    #[test]
    fn password_is_masked_but_text_is_kept() {
        let mut t = TextInput::password();
        typed(&mut t, "secret");
        assert_eq!(t.layout(20).lines, ["••••••"]);
        assert_eq!(t.text(), "secret");
    }

    /// Emoji and other characters made of several code points, each with
    /// the text before and after it so the edits land between them.
    const CLUSTERS: &[&str] = &["👍🏽", "👨‍👩‍👧‍👦", "🇯🇵", "1️⃣", "❤️", "e\u{301}"];

    #[test]
    fn backspace_and_delete_remove_a_whole_emoji() {
        for c in CLUSTERS {
            let mut t = TextInput::single(&format!("a{c}"));
            t.handle_key(key(KeyCode::Backspace));
            assert_eq!(t.text(), "a", "backspace after {c:?}");

            let mut t = TextInput::single(&format!("{c}b"));
            t.handle_key(key(KeyCode::Home));
            t.handle_key(key(KeyCode::Delete));
            assert_eq!(t.text(), "b", "delete before {c:?}");
        }
    }

    #[test]
    fn the_cursor_steps_over_an_emoji_and_never_lands_inside_it() {
        for c in CLUSTERS {
            let mut t = TextInput::single(&format!("a{c}b"));
            t.handle_key(key(KeyCode::Left));
            t.handle_key(key(KeyCode::Left));
            typed(&mut t, "x");
            assert_eq!(t.text(), format!("ax{c}b"), "left over {c:?}");
            t.handle_key(key(KeyCode::Right));
            typed(&mut t, "y");
            assert_eq!(t.text(), format!("ax{c}yb"), "right over {c:?}");
        }
    }

    #[test]
    fn an_emoji_takes_its_display_width_and_is_never_split_across_lines() {
        let t = TextInput::single("👨‍👩‍👧‍👦");
        assert_eq!(t.layout(20).cursor, (0, 2));
        let t = TextInput::single("a👍🏽");
        assert_eq!(t.layout(20).cursor, (0, 3));
        let t = TextInput::multi("👍🏽👍🏽👍🏽");
        let l = t.layout(4);
        assert_eq!(l.lines, ["👍🏽👍🏽", "👍🏽"]);
        assert_eq!(l.cursor, (1, 2));
    }

    #[test]
    fn vertical_moves_count_an_emoji_as_one_column_step() {
        let mut t = TextInput::multi("👨‍👩‍👧‍👦x\nab");
        t.handle_key(key(KeyCode::Up));
        typed(&mut t, "!");
        assert_eq!(t.text(), "👨‍👩‍👧‍👦x!\nab");
    }

    #[test]
    fn vertical_moves_from_a_line_start_stay_at_the_line_start() {
        let mut t = TextInput::multi("👍🏽x\nab");
        t.handle_key(key(KeyCode::Home));
        t.handle_key(key(KeyCode::Up));
        typed(&mut t, "!");
        assert_eq!(t.text(), "!👍🏽x\nab");
    }

    #[test]
    fn ctrl_u_clears_to_line_start() {
        let mut t = TextInput::multi("keep\ndrop me");
        t.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert_eq!(t.text(), "keep\n");
    }

    // A character that joins the one after it (a regional indicator before
    // a flag, a ZWJ before an emoji) leaves the cursor after the cluster it
    // made, where it is drawn, not inside it.
    #[test]
    fn typing_into_a_cluster_leaves_the_cursor_after_it() {
        for (text, typed) in [("🇺🇸", "🇯"), ("👩", "👨\u{200d}")] {
            let mut t = TextInput::multi(text);
            t.handle_key(key(KeyCode::Home));
            for c in typed.chars() {
                t.handle_key(key(KeyCode::Char(c)));
            }
            let drawn = t.layout(40).cursor;
            t.handle_key(key(KeyCode::Char('x')));
            let x = t.text().find('x').unwrap();
            let col = crate::tui::text::cells(&t.text()[..x]);
            assert_eq!(drawn, (0, col), "{:?}", t.text());
        }
    }

    // The cursor is drawn where the next character goes: before a wide
    // character moved to the next line, the gap it left on this one; before
    // a zero-width one at the end of a full line, the next line.
    #[test]
    fn the_cursor_is_drawn_where_the_next_character_goes() {
        let mut t = TextInput::single("ab日");
        t.handle_key(key(KeyCode::Left));
        assert_eq!(t.layout(3).cursor, (0, 2));
        t.handle_key(key(KeyCode::Char('x')));
        assert_eq!(t.layout(3).lines, ["abx", "日"]);

        let mut t = TextInput::single("abc\u{200b}d");
        t.handle_key(key(KeyCode::Left));
        t.handle_key(key(KeyCode::Left));
        assert_eq!(t.layout(3).cursor, (1, 0));
        t.handle_key(key(KeyCode::Char('x')));
        assert_eq!(t.layout(3).lines, ["abc", "x\u{200b}d"]);

        let mut t = TextInput::single("abc\u{200b}");
        t.handle_key(key(KeyCode::Left));
        let layout = t.layout(3);
        assert_eq!(layout.cursor, (1, 0));
        assert_eq!(layout.lines.len(), 2);
    }

    // A long paste into the middle of long text goes in at once, in time
    // that grows with the text, not with its square.
    #[test]
    fn a_long_paste_into_long_text_is_quick() {
        let mut t = TextInput::multi(&"a".repeat(200_000));
        t.handle_key(key(KeyCode::Home));
        let started = std::time::Instant::now();
        t.insert_str(&"日本語👍🏽\n".repeat(40_000));
        assert!(
            started.elapsed() < std::time::Duration::from_secs(1),
            "{:?}",
            started.elapsed()
        );
        assert!(t.text().starts_with("日本語👍🏽\n日本語"));
        assert!(t.text().ends_with("aaa"));
    }
}
