//! Keys and worker answers change state here; no network or terminal I/O.
use super::{
    note,
    settings::Settings,
    worker::{Answer, Job},
};
use crate::api::{event::Event, nip19::Identifier};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::{HashMap, HashSet};
use terminal_kit::{
    input::TextInput,
    theme::{self, ColorDepth, Theme},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tab {
    Home,
    Notifications,
    Profile,
    Chat,
}
impl Tab {
    const ALL: [Self; 4] = [Self::Home, Self::Notifications, Self::Profile, Self::Chat];
    pub(super) fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

#[derive(Clone, Default)]
pub(super) struct List {
    pub notes: Vec<Event>,
    pub selected: usize,
    pub loading: Option<u64>,
    pub count: usize,
    pub more: bool,
    pub loaded: bool,
    pub until: Option<u64>,
    pub boundary: usize,
}

pub(super) enum Overlay {
    Help(usize),
    Compose {
        text: TextInput,
        target: Option<Event>,
        sending: Option<u64>,
    },
    Settings(usize),
    Themes {
        selected: usize,
        original: usize,
    },
    Languages(usize),
    Pictures {
        urls: Vec<String>,
        selected: usize,
    },
}

pub(super) struct App {
    pub extra: super::extras::Extra,
    pub search: super::search::Search,
    pub tab: Tab,
    pub lists: [List; 4],
    pub threads: Vec<(Event, List)>,
    pub overlay: Option<Overlay>,
    pub quit: bool,
    pub public: String,
    pub profile: String,
    pub message: String,
    pub answered: usize,
    pub relays: usize,
    pub marks: HashSet<(String, u16)>,
    pub(super) pending: HashMap<u64, (String, u16)>,
    sequence: u64,
    pub settings: Settings,
    pub writable_settings: bool,
    pub save: bool,
    pub language: &'static str,
    pub depth: ColorDepth,
    pub theme_index: usize,
    pub theme: Theme,
    settings_return: bool,
    pub copy: Option<String>,
}

impl App {
    pub(super) fn new(
        public: String,
        relays: usize,
        settings: Settings,
        depth: ColorDepth,
    ) -> Self {
        let (theme_index, theme) = theme::chosen(settings.theme.as_deref(), depth);
        let language = settings.language();
        Self {
            extra: super::extras::Extra::default(),
            search: super::search::Search::default(),
            tab: Tab::Home,
            lists: std::array::from_fn(|_| List::default()),
            threads: Vec::new(),
            overlay: None,
            quit: false,
            public,
            profile: String::new(),
            message: String::new(),
            answered: 0,
            relays,
            marks: HashSet::new(),
            pending: HashMap::new(),
            sequence: 0,
            settings,
            writable_settings: true,
            save: false,
            language,
            depth,
            theme_index,
            theme,
            settings_return: false,
            copy: None,
        }
    }
    pub(super) fn t<'a>(&self, en: &'a str) -> &'a str {
        crate::i18n::CATALOG.t(self.lang(), en)
    }
    pub(super) fn tf(&self, template: &str, args: &[&str]) -> String {
        crate::i18n::CATALOG.tf(self.lang(), template, args)
    }
    fn lang(&self) -> terminal_kit::i18n::Lang {
        if self.language == "ja" {
            terminal_kit::i18n::Lang::Ja
        } else {
            terminal_kit::i18n::Lang::En
        }
    }
    pub(super) fn list(&self) -> &List {
        if self.searching() && self.threads.is_empty() {
            return &self.search.list;
        }
        if self.threads.is_empty()
            && self.tab == Tab::Home
            && let Some(i) = self.extra.column
            && let Some(list) = self.extra.column_lists.get(i)
        {
            return list;
        }
        self.threads
            .last()
            .map(|(_, list)| list)
            .unwrap_or(&self.lists[self.tab.index()])
    }
    fn list_mut(&mut self) -> &mut List {
        if self.searching() && self.threads.is_empty() {
            return &mut self.search.list;
        }
        if self.threads.is_empty()
            && self.tab == Tab::Home
            && let Some(i) = self.extra.column
            && i < self.extra.column_lists.len()
        {
            return &mut self.extra.column_lists[i];
        }
        self.threads
            .last_mut()
            .map(|(_, list)| list)
            .unwrap_or(&mut self.lists[self.tab.index()])
    }
    pub(super) fn selected(&self) -> Option<&Event> {
        self.list().notes.get(self.list().selected)
    }
    pub(super) fn seq(&mut self) -> u64 {
        self.sequence += 1;
        self.sequence
    }

    pub(super) fn load(&mut self, refresh: bool) -> Vec<Job> {
        if self.tab == Tab::Chat {
            return self.task(super::extras::Operation::Chat(
                self.extra.peer.clone(),
                None,
            ));
        }
        if self.list().loading.is_some() {
            return vec![];
        }
        let seq = self.seq();
        let target = self.target();
        let list = self.list_mut();
        list.count = 20 + if refresh { 0 } else { list.boundary };
        let until = if refresh { None } else { list.until };
        list.loading = Some(seq);
        vec![Job::Load {
            seq,
            target,
            count: list.count,
            until,
        }]
    }

    pub(super) fn paste(&mut self, text: &str) {
        if self.extra.task.is_some() || self.extra.browser.is_some() {
            return;
        }
        if let Some(dialog) = &mut self.extra.dialog {
            match dialog {
                super::extras::Dialog::Input { text: input, .. } => input.insert_str(text),
                super::extras::Dialog::Profile(editor) => {
                    editor.fields[editor.selected].insert_str(text)
                }
                _ => (),
            }
            return;
        }
        if let Some(Overlay::Compose {
            text: input,
            sending: None,
            ..
        }) = &mut self.overlay
        {
            input.insert_str(text);
        }
    }

    pub(super) fn key(&mut self, key: KeyEvent) -> Vec<Job> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return vec![];
        }
        if let Some(jobs) = self.extra_key(key) {
            return jobs;
        }
        if self.overlay.is_some() {
            return self.overlay_key(key);
        }
        let code = key.code;
        match code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.overlay = Some(Overlay::Help(0)),
            KeyCode::Char('s') => self.overlay = Some(Overlay::Settings(0)),
            KeyCode::Char('T') => {
                self.settings_return = false;
                self.themes();
            }
            KeyCode::Char('n') => self.compose(None),
            KeyCode::Char('r') => {
                if let Some(note) = self.selected().cloned() {
                    self.compose(Some(note));
                }
            }
            KeyCode::Char('l') | KeyCode::Char('b') => {
                return self.react(if code == KeyCode::Char('l') { 7 } else { 6 });
            }
            KeyCode::Char('c') => self.copy_note(),
            KeyCode::Char('v' | 'V') => {
                let note = self
                    .selected()
                    .and_then(|n| {
                        if code == KeyCode::Char('V') {
                            n.tag_values("q")
                                .next()
                                .and_then(|id| self.extra.references.get(id))
                        } else {
                            Some(n)
                        }
                    })
                    .cloned();
                if let Some(note) = note {
                    return self.open_thread(note);
                }
            }
            KeyCode::Char(' ') => {
                if let Some(note) = self.selected() {
                    let urls = if self.hidden(note) {
                        vec![]
                    } else {
                        note::attachments(note)
                    };
                    if !urls.is_empty() {
                        self.overlay = Some(Overlay::Pictures { urls, selected: 0 });
                    }
                }
            }
            KeyCode::Esc => {
                if self.threads.pop().is_none() {
                    self.leave_search();
                }
            }
            KeyCode::Char('R') | KeyCode::F(5) => return self.load(true),
            KeyCode::Char('1'..='4') | KeyCode::Tab | KeyCode::BackTab => {
                let index = match code {
                    KeyCode::Char(c) => c as usize - '1' as usize,
                    KeyCode::BackTab => (self.tab.index() + 3) % 4,
                    _ => (self.tab.index() + 1) % 4,
                };
                self.leave_search();
                self.extra.column = None;
                self.tab = Tab::ALL[index];
                self.threads.clear();
                if self.tab == Tab::Notifications {
                    self.extra
                        .seen
                        .extend(self.lists[1].notes.iter().map(|n| n.id.clone()));
                    self.extra.persist = true;
                }
                if !self.list().loaded {
                    return self.load(true);
                }
            }
            _ => {
                let list = self.list_mut();
                list.selected = moved(code, list.selected, list.notes.len());
                if matches!(
                    code,
                    KeyCode::Char('j' | 'G') | KeyCode::Down | KeyCode::End | KeyCode::PageDown
                ) && list.more
                    && list.selected.saturating_add(3) >= list.notes.len()
                {
                    return self.load(false);
                }
            }
        }
        vec![]
    }

    pub(super) fn compose(&mut self, target: Option<Event>) {
        self.extra.cw = None;
        self.extra.quote = None;
        self.extra.attachments.clear();
        self.extra.attachment = 0;
        self.overlay = Some(Overlay::Compose {
            text: TextInput::multi(""),
            target,
            sending: None,
        });
    }
    fn themes(&mut self) {
        self.overlay = Some(Overlay::Themes {
            selected: self.theme_index,
            original: self.theme_index,
        });
    }
    fn set_theme(&mut self, index: usize) {
        self.theme_index = index;
        self.theme = theme::THEMES[index].for_depth(self.depth);
    }
    fn close_picker(&mut self) {
        self.overlay = self.settings_return.then_some(Overlay::Settings(0));
        self.settings_return = false;
    }
    fn save_settings(&mut self) {
        if self.writable_settings {
            self.save = true;
        } else {
            self.message = self
                .t("Settings file could not be read; changes are not saved")
                .into();
        }
    }

    fn overlay_key(&mut self, key: KeyEvent) -> Vec<Job> {
        let Some(mut overlay) = self.overlay.take() else {
            return vec![];
        };
        let mut keep = true;
        let mut jobs = vec![];
        match &mut overlay {
            Overlay::Compose {
                text,
                target,
                sending,
            } => {
                if sending.is_none() {
                    if key.code == KeyCode::Esc {
                        keep = false;
                    } else if key.code == KeyCode::Char('s')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        if !text.text().trim().is_empty() || !self.extra.attachments.is_empty() {
                            let seq = self.seq();
                            let event = crate::api::actions::post(
                                text.text(),
                                target.as_ref(),
                                self.extra.quote.as_ref(),
                                self.extra.cw.as_deref(),
                                "",
                            );
                            if self.extra.attachments.is_empty() {
                                jobs.push(Job::Write { seq, event });
                            } else {
                                self.extra.task = Some(seq);
                                jobs.push(Job::Task {
                                    seq,
                                    operation: super::extras::Operation::Post(
                                        event,
                                        self.extra.attachments.clone(),
                                    ),
                                });
                            }
                            *sending = Some(seq);
                            self.message = self.t("Sending…").into();
                        }
                    } else {
                        text.handle_key(key);
                    }
                }
            }
            Overlay::Help(scroll) => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Char('q' | '?')) {
                    keep = false;
                } else {
                    *scroll = moved(key.code, *scroll, super::view::HELP.len());
                }
            }
            Overlay::Settings(selected) => {
                *selected = moved(key.code, *selected, 2);
                match key.code {
                    KeyCode::Esc => keep = false,
                    KeyCode::Enter => {
                        self.settings_return = true;
                        if *selected == 0 {
                            self.themes();
                        } else {
                            self.overlay =
                                Some(Overlay::Languages(usize::from(self.language == "ja")));
                        }
                        keep = false;
                    }
                    KeyCode::Char('x') => {
                        if *selected == 0 {
                            self.settings.theme = None;
                            self.set_theme(theme::chosen(None, self.depth).0);
                        } else {
                            self.settings.language = None;
                            self.language = self.settings.language();
                        }
                        self.save_settings();
                    }
                    _ => (),
                }
            }
            Overlay::Themes { selected, original } => {
                *selected = moved(key.code, *selected, theme::THEMES.len());
                self.set_theme(*selected);
                match key.code {
                    KeyCode::Enter => {
                        self.settings.theme = Some(theme::THEMES[*selected].name.into());
                        self.save_settings();
                        self.close_picker();
                        keep = false;
                    }
                    KeyCode::Esc => {
                        self.set_theme(*original);
                        self.close_picker();
                        keep = false;
                    }
                    _ => (),
                }
            }
            Overlay::Languages(selected) => {
                *selected = moved(key.code, *selected, 2);
                match key.code {
                    KeyCode::Enter => {
                        self.language = ["en", "ja"][*selected];
                        self.settings.language = Some(self.language.into());
                        self.save_settings();
                        self.close_picker();
                        keep = false;
                    }
                    KeyCode::Esc => {
                        self.close_picker();
                        keep = false;
                    }
                    _ => (),
                }
            }
            Overlay::Pictures { urls, selected } => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => keep = false,
                KeyCode::Char('r') => self.extra.replay = self.extra.replay.wrapping_add(1),
                KeyCode::Char('d') => {
                    if let Some(note) = self.selected().cloned() {
                        jobs = self.task(super::extras::Operation::Download(note));
                    }
                }
                KeyCode::Char('h') | KeyCode::Left => *selected = selected.saturating_sub(1),
                KeyCode::Char('l') | KeyCode::Right => {
                    *selected = (*selected + 1).min(urls.len().saturating_sub(1))
                }
                _ => (),
            },
        }
        if keep {
            self.overlay = Some(overlay);
        }
        jobs
    }

    fn open_thread(&mut self, note: Event) -> Vec<Job> {
        self.threads.push((
            note.clone(),
            List {
                notes: vec![note.clone()],
                ..List::default()
            },
        ));
        if let Some(notes) = self.extra.cache.get(&note.id).cloned() {
            let list = self.list_mut();
            list.selected = notes.iter().position(|n| n.id == note.id).unwrap_or(0);
            list.notes = notes;
            list.loaded = true;
            return vec![];
        }
        if let Some(seq) = self
            .extra
            .prefetch
            .iter()
            .find(|(_, id)| **id == note.id)
            .map(|(seq, _)| *seq)
        {
            self.list_mut().loading = Some(seq);
            return vec![];
        }
        self.load(true)
    }

    fn react(&mut self, kind: u16) -> Vec<Job> {
        let Some(note) = self.selected().cloned() else {
            return vec![];
        };
        let mark = (note.id.clone(), kind);
        if !self.marks.insert(mark.clone()) {
            if self.pending.values().any(|p| p == &mark) {
                return vec![];
            }
            return self.task(super::extras::Operation::Retract(note, kind));
        }
        let content = if kind == 7 {
            "+".into()
        } else {
            match serde_json::to_string(&note) {
                Ok(value) => value,
                Err(e) => {
                    self.marks.remove(&mark);
                    self.message = e.to_string();
                    return vec![];
                }
            }
        };
        let seq = self.seq();
        self.pending.insert(seq, mark);
        self.message = self.t("Sending…").into();
        vec![Job::Write {
            seq,
            event: note::draft(kind, content, Some(&note)),
        }]
    }

    fn copy_note(&mut self) {
        let Some(note) = self.selected() else {
            return;
        };
        let identifier = (|| {
            Identifier::Event {
                id: crate::api::event::bytes(&note.id)?,
                relays: vec![],
                author: Some(crate::api::event::bytes(&note.pubkey)?),
                kind: Some(1),
            }
            .encode()
        })();
        match identifier {
            Ok(encoded) => {
                self.copy = Some(format!("nostr:{encoded}"));
                self.message = self.t("Copied note identifier").into();
            }
            Err(e) => self.message = e.error,
        }
    }

    pub(super) fn answer(&mut self, answer: Answer) {
        match answer {
            Answer::Task(seq, result) => self.extra_answer(seq, result),
            Answer::Prefetched(seq, result) => self.prefetched(seq, result),
            Answer::Relationship(seq, author, follow, mute) => {
                if seq >= self.extra.relationship_seq && self.extra.author.as_ref() == Some(&author)
                {
                    self.extra.relationship_seq = seq;
                    self.extra.relationship = Some((author, follow, mute));
                }
            }
            Answer::Referenced(events) => {
                for e in events {
                    self.extra.references.insert(e.id.clone(), e);
                }
                while self.extra.references.len() > 256 {
                    if let Some(id) = self.extra.references.keys().next().cloned() {
                        self.extra.references.remove(&id);
                    }
                }
            }
            Answer::Metadata(values) => self.extra.metadata.extend(values),
            Answer::Marks(seq, ids, events) => {
                if seq == self.sequence && self.pending.is_empty() && self.extra.task.is_none() {
                    self.marks.retain(|(id, _)| !ids.contains(id));
                    for e in events {
                        if let Some(id) = e.tag_values("e").last() {
                            self.marks
                                .insert((id.into(), if e.kind == 16 { 6 } else { e.kind }));
                        }
                    }
                }
            }
            Answer::Authorize(url) => {
                self.message = format!("{}: {url}", self.t("Remote signer authorization"))
            }
            Answer::Loaded(seq, result) => {
                let reset_selection = self.search.list.loading == Some(seq)
                    && std::mem::take(&mut self.search.reset_selection);
                let notifications = self.lists[1].loading == Some(seq);
                let focal = self
                    .threads
                    .iter()
                    .find(|(_, l)| l.loading == Some(seq))
                    .map(|(n, _)| n.id.clone());
                let list = self
                    .lists
                    .iter_mut()
                    .chain(self.threads.iter_mut().map(|(_, l)| l))
                    .chain(self.extra.column_lists.iter_mut())
                    .chain(std::iter::once(&mut self.search.list))
                    .find(|l| l.loading == Some(seq));
                let Some(list) = list else {
                    return;
                };
                list.loading = None;
                match result {
                    Ok(out) => {
                        let selected = if reset_selection {
                            list.selected = 0;
                            None
                        } else {
                            list.notes
                                .get(list.selected)
                                .map(|n| n.id.clone())
                                .or(focal)
                        };
                        if out.value.append {
                            let mut ids: HashSet<_> =
                                list.notes.iter().map(|e| e.id.clone()).collect();
                            list.notes.extend(
                                out.value
                                    .notes
                                    .into_iter()
                                    .filter(|e| ids.insert(e.id.clone())),
                            );
                        } else {
                            list.notes = out.value.notes;
                        }
                        list.until = out.value.until;
                        list.boundary = out.value.boundary;
                        list.selected = selected
                            .and_then(|id| list.notes.iter().position(|n| n.id == id))
                            .unwrap_or(list.selected)
                            .min(list.notes.len().saturating_sub(1));
                        list.loaded = true;
                        if notifications && self.tab == Tab::Notifications {
                            self.extra
                                .seen
                                .extend(list.notes.iter().map(|e| e.id.clone()));
                            self.extra.persist = true;
                        }
                        list.more = out.value.more;
                        if let Some(profile) = out.value.profile {
                            self.profile = profile;
                        }
                        self.answered = out.answered;
                        if !out.warnings.is_empty() {
                            self.message = out.warnings.join("; ");
                        }
                    }
                    Err(e) => {
                        self.answered = 0;
                        list.more = false;
                        self.message = e.error;
                    }
                }
            }
            Answer::Written(seq, result) => {
                let mark = self.pending.remove(&seq);
                let composing = matches!(&self.overlay, Some(Overlay::Compose { sending: Some(id), .. }) if *id == seq);
                // A successful transport with zero acceptances is still a failed write.
                let result = result.and_then(|out| {
                    if out.value == 0 {
                        Err(crate::error::Error::network("no relay accepted the event"))
                    } else {
                        Ok(out)
                    }
                });
                match result {
                    Ok(out) => {
                        self.invalidate_threads();
                        self.answered = out.answered;
                        if composing {
                            self.overlay = None;
                        }
                        self.message = if out.warnings.is_empty() {
                            self.t("Sent").into()
                        } else {
                            out.warnings.join("; ")
                        };
                    }
                    Err(e) => {
                        if let Some(mark) = mark {
                            self.marks.remove(&mark);
                        }
                        if composing
                            && let Some(Overlay::Compose { sending, .. }) = &mut self.overlay
                        {
                            *sending = None;
                        }
                        self.answered = 0;
                        self.message = e.error;
                    }
                }
            }
        }
    }
}

pub(super) fn moved(key: KeyCode, index: usize, len: usize) -> usize {
    match key {
        KeyCode::Char('j') | KeyCode::Down => index.saturating_add(1),
        KeyCode::Char('k') | KeyCode::Up => index.saturating_sub(1),
        KeyCode::Char('g') | KeyCode::Home => 0,
        KeyCode::Char('G') | KeyCode::End => len.saturating_sub(1),
        KeyCode::PageDown => index.saturating_add(5),
        KeyCode::PageUp => index.saturating_sub(5),
        _ => index,
    }
    .min(len.saturating_sub(1))
}

#[cfg(test)]
mod tests;
