//! The profile editor, as bsky's: `e` on your own Profile tab edits the
//! name, the description, and the avatar and banner (a path, or `ctrl+o`
//! to choose a picture), `tab` moves between them, `ctrl+s` saves with
//! `i/update`, `esc` closes without saving. A field left as it came is
//! not sent.

use std::path::PathBuf;

use super::write::browse_start;
use super::*;
use crate::i18n::n;
use crate::tui::files::{Action, Browser};
use crate::tui::input::TextInput;
use crate::tui::worker::ProfileChange;

/// The profile editor.
#[derive(Debug, Clone, PartialEq)]
pub struct EditProfile {
    /// The name, the description, a new avatar's path, a new banner's.
    pub fields: [TextInput; 4],
    pub focus: usize,
    pub loading: bool,
    pub saving: bool,
    /// The picture browser, open to choose the avatar or the banner.
    pub browser: Option<Browser>,
    /// The picture chosen in the browser for each, kept as the path
    /// itself, which may not be text that round-trips.
    pub chosen: [Option<PathBuf>; 2],
    /// The name and description as they came: a field still equal to its
    /// own is not sent.
    pub loaded: [String; 2],
}

impl EditProfile {
    pub const LABELS: [&'static str; 4] = [
        n!("Name"),
        n!("Description"),
        n!("New avatar (a path, or ctrl+o to choose; optional)"),
        n!("New banner (a path, or ctrl+o to choose; optional)"),
    ];

    /// The picture typed or chosen for field `i` (2 the avatar, 3 the
    /// banner), if any.
    fn picture(&self, i: usize) -> Option<PathBuf> {
        let typed = self.fields[i].text();
        let typed = typed.trim();
        if typed.is_empty() {
            return None;
        }
        match &self.chosen[i - 2] {
            Some(p) if p.file_name().is_some_and(|n| n.to_string_lossy() == typed) => {
                Some(p.clone())
            }
            _ => Some(crate::tui::files::expand_home(typed)),
        }
    }

    /// What saving changes: the fields not left as they came.
    pub fn change(&self) -> ProfileChange {
        let name = self.fields[0].text();
        let description = self.fields[1].text();
        ProfileChange {
            name: (name != self.loaded[0]).then_some(name),
            description: (description != self.loaded[1]).then_some(description),
            avatar: self.picture(2),
            banner: self.picture(3),
        }
    }
}

impl App {
    /// `e` on your own profile: the editor, which waits for the profile
    /// it starts from.
    pub(super) fn edit_profile(&mut self) -> Jobs {
        if !self.may("write:account", "edit your profile") {
            return Vec::new();
        }
        self.overlay = Some(Overlay::EditProfile(Box::new(EditProfile {
            fields: [
                TextInput::single(""),
                TextInput::multi(""),
                TextInput::single(""),
                TextInput::single(""),
            ],
            focus: 0,
            loading: true,
            saving: false,
            browser: None,
            chosen: [None, None],
            loaded: Default::default(),
        })));
        vec![self.stamp(Job::LoadProfileEditor)]
    }

    pub(super) fn edit_profile_key(&mut self, key: KeyEvent) -> Jobs {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let start = browse_start(&self.browse_from);
        let Some(Overlay::EditProfile(e)) = &mut self.overlay else {
            return Vec::new();
        };
        if let Some(b) = &mut e.browser {
            match b.key(key) {
                Action::None => {}
                Action::Close => {
                    self.browse_from = Some(b.dir.clone());
                    e.browser = None;
                }
                Action::Choose(paths) => {
                    self.browse_from = Some(b.dir.clone());
                    e.browser = None;
                    if let Some(p) = paths.into_iter().next() {
                        let at = if e.focus == 3 { 3 } else { 2 };
                        let name = p
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        e.fields[at] = TextInput::single(&name);
                        e.chosen[at - 2] = Some(p);
                        e.focus = at;
                    }
                }
            }
            return Vec::new();
        }
        if e.saving {
            return Vec::new();
        }
        match key.code {
            KeyCode::Esc => self.overlay = None,
            KeyCode::Tab => e.focus = (e.focus + 1) % 4,
            KeyCode::BackTab => e.focus = (e.focus + 3) % 4,
            KeyCode::Char('o') if ctrl => {
                // The picture field with focus, else the avatar's.
                if e.focus < 2 {
                    e.focus = 2;
                }
                e.browser = Some(Browser::open(&start, 1, false));
            }
            KeyCode::Char('s') if ctrl => {
                if e.loading {
                    return Vec::new();
                }
                let change = e.change();
                if change.is_empty() {
                    self.overlay = None;
                    self.info(n!("nothing changed"));
                    return Vec::new();
                }
                for p in change.avatar.iter().chain(&change.banner) {
                    if !crate::tui::files::is_picture_name(&p.to_string_lossy()) {
                        self.error(crate::i18n::tf(
                            "{} is not a picture (JPEG, PNG, GIF or WebP)",
                            &[&p.display().to_string()],
                        ));
                        return Vec::new();
                    }
                }
                e.saving = true;
                return vec![self.stamp(Job::SaveProfile(Box::new(change)))];
            }
            _ if e.loading => {}
            _ => {
                e.fields[e.focus].handle_key(key);
                // A path typed by hand is not the one chosen.
                if e.focus >= 2 {
                    e.chosen[e.focus - 2] = None;
                }
            }
        }
        Vec::new()
    }

    /// The profile the editor starts from, or the profile saved.
    pub(super) fn edit_profile_answer(&mut self, event: Event) -> Jobs {
        match event {
            Event::ProfileEditor(result) => {
                let Some(Overlay::EditProfile(e)) = &mut self.overlay else {
                    return Vec::new();
                };
                match result {
                    Ok(u) => {
                        let name = u.name.clone().unwrap_or_default();
                        let description = u.description.clone().unwrap_or_default();
                        e.fields[0] = TextInput::single(&name);
                        e.fields[1] = TextInput::multi(&description);
                        e.loaded = [e.fields[0].text(), e.fields[1].text()];
                        e.loading = false;
                    }
                    Err(err) => {
                        self.overlay = None;
                        self.error(said(&err));
                    }
                }
                Vec::new()
            }
            Event::ProfileSaved(result) => {
                let editing = matches!(self.overlay, Some(Overlay::EditProfile(_)));
                match result {
                    Ok(u) => {
                        if editing {
                            self.overlay = None;
                        }
                        if self.profile.own(self.my_user_id()) {
                            self.profile.user = Some(*u);
                        }
                        self.info(n!("profile saved"));
                    }
                    Err(err) => {
                        if let Some(Overlay::EditProfile(e)) = &mut self.overlay {
                            e.saving = false;
                        }
                        self.error(said(&err));
                    }
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }
}
