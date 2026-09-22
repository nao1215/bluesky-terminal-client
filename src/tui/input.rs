//! A small text editor for the login form, the composer, and the profile editor.
//!
//! The buffer is a `Vec<char>` with a cursor index, which keeps editing and
//! cursor movement exact for multibyte text. Layout wraps by display width at
//! character boundaries, so CJK text (which has no spaces) wraps too.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_width::UnicodeWidthChar;

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
        let chars: Vec<char> = text.chars().filter(|c| *c != '\n').collect();
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
        let chars: Vec<char> = text.chars().collect();
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
        for c in s.chars() {
            if c == '\r' {
                continue;
            }
            if c == '\n' && !self.multiline {
                continue;
            }
            self.insert(c);
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
            KeyCode::Char(c) if !ctrl => self.insert(c),
            KeyCode::Char('a') if ctrl => self.cursor = self.line_start(),
            KeyCode::Char('e') if ctrl => self.cursor = self.line_end(),
            KeyCode::Char('u') if ctrl => {
                let start = self.line_start();
                self.chars.drain(start..self.cursor);
                self.cursor = start;
            }
            KeyCode::Enter if self.multiline => self.insert('\n'),
            KeyCode::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.chars.remove(self.cursor);
            }
            KeyCode::Backspace => {}
            KeyCode::Delete if self.cursor < self.chars.len() => {
                self.chars.remove(self.cursor);
            }
            KeyCode::Delete => {}
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(self.chars.len()),
            KeyCode::Home => self.cursor = self.line_start(),
            KeyCode::End => self.cursor = self.line_end(),
            KeyCode::Up if self.multiline => self.vertical(-1),
            KeyCode::Down if self.multiline => self.vertical(1),
            _ => return false,
        }
        true
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

    /// Move to the same column of the previous or next logical line.
    fn vertical(&mut self, dir: i32) {
        let col = self.cursor - self.line_start();
        if dir < 0 {
            let start = self.line_start();
            if start == 0 {
                self.cursor = 0;
                return;
            }
            self.cursor = start - 1;
            let prev_start = self.line_start();
            self.cursor = (prev_start + col).min(start - 1);
        } else {
            let end = self.line_end();
            if end == self.chars.len() {
                self.cursor = end;
                return;
            }
            self.cursor = end + 1;
            let next_end = self.line_end();
            self.cursor = (end + 1 + col).min(next_end);
        }
    }

    /// Lay the text out in `width` columns (at least 1).
    pub fn layout(&self, width: usize) -> Layout {
        let width = width.max(1);
        let mut lines = vec![String::new()];
        let mut col = 0usize;
        let mut cursor = (0, 0);
        for (i, &c) in self.chars.iter().enumerate() {
            if i == self.cursor {
                cursor = (lines.len() - 1, col);
            }
            if c == '\n' {
                lines.push(String::new());
                col = 0;
                continue;
            }
            let shown = if self.masked { '•' } else { c };
            let w = shown.width().unwrap_or(0);
            if col + w > width {
                lines.push(String::new());
                col = 0;
                if i == self.cursor {
                    cursor = (lines.len() - 1, 0);
                }
            }
            lines.last_mut().unwrap().push(shown);
            col += w;
        }
        if self.cursor == self.chars.len() {
            if col >= width {
                lines.push(String::new());
                col = 0;
            }
            cursor = (lines.len() - 1, col);
        }
        Layout { lines, cursor }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn paste_drops_newlines_in_single_line_fields() {
        let mut t = TextInput::single("");
        t.insert_str("abc\r\ndef");
        assert_eq!(t.text(), "abcdef");
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

    #[test]
    fn ctrl_u_clears_to_line_start() {
        let mut t = TextInput::multi("keep\ndrop me");
        t.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert_eq!(t.text(), "keep\n");
    }
}
