//! Additional screens and confirmed operations, kept separate from basic navigation.
use super::{
    app::{App, List, Overlay, Tab},
    keys::ACTIONS,
    worker::{Job, Target},
};
use crate::{
    api::{
        Client, actions,
        event::{Event, Unsigned},
        relay::Outcome,
    },
    error::Result,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};
use terminal_kit::{files::Browser, input::TextInput};

#[derive(Clone)]
pub(super) enum Operation {
    RemoveColumn(usize),
    DmRelays,
    AddAccount(String, Value),
    RemoveAccount(String),
    Delete(Event),
    Relation(u16, String),
    Report(Event, String, String),
    Retract(Event, u16),
    Reaction(Event, String),
    Profile(Value, Vec<(String, PathBuf)>),
    Post(Unsigned, Vec<(PathBuf, String)>),
    Chat(Option<String>, Option<String>),
    Download(Event),
}
impl Operation {
    pub(super) fn execute(self, client: &Client<'_>) -> Result<Value> {
        let relation = match &self {
            Self::Relation(kind, author) => Some((*kind, author.clone())),
            _ => None,
        };
        let draft = match self {
            Self::DmRelays => client.inbox_list()?,
            Self::RemoveColumn(_) => return Ok(json!({"local":true})),
            Self::AddAccount(name, value) => {
                crate::accounts::add(Some(&name), value)?;
                return Ok(json!({"account":name}));
            }
            Self::RemoveAccount(name) => {
                crate::accounts::remove(&name)?;
                return Ok(json!({"removedAccount":name}));
            }
            Self::Delete(e) => actions::deletion(&[e], &client.config.public, "")?,
            Self::Relation(kind, author) => {
                let old = client.latest(kind)?.value;
                let add = old.is_none_or(|e| !e.tag_values("p").any(|p| p == author));
                client.relation(kind, &author, add)?
            }
            Self::Report(e, reason, comment) => actions::report(&e, &reason, &comment)?,
            Self::Reaction(e, text) => actions::reaction(&e, &text),
            Self::Retract(e, kind) => match client.retract(&e, kind)? {
                Some(d) => d,
                None => return Ok(json!({"already":true})),
            },
            Self::Profile(mut patch, files) => {
                let mut tags = Vec::new();
                for (field, path) in files {
                    let tag = client.upload(&path, "")?;
                    patch[&field] = json!(tag[1].trim_start_matches("url "));
                    tags.push(tag);
                }
                let mut draft = client.update_profile(patch)?;
                draft.tags.extend(tags);
                draft
            }
            Self::Post(mut draft, attachments) => {
                for (path, alt) in attachments {
                    let tag = client.upload(&path, &alt)?;
                    draft.content.push('\n');
                    draft.content.push_str(tag[1].trim_start_matches("url "));
                    draft.tags.push(tag);
                }
                draft
            }
            Self::Chat(peer, text) => {
                let (mut messages, mut warnings) =
                    client.chat(peer.as_deref(), text.as_deref(), 100)?;
                if text.is_some() {
                    match client.chat(peer.as_deref(), None, 100) {
                        Ok((m, w)) => {
                            messages = m;
                            warnings.extend(w);
                        }
                        Err(e) => {
                            messages.clear();
                            warnings.push(format!("Message sent; refresh failed: {}", e.error));
                        }
                    }
                }
                return Ok(json!({"messages":messages,"warnings":warnings}));
            }
            Self::Download(e) => {
                let dir = crate::config::config_dir()?.join("downloads");
                return Ok(json!({"saved":crate::api::media::download(&e,&dir)?}));
            }
        };
        let out = client.publish(draft)?;
        let relation = relation.map(|(kind, author)| json!({"kind":kind,"author":author,"enabled":out.value.tag_values("p").any(|p|p==author)}));
        Ok(
            json!({"relation":relation,"id":out.value.id,"profile":(out.value.kind == 0).then_some(out.value.content),"acceptedRelays":out.answered,"warnings":out.warnings}),
        )
    }
}
pub(super) enum Dialog {
    Profile(super::profile::Editor),
    Input { purpose: Purpose, text: TextInput },
    Confirm { operation: Operation },
    Actions(usize),
    Accounts { names: Vec<String>, selected: usize },
}
#[derive(Clone, Copy)]
pub(super) enum Purpose {
    AddAccount,
    Search,
    ChatPeer,
    ChatMessage,
    Reaction,
    Report,
    Warning,
    Alt,
}
#[derive(Default)]
pub(super) struct Extra {
    pub last_notifications: Option<std::time::Instant>,
    pub last_chat: Option<std::time::Instant>,
    pub pending_dialog: Option<Dialog>,
    pub operation: Option<Operation>,
    pub dialog: Option<Dialog>,
    pub browser: Option<Browser>,
    pub browser_profile: Option<String>,
    pub profile_files: Vec<(String, PathBuf)>,
    pub author: Option<String>,
    pub relationship: Option<(String, bool, bool)>,
    pub relationship_seq: u64,
    pub peer: Option<String>,
    pub messages: Vec<Value>,
    pub chat_selected: usize,
    pub metadata: HashMap<String, String>,
    pub expanded: HashSet<String>,
    pub cw: Option<String>,
    pub quote: Option<Event>,
    pub attachments: Vec<(PathBuf, String)>,
    pub attachment: usize,
    pub references: HashMap<String, Event>,
    pub task: Option<u64>,
    pub switch: Option<String>,
    pub accounts: Vec<String>,
    pub replay: u32,
    pub seen: HashSet<String>,
    pub persist: bool,
    pub column_lists: Vec<List>,
    pub columns: Vec<String>,
    pub column: Option<usize>,
    pub cache: HashMap<String, Vec<Event>>,
    pub prefetch: HashMap<u64, String>,
    pub dwell: terminal_kit::dwell::Dwell,
}
impl App {
    pub(super) fn invalidate_threads(&mut self) {
        self.extra.cache.clear();
        for (seq, _) in self.extra.prefetch.drain() {
            for (_, list) in &mut self.threads {
                if list.loading == Some(seq) {
                    list.loading = None;
                    list.more = true;
                }
            }
        }
    }

    pub(super) fn hidden(&self, note: &Event) -> bool {
        let inner = actions::reposted(note);
        inner
            .as_ref()
            .unwrap_or(note)
            .tags
            .iter()
            .any(|t| t.first().is_some_and(|s| s == "content-warning"))
            && !self.extra.expanded.contains(&note.id)
    }
    pub(super) fn task(&mut self, operation: Operation) -> Vec<Job> {
        if self.extra.task.is_some() {
            return vec![];
        }
        let seq = self.seq();
        self.extra.task = Some(seq);
        self.extra.operation = Some(operation.clone());
        vec![Job::Task { seq, operation }]
    }
    pub(super) fn extra_key(&mut self, key: KeyEvent) -> Option<Vec<Job>> {
        if let Some(browser) = &mut self.extra.browser {
            match browser.key(key) {
                terminal_kit::files::Action::Choose(paths) => {
                    if let Some(field) = self.extra.browser_profile.take() {
                        if let Some(path) = paths.into_iter().next() {
                            self.extra.profile_files.retain(|(f, _)| *f != field);
                            self.extra.profile_files.push((field, path));
                        }
                    } else {
                        self.extra
                            .attachments
                            .extend(paths.into_iter().map(|p| (p, String::new())));
                    }
                    self.extra.browser = None;
                }
                terminal_kit::files::Action::Close => {
                    self.extra.browser = None;
                    self.extra.browser_profile = None;
                }
                terminal_kit::files::Action::None => (),
            }
            return Some(vec![]);
        }
        if let Some(dialog) = self.extra.dialog.take() {
            return Some(self.dialog_key(dialog, key));
        }
        if let Some(Overlay::Compose { sending: None, .. }) = &self.overlay {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                match key.code {
                    KeyCode::Char('o') => {
                        if self.extra.attachments.len() >= 8 {
                            self.message = self.t("At most eight attachments").into();
                            return Some(vec![]);
                        }
                        self.extra.browser = Some(Browser::open(
                            &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                            8 - self.extra.attachments.len().min(8),
                            true,
                        ));
                        return Some(vec![]);
                    }
                    KeyCode::Char('x') => {
                        if self.extra.attachment < self.extra.attachments.len() {
                            self.extra.attachments.remove(self.extra.attachment);
                        }
                        self.extra.attachment = self
                            .extra
                            .attachment
                            .min(self.extra.attachments.len().saturating_sub(1));
                        return Some(vec![]);
                    }
                    KeyCode::Char('w') => {
                        self.input(Purpose::Warning, self.extra.cw.clone().unwrap_or_default());
                        return Some(vec![]);
                    }
                    _ => (),
                }
            }
            if matches!(key.code, KeyCode::Tab | KeyCode::BackTab)
                && !self.extra.attachments.is_empty()
            {
                self.extra.attachment = if key.code == KeyCode::BackTab {
                    self.extra.attachments.len() - 1
                } else {
                    self.extra.attachment.min(self.extra.attachments.len() - 1)
                };
                self.input(
                    Purpose::Alt,
                    self.extra
                        .attachments
                        .get(self.extra.attachment)
                        .map(|(_, a)| a.clone())
                        .unwrap_or_default(),
                );
                return Some(vec![]);
            }
        }
        if self.overlay.is_some() {
            return None;
        }
        match key.code {
            KeyCode::Enter => {
                if self.tab == Tab::Chat {
                    if self.extra.peer.is_none() {
                        if let Some(peer) = self
                            .extra
                            .messages
                            .get(self.extra.chat_selected)
                            .and_then(|c| c["members"].as_array())
                            .and_then(|m| {
                                m.iter()
                                    .find(|p| p["id"] != self.public)
                                    .or_else(|| m.first())
                            })
                            .and_then(|m| m["id"].as_str())
                            .map(str::to_owned)
                        {
                            self.extra.peer = Some(peer);
                            return Some(self.load(true));
                        }
                    } else {
                        self.input(Purpose::ChatMessage, String::new());
                    }
                } else if let Some(note) = self.selected().cloned() {
                    self.extra.author = Some(note.pubkey);
                    self.threads.clear();
                    self.tab = Tab::Profile;
                    self.lists[2] = List::default();
                    return Some(self.load(true));
                }
            }
            KeyCode::Char('/') => self.input(Purpose::Search, self.search.query.clone()),
            KeyCode::Char('I') => {
                self.extra.dialog = Some(Dialog::Confirm {
                    operation: Operation::DmRelays,
                })
            }
            KeyCode::Char('m') => self.input(Purpose::ChatPeer, String::new()),
            KeyCode::Char('i') if self.tab == Tab::Chat => {
                self.input(Purpose::ChatMessage, String::new())
            }
            KeyCode::Char('e')
                if self.tab == Tab::Profile
                    && self.extra.author.as_ref().is_none_or(|a| a == &self.public) =>
            {
                self.extra.profile_files.clear();
                self.extra.dialog =
                    Some(Dialog::Profile(super::profile::Editor::new(&self.profile)));
            }
            KeyCode::Char('e') => self.input(Purpose::Reaction, "+".into()),
            KeyCode::Char('.') => self.extra.dialog = Some(Dialog::Actions(0)),
            KeyCode::Char('A') => {
                self.extra.dialog = Some(Dialog::Accounts {
                    names: self.extra.accounts.clone(),
                    selected: 0,
                })
            }
            KeyCode::Char('w') => {
                if let Some(note) = self.selected() {
                    let id = note.id.clone();
                    if !self.extra.expanded.remove(&id) {
                        self.extra.expanded.insert(id);
                    }
                }
            }
            KeyCode::Char('Q') => {
                let quote = self.selected().cloned();
                self.compose(None);
                self.extra.quote = quote;
            }
            KeyCode::Char('D') => {
                if let Some(note) = self.selected().filter(|n| n.pubkey == self.public).cloned() {
                    self.extra.dialog = Some(Dialog::Confirm {
                        operation: Operation::Delete(note),
                    });
                }
            }
            KeyCode::Char('f' | 'M' | 'B') => {
                if let Some(note) = self.selected().cloned() {
                    self.extra.dialog = Some(Dialog::Confirm {
                        operation: Operation::Relation(
                            if key.code == KeyCode::Char('f') {
                                3
                            } else {
                                10000
                            },
                            note.pubkey,
                        ),
                    });
                }
            }
            KeyCode::Char('!') => self.input(Purpose::Report, "other ".into()),
            KeyCode::Char('d') => {
                if let Some(note) = self.selected().cloned() {
                    return Some(self.task(Operation::Download(note)));
                }
            }
            KeyCode::Char('+') => {
                let label = match self.target() {
                    Target::Search(query) => format!("search:{query}"),
                    Target::Author(author) => format!("author:{author}"),
                    _ => match self.tab {
                        Tab::Home => "home".into(),
                        Tab::Notifications => "notifications".into(),
                        Tab::Profile => format!(
                            "author:{}",
                            self.extra.author.as_deref().unwrap_or(&self.public)
                        ),
                        Tab::Chat => return Some(vec![]),
                    },
                };
                let list = self.list().clone();
                self.extra.columns.push(label);
                self.extra.column_lists.push(list);
                self.extra.column = Some(self.extra.columns.len() - 1);
                self.tab = Tab::Home;
                self.extra.persist = true;
            }
            KeyCode::Char('H' | 'L') | KeyCode::Left | KeyCode::Right
                if !self.extra.columns.is_empty() =>
            {
                let len = self.extra.columns.len();
                let old = self.extra.column.unwrap_or(0);
                let next = if matches!(key.code, KeyCode::Char('H') | KeyCode::Left) {
                    (old + len - 1) % len
                } else {
                    (old + 1) % len
                };
                self.extra.column = Some(next);
                self.tab = Tab::Home;
                return Some(self.load(true));
            }
            KeyCode::Char('x') if self.extra.column.is_some() => {
                self.extra.dialog = Some(Dialog::Confirm {
                    operation: Operation::RemoveColumn(self.extra.column.unwrap_or(0)),
                });
            }
            KeyCode::Char('<' | '>') if self.extra.column.is_some() => {
                let i = self.extra.column.unwrap_or(0);
                let j = if key.code == KeyCode::Char('<') {
                    i.saturating_sub(1)
                } else {
                    (i + 1).min(self.extra.columns.len().saturating_sub(1))
                };
                self.extra.columns.swap(i, j);
                self.extra.column_lists.swap(i, j);
                self.extra.column = Some(j);
                self.extra.persist = true;
            }
            KeyCode::Char('j' | 'k') | KeyCode::Down | KeyCode::Up if self.tab == Tab::Chat => {
                self.extra.chat_selected = super::app::moved(
                    key.code,
                    self.extra.chat_selected,
                    self.extra.messages.len(),
                );
            }
            KeyCode::Esc if self.tab == Tab::Chat && self.extra.peer.is_some() => {
                self.extra.peer = None;
                self.extra.chat_selected = 0;
                return Some(self.load(true));
            }
            KeyCode::Esc
                if self.tab == Tab::Profile
                    && self.extra.author.is_some()
                    && self.threads.is_empty() =>
            {
                self.extra.author = None;
                self.lists[2] = List::default();
                self.tab = Tab::Home;
            }
            _ => return None,
        }
        Some(vec![])
    }
    fn input(&mut self, purpose: Purpose, text: String) {
        self.extra.dialog = Some(Dialog::Input {
            purpose,
            text: if matches!(purpose, Purpose::Search) {
                TextInput::single(&text)
            } else {
                TextInput::multi(&text)
            },
        });
    }
    fn dialog_key(&mut self, mut dialog: Dialog, key: KeyEvent) -> Vec<Job> {
        if key.code == KeyCode::Esc {
            return vec![];
        }
        if self.extra.task.is_some() {
            self.extra.dialog = Some(dialog);
            return vec![];
        }
        if let Dialog::Input {
            purpose: Purpose::Alt,
            text,
        } = &mut dialog
            && !self.extra.attachments.is_empty()
            && (matches!(key.code, KeyCode::Tab | KeyCode::BackTab)
                || (key.code == KeyCode::Char('x')
                    && key.modifiers.contains(KeyModifiers::CONTROL)))
        {
            self.extra.attachments[self.extra.attachment].1 = text.text();
            if key.code == KeyCode::Char('x') {
                self.extra.attachments.remove(self.extra.attachment);
                if self.extra.attachments.is_empty() {
                    return vec![];
                }
                self.extra.attachment = self.extra.attachment.min(self.extra.attachments.len() - 1);
            } else {
                let len = self.extra.attachments.len();
                self.extra.attachment = (self.extra.attachment
                    + if key.code == KeyCode::Tab { 1 } else { len - 1 })
                    % len;
            }
            *text = TextInput::multi(&self.extra.attachments[self.extra.attachment].1);
            self.extra.dialog = Some(dialog);
            return vec![];
        }
        let mut jobs = vec![];
        let mut keep = true;
        match &mut dialog {
            Dialog::Profile(editor) => match editor.key(key) {
                super::profile::Action::None => (),
                super::profile::Action::Close => keep = false,
                super::profile::Action::Submit(patch) => {
                    jobs = self.task(Operation::Profile(patch, self.extra.profile_files.clone()));
                    keep = false;
                }
                super::profile::Action::Attach(field) => {
                    self.extra.browser_profile = Some(field.into());
                    self.extra.browser = Some(Browser::open(
                        &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                        1,
                        false,
                    ));
                }
                super::profile::Action::Remove(field) => {
                    self.extra.profile_files.retain(|(f, _)| f != field)
                }
            },
            Dialog::Confirm { operation } => {
                if key.code == KeyCode::Char('y') {
                    jobs = self.task(operation.clone());
                    keep = false;
                } else if key.code == KeyCode::Char('n') {
                    keep = false;
                }
            }
            Dialog::Input { purpose, text } => {
                if (key.code == KeyCode::Enter
                    && (!matches!(purpose, Purpose::ChatMessage)
                        || key.modifiers.contains(KeyModifiers::CONTROL)))
                    || (matches!(purpose, Purpose::ChatMessage)
                        && key.code == KeyCode::Char('s')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    let value = text.text();
                    keep = false;
                    match purpose {
                        Purpose::AddAccount => {
                            match value
                                .split_once(' ')
                                .and_then(|(n, j)| serde_json::from_str(j).ok().map(|v| (n, v)))
                            {
                                Some((name, v)) => {
                                    jobs = self.task(Operation::AddAccount(name.into(), v))
                                }
                                None => {
                                    self.message =
                                        self.t("Enter name followed by configuration JSON").into();
                                    keep = true;
                                }
                            }
                        }
                        Purpose::Search => {
                            if value.trim().is_empty() {
                                keep = true;
                            } else {
                                jobs = self.start_search(value.trim().into());
                            }
                        }
                        Purpose::ChatPeer => match actions::public(value.trim()) {
                            Ok(p) => {
                                self.extra.peer = Some(p);
                                self.tab = Tab::Chat;
                                jobs = self.load(true);
                            }
                            Err(e) => {
                                self.message = e.error;
                                keep = true;
                            }
                        },
                        Purpose::ChatMessage => {
                            jobs = self.task(Operation::Chat(self.extra.peer.clone(), Some(value)))
                        }
                        Purpose::Reaction => {
                            if let Some(note) = self.selected().cloned() {
                                jobs = self.task(Operation::Reaction(note, value));
                            }
                        }
                        Purpose::Report => {
                            if let Some(note) = self.selected().cloned() {
                                let (reason, comment) =
                                    value.split_once(' ').unwrap_or((&value, ""));
                                self.extra.dialog = Some(Dialog::Confirm {
                                    operation: Operation::Report(
                                        note,
                                        reason.into(),
                                        comment.into(),
                                    ),
                                });
                            }
                        }
                        Purpose::Warning => self.extra.cw = (!value.is_empty()).then_some(value),
                        Purpose::Alt => {
                            if let Some((_, alt)) =
                                self.extra.attachments.get_mut(self.extra.attachment)
                            {
                                *alt = value;
                            }
                        }
                    }
                } else {
                    text.handle_key(key);
                }
            }
            Dialog::Actions(selected) => {
                *selected = super::app::moved(key.code, *selected, ACTIONS.len());
                if key.code == KeyCode::Enter {
                    let code = ACTIONS[*selected].0;
                    return self.key(KeyEvent::from(KeyCode::Char(code)));
                }
            }
            Dialog::Accounts { names, selected } => {
                *selected = super::app::moved(key.code, *selected, names.len());
                if key.code == KeyCode::Char('a') {
                    self.input(Purpose::AddAccount, String::new());
                    keep = false;
                }
                if key.code == KeyCode::Char('x')
                    && let Some(name) = names.get(*selected)
                {
                    self.extra.dialog = Some(Dialog::Confirm {
                        operation: Operation::RemoveAccount(name.clone()),
                    });
                    keep = false;
                }
                if key.code == KeyCode::Enter && !self.writing() {
                    self.extra.switch = names.get(*selected).cloned();
                    keep = false;
                }
            }
        }
        if !keep
            && !jobs.is_empty()
            && matches!(
                dialog,
                Dialog::Profile(_)
                    | Dialog::Input {
                        purpose: Purpose::ChatMessage | Purpose::AddAccount | Purpose::Reaction,
                        ..
                    }
            )
        {
            self.extra.pending_dialog = Some(dialog);
        } else if keep {
            self.extra.dialog = Some(dialog);
        }
        jobs
    }
    pub(super) fn extra_answer(&mut self, seq: u64, result: Result<Value>) {
        if self.extra.task != Some(seq) {
            return;
        }
        self.extra.task = None;
        match result {
            Ok(v) => {
                if v["acceptedRelays"].as_u64().is_some_and(|n| n > 0) {
                    self.invalidate_threads();
                }
                if let Some(m) = v["messages"].as_array() {
                    self.extra.messages = m.clone();
                    self.extra.chat_selected =
                        self.extra.chat_selected.min(m.len().saturating_sub(1));
                }
                if v["relation"].is_object() {
                    self.extra.relationship_seq = seq;
                }
                if let Some((author, follow, mute)) = &mut self.extra.relationship
                    && v["relation"]["author"] == *author
                    && let Some(enabled) = v["relation"]["enabled"].as_bool()
                {
                    if v["relation"]["kind"] == 3 {
                        *follow = enabled;
                    } else {
                        *mute = enabled;
                    }
                }
                if let Some(profile) = v["profile"].as_str() {
                    self.profile = profile.into();
                    self.extra
                        .metadata
                        .insert(self.public.clone(), profile.into());
                }
                if let Some(operation) = self.extra.operation.take() {
                    match operation {
                        Operation::Reaction(e, _) => {
                            self.marks.insert((e.id, 7));
                        }
                        Operation::Retract(e, kind) => {
                            self.marks.remove(&(e.id, kind));
                        }
                        Operation::Delete(e) => {
                            for list in &mut self.lists {
                                list.notes.retain(|n| n.id != e.id);
                                list.selected =
                                    list.selected.min(list.notes.len().saturating_sub(1));
                            }
                        }
                        Operation::RemoveColumn(i) => {
                            if i < self.extra.columns.len() {
                                self.extra.columns.remove(i);
                                self.extra.column_lists.remove(i);
                                self.extra.column = None;
                                self.extra.persist = true;
                            }
                        }
                        Operation::AddAccount(..) | Operation::RemoveAccount(..) => {
                            match crate::accounts::list() {
                                Ok(names) => self.extra.accounts = names,
                                Err(e) => self.message = e.error,
                            }
                        }
                        _ => (),
                    }
                }
                self.extra.pending_dialog = None;
                self.overlay = None;
                self.message = v["warnings"]
                    .as_array()
                    .filter(|w| !w.is_empty())
                    .map(|w| {
                        w.iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join("; ")
                    })
                    .unwrap_or_else(|| self.t("Completed").into());
            }
            Err(e) => {
                self.extra.operation = None;
                self.extra.dialog = self.extra.pending_dialog.take();
                self.message = e.error;
                if let Some(Overlay::Compose { sending, .. }) = &mut self.overlay {
                    *sending = None;
                }
            }
        }
    }
    pub(super) fn writing(&self) -> bool {
        self.extra.task.is_some()
            || !self.pending.is_empty()
            || matches!(
                self.overlay,
                Some(Overlay::Compose {
                    sending: Some(_),
                    ..
                })
            )
    }
    pub(super) fn background(&mut self) -> Vec<Job> {
        let now = std::time::Instant::now();
        let mut jobs = Vec::new();
        if self
            .extra
            .last_notifications
            .is_none_or(|t| now.duration_since(t).as_secs() >= 30)
            && self.lists[1].loading.is_none()
        {
            self.extra.last_notifications = Some(now);
            let seq = self.seq();
            self.lists[1].loading = Some(seq);
            jobs.push(Job::Load {
                seq,
                target: Target::Tab(Tab::Notifications),
                count: 20,
                until: None,
            });
        }
        if self.tab == Tab::Chat
            && self.extra.dialog.is_none()
            && !self.writing()
            && self
                .extra
                .last_chat
                .is_none_or(|t| now.duration_since(t).as_secs() >= 15)
        {
            self.extra.last_chat = Some(now);
            jobs.extend(self.task(Operation::Chat(self.extra.peer.clone(), None)));
        }
        jobs
    }
    pub(super) fn prefetch(&mut self) -> Vec<Job> {
        let note = self.selected().cloned();
        let id = note.as_ref().map(|n| n.id.as_str());
        if self.overlay.is_none()
            && self.extra.dialog.is_none()
            && self.extra.dwell.ready(
                id,
                std::time::Instant::now(),
                std::time::Duration::from_millis(400),
            )
            && let Some(note) = note
            && !self.extra.cache.contains_key(&note.id)
            && !self.extra.prefetch.values().any(|id| id == &note.id)
        {
            let seq = self.seq();
            self.extra.prefetch.insert(seq, note.id.clone());
            return vec![Job::Prefetch { seq, note }];
        }
        vec![]
    }
    pub(super) fn prefetched(&mut self, seq: u64, result: Result<Outcome<Vec<Event>>>) {
        let Some(id) = self.extra.prefetch.remove(&seq) else {
            return;
        };
        if let Ok(out) = &result {
            if self.extra.cache.len() >= 32 {
                self.extra.cache.clear();
            }
            self.extra.cache.insert(id, out.value.clone());
        }
        if self.threads.iter().any(|(_, l)| l.loading == Some(seq)) {
            self.answer(super::worker::Answer::Loaded(
                seq,
                result.map(|out| Outcome {
                    value: super::worker::Page {
                        notes: out.value,
                        profile: None,
                        more: false,
                        append: false,
                        until: None,
                        boundary: 0,
                    },
                    warnings: out.warnings,
                    answered: out.answered,
                }),
            ));
        }
    }
    pub(super) fn target(&self) -> Target {
        if let Some((e, _)) = self.threads.last() {
            return Target::Thread(e.clone());
        }
        if self.tab == Tab::Home {
            if let Some(i) = self.extra.column
                && let Some(column) = self.extra.columns.get(i)
            {
                if column == "home" {
                    return Target::Tab(Tab::Home);
                }
                if column == "notifications" {
                    return Target::Tab(Tab::Notifications);
                }
                if let Some(author) = column.strip_prefix("author:") {
                    return Target::Author(author.into());
                }
                return Target::Search(column.strip_prefix("search:").unwrap_or(column).into());
            }
            if self.searching() {
                return Target::Search(self.search.query.clone());
            }
        }
        if self.tab == Tab::Profile
            && let Some(author) = &self.extra.author
        {
            return Target::Author(author.clone());
        }
        Target::Tab(self.tab)
    }
}
