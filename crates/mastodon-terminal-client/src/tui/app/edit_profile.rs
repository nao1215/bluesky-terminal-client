//! Editing the authenticated account with raw source fields.
use super::*;
use crate::api::profile::ProfileChange;
use crate::api::types::ProfileField;
use serde_json::json;
use terminal_kit::files::{Action, Browser};

#[derive(Debug, Clone)]
pub(crate) struct EditProfile {
    pub fields: Vec<TextInput>,
    pub focus: usize,
    pub toggles: [bool; 3],
    pub browser: Option<Browser>,
    pub pictures: [Option<std::path::PathBuf>; 2],
    pub waiting: Option<u64>,
    pub loaded: Option<Account>,
}
impl EditProfile {
    pub(crate) const LABELS: [&'static str; 15] = [
        n!("Display name"),
        n!("Bio"),
        n!("Locked"),
        n!("Bot"),
        n!("Discoverable"),
        n!("Field 1 name"),
        n!("Field 1 value"),
        n!("Field 2 name"),
        n!("Field 2 value"),
        n!("Field 3 name"),
        n!("Field 3 value"),
        n!("Field 4 name"),
        n!("Field 4 value"),
        n!("Avatar (ctrl+o)"),
        n!("Header (ctrl+o)"),
    ];
    fn change(&self) -> Option<ProfileChange> {
        let a = self.loaded.as_ref()?;
        let source = a.source.as_ref()?;
        let mut attrs = json!({});
        for (key, value, before) in [
            ("display_name", self.fields[0].text(), &a.display_name),
            ("note", self.fields[1].text(), &source.note),
        ] {
            if &value != before {
                attrs[key] = json!(value);
            }
        }
        for (i, key) in ["locked", "bot", "discoverable"].iter().enumerate() {
            if self.toggles[i] != [a.locked, a.bot, a.discoverable.unwrap_or(false)][i] {
                attrs[key] = json!(self.toggles[i]);
            }
        }
        let fields: Vec<_> = (0..4)
            .map(|i| ProfileField {
                name: self.fields[5 + i * 2].text(),
                value: self.fields[6 + i * 2].text(),
            })
            .collect();
        let mut original = source.fields.clone();
        original.resize(4, ProfileField::default());
        if fields != original {
            let mut all = source.fields.clone();
            all.resize(all.len().max(4), ProfileField::default());
            all[..4].clone_from_slice(&fields);
            attrs["fields_attributes"] = serde_json::Value::Object(
                all.into_iter()
                    .enumerate()
                    .map(|(i, f)| (i.to_string(), json!(f)))
                    .collect(),
            );
        }
        Some(ProfileChange {
            attributes: attrs,
            pictures: self
                .pictures
                .iter()
                .enumerate()
                .filter_map(|(i, p)| p.clone().map(|p| (["avatar", "header"][i].into(), p)))
                .collect(),
        })
    }
}
impl App {
    pub(super) fn edit_profile(&mut self) -> Jobs {
        if self.tab != Tab::Profile
            || !self.threads.is_empty()
            || !self.profile.account.as_ref().is_some_and(|a| self.own(a))
        {
            return Vec::new();
        }
        let job = self.job(Job::ProfileEditor);
        self.overlay = Some(Overlay::EditProfile(Box::new(EditProfile {
            fields: (0..15)
                .map(|i| {
                    if i == 1 {
                        TextInput::multi("")
                    } else {
                        TextInput::single("")
                    }
                })
                .collect(),
            focus: 0,
            toggles: [false; 3],
            browser: None,
            pictures: [None, None],
            waiting: Some(job.0),
            loaded: None,
        })));
        vec![job]
    }
    pub(super) fn edit_profile_key(&mut self, mut e: Box<EditProfile>, key: KeyEvent) -> Jobs {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if let Some(browser) = &mut e.browser {
            match browser.key(key) {
                Action::None => {}
                Action::Close => {
                    self.browse_from = Some(browser.dir.clone());
                    e.browser = None;
                }
                Action::Choose(paths) => {
                    self.browse_from = Some(browser.dir.clone());
                    if let Some(path) = paths.into_iter().next() {
                        e.fields[e.focus] = TextInput::single(
                            &path.file_name().unwrap_or_default().to_string_lossy(),
                        );
                        e.pictures[e.focus - 13] = Some(path);
                    }
                    e.browser = None;
                }
            }
        } else if e.waiting.is_none() {
            match key.code {
                KeyCode::Esc => return Vec::new(),
                KeyCode::Tab => e.focus = (e.focus + 1) % 15,
                KeyCode::BackTab => e.focus = (e.focus + 14) % 15,
                KeyCode::Char('o') if ctrl => {
                    e.focus = e.focus.max(13);
                    let dir = self
                        .browse_from
                        .clone()
                        .or_else(|| std::env::current_dir().ok())
                        .unwrap_or_default();
                    e.browser = Some(Browser::open(&dir, 1, false));
                }
                KeyCode::Char('x') if ctrl && e.focus >= 13 => {
                    e.pictures[e.focus - 13] = None;
                    e.fields[e.focus] = TextInput::single("");
                }
                KeyCode::Char('s') if ctrl && e.loaded.is_some() => {
                    let Some(change) = e.change() else {
                        self.overlay = Some(Overlay::EditProfile(e));
                        return Vec::new();
                    };
                    if change.attributes.as_object().is_some_and(|m| m.is_empty())
                        && change.pictures.is_empty()
                    {
                        self.say(t("nothing changed"));
                        return Vec::new();
                    }
                    let job = self.job(Job::SaveProfile(Box::new(change)));
                    e.waiting = Some(job.0);
                    self.overlay = Some(Overlay::EditProfile(e));
                    return vec![job];
                }
                KeyCode::Char(' ') | KeyCode::Enter if (2..5).contains(&e.focus) => {
                    e.toggles[e.focus - 2] = !e.toggles[e.focus - 2]
                }
                _ if e.focus < 13 && !(2..5).contains(&e.focus) => {
                    e.fields[e.focus].handle_key(key);
                }
                _ => {}
            }
        } else if e.loaded.is_none() && key.code == KeyCode::Esc {
            return Vec::new();
        }
        self.overlay = Some(Overlay::EditProfile(e));
        Vec::new()
    }
    pub(super) fn profile_editor_answer(&mut self, seq: u64, result: Result<Account>, saved: bool) {
        let Some(Overlay::EditProfile(e)) = &mut self.overlay else {
            return;
        };
        if e.waiting != Some(seq) {
            return;
        }
        e.waiting = None;
        match result {
            Ok(account) if saved => {
                self.profile.account = Some(account);
                self.overlay = None;
                self.say(t("profile saved"));
            }
            Ok(account) => {
                let Some(source) = &account.source else {
                    self.overlay = None;
                    self.fail(t("the server did not return editable profile fields"));
                    return;
                };
                e.fields[0] = TextInput::single(&account.display_name);
                e.fields[1] = TextInput::multi(&source.note);
                e.toggles = [
                    account.locked,
                    account.bot,
                    account.discoverable.unwrap_or(false),
                ];
                for (i, field) in source.fields.iter().take(4).enumerate() {
                    e.fields[5 + i * 2] = TextInput::single(&field.name);
                    e.fields[6 + i * 2] = TextInput::single(&field.value);
                }
                e.loaded = Some(account);
            }
            Err(err) => {
                if !saved {
                    self.overlay = None;
                }
                self.fail(err.message().to_owned());
            }
        }
    }
}
