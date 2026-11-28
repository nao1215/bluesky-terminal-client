//! Profile fields, preserving unknown server metadata through a merge patch.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::{Value, json};
use terminal_kit::input::TextInput;
pub(super) const FIELDS: [(&str, &str, &str); 7] = [
    ("name", "Name", "名前"),
    ("display_name", "Display name", "表示名"),
    ("about", "About", "自己紹介"),
    ("picture", "Picture URL", "アイコン URL"),
    ("banner", "Banner URL", "バナー URL"),
    ("website", "Website", "ウェブサイト"),
    ("nip05", "NIP-05", "NIP-05"),
];
pub(super) struct Editor {
    pub fields: Vec<TextInput>,
    pub selected: usize,
}
pub(super) enum Action {
    None,
    Close,
    Submit(Value),
    Attach(&'static str),
    Remove(&'static str),
}
impl Editor {
    pub(super) fn new(profile: &str) -> Self {
        let value: Value = serde_json::from_str(profile).unwrap_or_default();
        Self {
            fields: FIELDS
                .iter()
                .map(|(key, _, _)| TextInput::multi(value[key].as_str().unwrap_or("")))
                .collect(),
            selected: 0,
        }
    }
    pub(super) fn key(&mut self, key: KeyEvent) -> Action {
        if key.code == KeyCode::Esc {
            return Action::Close;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('s') | KeyCode::Enter => {
                    let mut patch = json!({});
                    for (i, (key, _, _)) in FIELDS.iter().enumerate() {
                        patch[key] = json!(self.fields[i].text());
                    }
                    return Action::Submit(patch);
                }
                KeyCode::Char('o') if matches!(FIELDS[self.selected].0, "picture" | "banner") => {
                    return Action::Attach(FIELDS[self.selected].0);
                }
                KeyCode::Char('x') if matches!(FIELDS[self.selected].0, "picture" | "banner") => {
                    self.fields[self.selected] = TextInput::multi("");
                    return Action::Remove(FIELDS[self.selected].0);
                }
                _ => (),
            }
        }
        match key.code {
            KeyCode::Tab => self.selected = (self.selected + 1) % FIELDS.len(),
            KeyCode::BackTab => self.selected = (self.selected + FIELDS.len() - 1) % FIELDS.len(),
            _ => {
                self.fields[self.selected].handle_key(key);
            }
        }
        Action::None
    }
    pub(super) fn summary(&self, ja: bool) -> String {
        FIELDS
            .iter()
            .enumerate()
            .map(|(i, (_, en, jp))| {
                format!(
                    "{} {}: {}",
                    if i == self.selected { "›" } else { " " },
                    if ja { jp } else { en },
                    self.fields[i].text()
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn field_navigation_submission_and_attachment_removal() {
        let mut e = Editor::new(
            r#"{"name":"Alice","about":"line1\nline2","picture":"https://example.invalid/avatar"}"#,
        );
        let Action::Submit(v) = e.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))
        else {
            panic!()
        };
        assert_eq!(v["about"], "line1\nline2");
        e.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(e.selected, FIELDS.len() - 1);
        e.selected = 3;
        assert!(matches!(
            e.key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL)),
            Action::Attach("picture")
        ));
        assert!(matches!(
            e.key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)),
            Action::Remove("picture")
        ));
        let Action::Submit(v) = e.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))
        else {
            panic!()
        };
        assert_eq!(v["picture"], "");
        assert_eq!(v["name"], "Alice");
    }
}
