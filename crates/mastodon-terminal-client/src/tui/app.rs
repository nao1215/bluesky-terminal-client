//! The state of the full-screen client, as bsky's and msky's: keys and
//! answers come in, jobs for the worker go out, and [`super::view`] draws
//! what is here. Nothing here touches the terminal or the network, so the
//! tests drive it with keys and made-up answers.
//!
//! Every page asked for is numbered; an answer under a number the list no
//! longer waits for (the list was refreshed meanwhile) is dropped, so a
//! slow old page never lands on top of a new one.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use terminal_kit::input::TextInput;

use crate::api::types::{Account, Limits, Media, Notification, Status};
mod browse;
use crate::cli::{TimelineKind, Visibility};
use crate::config::{Session, Settings};
use crate::error::Result;
use crate::i18n::{n, t, tf};
use crate::tui::keys;
use crate::tui::list::{List, Row, ThreadView};
use crate::tui::theme::{self, ColorDepth, THEMES, Theme};
use crate::tui::worker::{Draft, Event, Job, next_of};
pub(crate) use browse::{Profile, Search};

/// The jobs a key or an answer starts, each with its number.
pub(crate) type Jobs = Vec<(u64, Job)>;

/// How long a message stays on the status row.
const MESSAGE_FOR: Duration = Duration::from_secs(4);
/// How long an error stays over the screen when no key closes it.
const ERROR_FOR: Duration = Duration::from_secs(8);
/// How far pgdn and pgup move.
const PAGE_STEP: usize = 5;

/// A tab, in the order the tab row shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Home,
    Local,
    Federated,
    Notifications,
    Search,
    Profile,
}

impl Tab {
    pub(crate) const ALL: [Tab; 6] = [
        Tab::Home,
        Tab::Local,
        Tab::Federated,
        Tab::Notifications,
        Tab::Search,
        Tab::Profile,
    ];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Tab::Home => n!("Home"),
            Tab::Local => n!("Local"),
            Tab::Federated => n!("Federated"),
            Tab::Notifications => n!("Notifications"),
            Tab::Search => n!("Search"),
            Tab::Profile => n!("Profile"),
        }
    }

    /// The timeline it shows, for the three that show one.
    pub(crate) fn kind(self) -> Option<TimelineKind> {
        match self {
            Tab::Home => Some(TimelineKind::Home),
            Tab::Local => Some(TimelineKind::Local),
            Tab::Federated => Some(TimelineKind::Public),
            Tab::Notifications | Tab::Search | Tab::Profile => None,
        }
    }

    /// Whether the server shows it to no visitor.
    pub(crate) fn needs_login(self) -> bool {
        matches!(self, Tab::Home | Tab::Notifications)
    }

    fn index(self) -> usize {
        Tab::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

/// Where a timeline's list is kept.
fn timeline_index(kind: TimelineKind) -> usize {
    match kind {
        TimelineKind::Home => 0,
        TimelineKind::Local => 1,
        TimelineKind::Public => 2,
    }
}

/// The field of the composer that has the keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Field {
    Text,
    Warning,
}

/// The composer, for a new status or a reply.
#[derive(Debug, Clone)]
pub(crate) struct Compose {
    pub text: TextInput,
    /// The content warning, when one is added.
    pub warning: Option<TextInput>,
    pub focus: Field,
    pub visibility: Visibility,
    /// The status it replies to, and its author as `@acct`.
    pub reply: Option<(String, String)>,
    /// The number of the post sent and not answered yet.
    pub sending: Option<u64>,
}

impl Compose {
    fn new(reply: Option<&Status>, mention: Option<&str>) -> Self {
        let mentions: String = match (reply, mention) {
            (Some(s), Some(me)) => {
                let mut v: Vec<String> = vec![s.account.acct.clone()];
                v.extend(s.mentions.iter().map(|m| m.acct.clone()));
                let mut seen = HashSet::new();
                v.into_iter()
                    .filter(|a| !a.is_empty() && !a.eq_ignore_ascii_case(me))
                    .filter(|a| seen.insert(a.to_ascii_lowercase()))
                    .map(|a| format!("@{a} "))
                    .collect()
            }
            _ => String::new(),
        };
        Self {
            text: TextInput::multi(&mentions),
            // A reply keeps the warning of the status it answers, as
            // Mastodon's own composer does.
            warning: reply
                .filter(|s| !s.spoiler_text.is_empty())
                .map(|s| TextInput::single(&s.spoiler_text)),
            focus: Field::Text,
            visibility: reply.map_or(Visibility::Public, |s| visibility_of(&s.visibility)),
            reply: reply.map(|s| (s.id.clone(), format!("@{}", s.account.acct))),
            sending: None,
        }
    }

    /// The field with the keys.
    fn field(&mut self) -> &mut TextInput {
        match (self.focus, &mut self.warning) {
            (Field::Warning, Some(w)) => w,
            _ => &mut self.text,
        }
    }
}

/// The visibility Mastodon names `name`, public for one it does not.
fn visibility_of(name: &str) -> Visibility {
    match name {
        "unlisted" => Visibility::Unlisted,
        "private" => Visibility::Private,
        "direct" => Visibility::Direct,
        _ => Visibility::Public,
    }
}

/// The next visibility ctrl+p offers.
fn next_visibility(v: Visibility) -> Visibility {
    match v {
        Visibility::Public => Visibility::Unlisted,
        Visibility::Unlisted => Visibility::Private,
        Visibility::Private => Visibility::Direct,
        Visibility::Direct => Visibility::Public,
    }
}

/// What a visibility is called on screen.
pub(crate) fn visibility_label(v: Visibility) -> &'static str {
    match v {
        Visibility::Public => n!("public"),
        Visibility::Unlisted => n!("quiet public"),
        Visibility::Private => n!("followers only"),
        Visibility::Direct => n!("mentioned only"),
    }
}

/// A box over the screen.
#[derive(Debug, Clone)]
pub(crate) enum Overlay {
    /// The `?` help, scrolled this far.
    Help {
        scroll: u16,
    },
    /// What `.` lists the keys do to the selected status.
    Actions {
        selected: usize,
    },
    /// The theme picker: the one previewed, and the one in use before.
    Themes {
        selected: usize,
        before: usize,
    },
    Compose(Box<Compose>),
    Settings {
        selected: usize,
    },
    Languages {
        selected: usize,
    },
    Accounts {
        selected: usize,
    },
    Viewer {
        files: Vec<Media>,
        index: usize,
        generation: u32,
    },
}

/// A line on the status row, or an error over the screen.
#[derive(Debug, Clone)]
pub(crate) struct Message {
    pub text: String,
    pub error: bool,
    at: Instant,
}

/// Which way a status was acted on, for the write on its way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Act {
    Favourite,
    Boost,
}

/// The client's state.
pub(crate) struct App {
    /// The account it reads as; a visitor without one.
    pub session: Option<Session>,
    /// The server it reads.
    pub origin: String,
    pub tab: Tab,
    /// Home, Local and Federated.
    pub timelines: [List<Status>; 3],
    pub notifications: List<Notification>,
    pub search: Search,
    pub profile: Profile,
    pub accounts: Vec<Session>,
    switch_to: Option<Session>,
    locale: crate::i18n::Lang,
    settings_return: bool,
    /// The threads opened, the last on top.
    pub threads: Vec<ThreadView>,
    pub overlay: Option<Overlay>,
    /// The statuses whose content warning `w` opened.
    pub opened: HashSet<String>,
    pub message: Option<Message>,
    pub theme: Theme,
    pub theme_index: usize,
    depth: ColorDepth,
    /// The settings kept for the next run, when they can be.
    settings: Option<Settings>,
    /// Settings changed and not saved yet.
    settings_save: bool,
    /// The address `c` copied, for the loop to put on the clipboard.
    copy: Option<String>,
    /// The limits of a post on this server.
    pub limits: Limits,
    /// The writes on their way, by status, so a second key on it waits.
    acting: HashSet<(String, Act)>,
    /// The jobs sent whose answers the screen waits for.
    waiting: HashSet<u64>,
    seq: u64,
    pub quit: bool,
}

impl App {
    /// The client for `session` (a visitor without one) on `origin`, and
    /// the first pages to ask for.
    pub(crate) fn new(session: Option<Session>, origin: &str) -> (Self, Jobs) {
        let (theme_index, theme) = theme::chosen(None, ColorDepth::TrueColor);
        let tab = if session.is_some() {
            Tab::Home
        } else {
            Tab::Local
        };
        let mut app = Self {
            session,
            origin: origin.to_string(),
            tab,
            timelines: Default::default(),
            notifications: List::default(),
            search: Search::default(),
            profile: Profile::default(),
            accounts: Vec::new(),
            switch_to: None,
            locale: crate::i18n::current(),
            settings_return: false,
            threads: Vec::new(),
            overlay: None,
            opened: HashSet::new(),
            message: None,
            theme,
            theme_index,
            depth: ColorDepth::TrueColor,
            settings: None,
            settings_save: false,
            copy: None,
            limits: Limits::default(),
            acting: HashSet::new(),
            waiting: HashSet::new(),
            seq: 0,
            quit: false,
        };
        let mut jobs = app.load_tab(tab, false);
        if app.session.is_some() {
            jobs.push(app.job(Job::Limits));
        }
        (app, jobs)
    }

    /// Use `settings` (none when they cannot be kept), on a terminal of
    /// `depth`.
    pub(crate) fn apply_settings(&mut self, settings: Option<Settings>, depth: ColorDepth) {
        self.depth = depth;
        let lang = settings
            .as_ref()
            .and_then(|s| s.language.as_deref())
            .and_then(crate::i18n::Lang::from_code)
            .unwrap_or(self.locale);
        crate::i18n::set(lang);
        let name = settings.as_ref().and_then(|s| s.theme.clone());
        self.settings = settings;
        let (i, t) = theme::chosen(name.as_deref(), depth);
        self.theme_index = i;
        self.theme = t;
    }

    /// The settings to save, once, when they changed.
    pub(crate) fn take_settings_save(&mut self) -> Option<Settings> {
        if !std::mem::take(&mut self.settings_save) {
            return None;
        }
        self.settings.clone()
    }

    /// The address to put on the clipboard, once.
    pub(crate) fn take_copy(&mut self) -> Option<String> {
        self.copy.take()
    }

    /// The server's host.
    pub(crate) fn host(&self) -> &str {
        crate::api::authority(&self.origin)
    }

    /// How many answers the screen waits for.
    pub(crate) fn pending(&self) -> usize {
        self.waiting.len()
    }

    /// Say `text` on the status row.
    pub(crate) fn say(&mut self, text: impl Into<String>) {
        self.message = Some(Message {
            text: text.into(),
            error: false,
            at: Instant::now(),
        });
    }

    /// Show `text` as an error over the screen.
    pub(crate) fn fail(&mut self, text: impl Into<String>) {
        self.message = Some(Message {
            text: text.into(),
            error: true,
            at: Instant::now(),
        });
    }

    /// Take the message away once it has been up long enough. Returns
    /// whether it went.
    pub(crate) fn expire_message(&mut self, now: Instant) -> bool {
        let gone = self.message.as_ref().is_some_and(|m| {
            now.duration_since(m.at) > if m.error { ERROR_FOR } else { MESSAGE_FOR }
        });
        if gone {
            self.message = None;
        }
        gone
    }

    /// `job` numbered, its answer waited for.
    fn job(&mut self, job: Job) -> (u64, Job) {
        self.seq += 1;
        self.waiting.insert(self.seq);
        (self.seq, job)
    }

    /// The timeline list of `kind`.
    pub(crate) fn timeline(&self, kind: TimelineKind) -> &List<Status> {
        &self.timelines[timeline_index(kind)]
    }

    /// Ask for the first page of `tab`, or the next one with `more`. A tab
    /// the server shows no visitor is not asked for without an account.
    fn load_tab(&mut self, tab: Tab, more: bool) -> Jobs {
        match tab {
            Tab::Search => return self.run_search(more),
            Tab::Profile => return self.load_profile(more),
            _ => {}
        }
        if tab.needs_login() && self.session.is_none() {
            return Vec::new();
        }
        let (seq, job) = match tab.kind() {
            Some(kind) => {
                let max_id = if more {
                    self.timeline(kind).next.clone()
                } else {
                    None
                };
                self.job(Job::Timeline { kind, max_id })
            }
            None => {
                let max_id = if more {
                    self.notifications.next.clone()
                } else {
                    None
                };
                self.job(Job::Notifications { max_id })
            }
        };
        match tab.kind() {
            Some(kind) => self.timelines[timeline_index(kind)].waiting = Some(seq),
            None => self.notifications.waiting = Some(seq),
        }
        vec![(seq, job)]
    }

    /// Show `tab`, asking for its first page when it has none yet.
    fn show_tab(&mut self, tab: Tab) -> Jobs {
        self.tab = tab;
        self.threads.clear();
        if tab == Tab::Search {
            self.search.editing = true;
            return Vec::new();
        }
        if tab == Tab::Profile {
            self.profile = Profile::default();
            return self.load_profile(false);
        }
        let loaded = match tab.kind() {
            Some(kind) => {
                let l = self.timeline(kind);
                l.loaded || l.waiting.is_some()
            }
            None => self.notifications.loaded || self.notifications.waiting.is_some(),
        };
        if loaded {
            Vec::new()
        } else {
            self.load_tab(tab, false)
        }
    }

    /// The status the keys act on: the selected one of the thread on top,
    /// else of the tab; a notification's status. The one a boost boosts is
    /// what a reader sees, so that is the one.
    pub(crate) fn selected(&self) -> Option<&Status> {
        if let Some(th) = self.threads.last() {
            return th.rows.current().map(|r| r.status.shown());
        }
        match self.tab {
            Tab::Search => {
                return if self.search.accounts {
                    None
                } else {
                    self.search.posts.current().map(Status::shown)
                };
            }
            Tab::Profile => return self.profile.posts.current().map(Status::shown),
            _ => {}
        }
        match self.tab.kind() {
            Some(kind) => self.timeline(kind).current().map(Status::shown),
            None => self
                .notifications
                .current()
                .and_then(|n| n.status.as_ref())
                .map(Status::shown),
        }
    }

    /// Whether the text behind `s`'s warning is shown.
    pub(crate) fn text_shown(&self, s: &Status) -> bool {
        s.spoiler_text.is_empty() || self.opened.contains(&s.id)
    }

    /// Move the selection of the list shown by `by`, asking for the next
    /// page near the end.
    fn step(&mut self, by: isize) -> Jobs {
        if let Some(th) = self.threads.last_mut() {
            th.rows.step(by);
            return Vec::new();
        }
        if matches!(self.tab, Tab::Search | Tab::Profile) {
            return self.browse_step(by);
        }
        let wants = match self.tab.kind() {
            Some(kind) => {
                let l = &mut self.timelines[timeline_index(kind)];
                l.step(by);
                l.wants_more()
            }
            None => {
                self.notifications.step(by);
                self.notifications.wants_more()
            }
        };
        if wants {
            self.load_tab(self.tab, true)
        } else {
            Vec::new()
        }
    }

    /// Go to the first or the last item.
    fn jump(&mut self, last: bool) -> Jobs {
        let by = if last { isize::MAX / 2 } else { isize::MIN / 2 };
        self.step(by)
    }

    /// Ask for the view shown again: the thread on top, else the tab.
    fn refresh(&mut self) -> Jobs {
        if let Some(id) = self.threads.last().map(|t| t.id.clone()) {
            let (seq, job) = self.job(Job::Thread { id });
            if let Some(th) = self.threads.last_mut() {
                th.rows.waiting = Some(seq);
            }
            return vec![(seq, job)];
        }
        if self.tab.needs_login() && self.session.is_none() {
            return Vec::new();
        }
        self.load_tab(self.tab, false)
    }

    /// Open the thread of the selected status.
    fn open_thread(&mut self) -> Jobs {
        let Some(s) = self.selected().cloned() else {
            return Vec::new();
        };
        // The status is shown at once; what it answers and its replies come.
        let mut rows = List {
            items: vec![Row {
                status: s.clone(),
                depth: 0,
                focus: true,
            }],
            loaded: false,
            ..List::default()
        };
        let (seq, job) = self.job(Job::Thread { id: s.id.clone() });
        rows.waiting = Some(seq);
        self.threads.push(ThreadView { id: s.id, rows });
        vec![(seq, job)]
    }

    /// What to say when a write needs an account.
    fn needs_account(&mut self) {
        let host = self.host().to_string();
        self.say(tf(
            "log in to do that: run `mstdn login --host {}`",
            &[&host],
        ));
    }

    /// Favourite the selected status, or take the favourite back.
    fn favourite(&mut self) -> Jobs {
        self.act(Act::Favourite)
    }

    /// Boost the selected status, or take the boost back.
    fn boost(&mut self) -> Jobs {
        self.act(Act::Boost)
    }

    fn act(&mut self, act: Act) -> Jobs {
        let Some(s) = self.selected() else {
            return Vec::new();
        };
        let id = s.id.clone();
        let closed = matches!(s.visibility.as_str(), "private" | "direct");
        let on = match act {
            Act::Favourite => s.favourited != Some(true),
            Act::Boost => s.reblogged != Some(true),
        };
        if self.session.is_none() {
            self.needs_account();
            return Vec::new();
        }
        if act == Act::Boost && closed {
            self.say(t(
                "a post for followers or the mentioned only cannot be boosted",
            ));
            return Vec::new();
        }
        if !self.acting.insert((id.clone(), act)) {
            return Vec::new();
        }
        // Shown at once; taken back if the server says no.
        self.mark(&id, act, on);
        let job = match act {
            Act::Favourite => Job::Favourite { id, on },
            Act::Boost => Job::Boost { id, on },
        };
        let (seq, job) = self.job(job);
        vec![(seq, job)]
    }

    /// Show status `id` as favourited or boosted (`on`) or not, wherever
    /// it is shown, its count with it.
    fn mark(&mut self, id: &str, act: Act, on: bool) {
        self.each_status(id, |s| {
            let (flag, count) = match act {
                Act::Favourite => (&mut s.favourited, &mut s.favourites_count),
                Act::Boost => (&mut s.reblogged, &mut s.reblogs_count),
            };
            if (*flag == Some(true)) != on {
                *count = if on {
                    count.saturating_add(1)
                } else {
                    count.saturating_sub(1)
                };
            }
            *flag = Some(on);
        });
    }

    /// Apply `f` to status `id` wherever it is shown: on its own, as the
    /// one a boost boosts, as what a notification is about, in a thread.
    fn each_status(&mut self, id: &str, mut f: impl FnMut(&mut Status)) {
        let mut visit = |s: &mut Status| {
            if s.id == id {
                f(s);
            }
            if let Some(b) = s.reblog.as_deref_mut()
                && b.id == id
            {
                f(b);
            }
        };
        for l in &mut self.timelines {
            l.items.iter_mut().for_each(&mut visit);
        }
        self.search.posts.items.iter_mut().for_each(&mut visit);
        self.profile.posts.items.iter_mut().for_each(&mut visit);
        for n in &mut self.notifications.items {
            if let Some(s) = n.status.as_mut() {
                visit(s);
            }
        }
        for th in &mut self.threads {
            for r in &mut th.rows.items {
                visit(&mut r.status);
            }
        }
    }

    /// Open the composer: for a reply to the selected status, or a new one.
    fn compose(&mut self, reply: bool) {
        if self.session.is_none() {
            self.needs_account();
            return;
        }
        let me = self.session.as_ref().map(|s| s.username.clone());
        let to = if reply {
            self.selected().cloned()
        } else {
            None
        };
        if reply && to.is_none() {
            return;
        }
        self.overlay = Some(Overlay::Compose(Box::new(Compose::new(
            to.as_ref(),
            me.as_deref(),
        ))));
    }

    /// Show or hide the text behind the selected status's warning.
    fn toggle_warning(&mut self) {
        let Some(s) = self.selected() else { return };
        if s.spoiler_text.is_empty() {
            return;
        }
        let id = s.id.clone();
        if !self.opened.remove(&id) {
            self.opened.insert(id);
        }
    }

    /// The selected status's page, on its own server.
    fn address(&self) -> Option<String> {
        let s = self.selected()?;
        s.url
            .clone()
            .filter(|u| !u.is_empty())
            .or_else(|| (!s.uri.is_empty()).then(|| s.uri.clone()))
    }

    /// Handle a key; the jobs it starts.
    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> Jobs {
        // An error over the screen goes with any key, which does nothing else.
        if self.message.as_ref().is_some_and(|m| m.error) {
            self.message = None;
            return Vec::new();
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return Vec::new();
        }
        if let Some(overlay) = self.overlay.take() {
            return self.overlay_key(overlay, key);
        }
        if self.tab == Tab::Search && self.threads.is_empty() && self.search.editing {
            return self.search_key(key);
        }
        self.list_key(key)
    }

    /// Text pasted: into the composer's field, when it is open.
    pub(crate) fn handle_paste(&mut self, text: &str) {
        if self.overlay.is_none() && self.tab == Tab::Search && self.search.editing {
            self.search.input.insert_str(text);
        }
        if let Some(Overlay::Compose(c)) = &mut self.overlay
            && c.sending.is_none()
        {
            c.field().insert_str(text);
        }
    }

    fn list_key(&mut self, key: KeyEvent) -> Jobs {
        match key.code {
            KeyCode::Char('q') => {
                self.quit = true;
                Vec::new()
            }
            KeyCode::Esc => {
                if self.threads.is_empty() && self.tab == Tab::Profile {
                    self.tab = self.profile.back.take().unwrap_or(Tab::Home);
                }
                self.threads.pop();
                Vec::new()
            }
            KeyCode::Char(c @ '1'..='6') => {
                let i = usize::from(c as u8 - b'1');
                self.show_tab(Tab::ALL[i])
            }
            KeyCode::Tab => self.show_tab(Tab::ALL[(self.tab.index() + 1) % Tab::ALL.len()]),
            KeyCode::BackTab => {
                let n = Tab::ALL.len();
                self.show_tab(Tab::ALL[(self.tab.index() + n - 1) % n])
            }
            KeyCode::Char('j') | KeyCode::Down => self.step(1),
            KeyCode::Char('k') | KeyCode::Up => self.step(-1),
            KeyCode::PageDown => self.step(PAGE_STEP as isize),
            KeyCode::PageUp => self.step(-(PAGE_STEP as isize)),
            KeyCode::Char('g') | KeyCode::Home => self.jump(false),
            KeyCode::Char('G') | KeyCode::End => self.jump(true),
            KeyCode::Char('R') | KeyCode::F(5) => self.refresh(),
            KeyCode::Char('v') => self.open_thread(),
            KeyCode::Enter => self.open_profile(),
            KeyCode::Char('/') => {
                self.tab = Tab::Search;
                self.threads.clear();
                self.search.editing = true;
                Vec::new()
            }
            KeyCode::Char('i') if self.tab == Tab::Search => {
                self.search.editing = true;
                Vec::new()
            }
            KeyCode::Char('t') if self.tab == Tab::Search && self.threads.is_empty() => {
                self.search.accounts = !self.search.accounts;
                self.run_search(false)
            }
            KeyCode::Char(' ') => {
                self.open_viewer();
                Vec::new()
            }
            KeyCode::Char('d') => self.download(),
            KeyCode::Char('A') => {
                self.overlay = Some(Overlay::Accounts {
                    selected: self
                        .accounts
                        .iter()
                        .position(|s| Some(s) == self.session.as_ref())
                        .unwrap_or(0),
                });
                Vec::new()
            }
            KeyCode::Char('s') => {
                self.overlay = Some(Overlay::Settings { selected: 0 });
                Vec::new()
            }
            KeyCode::Char('w') => {
                self.toggle_warning();
                Vec::new()
            }
            KeyCode::Char('l') => self.favourite(),
            KeyCode::Char('b') => self.boost(),
            KeyCode::Char('r') => {
                self.compose(true);
                Vec::new()
            }
            KeyCode::Char('n') => {
                self.compose(false);
                Vec::new()
            }
            KeyCode::Char('o') => match self.address() {
                Some(url) => {
                    let (seq, job) = self.job(Job::OpenLink { url });
                    vec![(seq, job)]
                }
                None => Vec::new(),
            },
            KeyCode::Char('c') => {
                if let Some(url) = self.address() {
                    self.copy = Some(url);
                    self.say(t("copied the address"));
                }
                Vec::new()
            }
            KeyCode::Char('.') => {
                if self.selected().is_some() {
                    self.overlay = Some(Overlay::Actions { selected: 0 });
                }
                Vec::new()
            }
            KeyCode::Char('T') => {
                self.overlay = Some(Overlay::Themes {
                    selected: self.theme_index,
                    before: self.theme_index,
                });
                Vec::new()
            }
            KeyCode::Char('?') => {
                self.overlay = Some(Overlay::Help { scroll: 0 });
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn overlay_key(&mut self, overlay: Overlay, key: KeyEvent) -> Jobs {
        match overlay {
            Overlay::Help { scroll } => {
                self.overlay = match key.code {
                    KeyCode::Esc | KeyCode::Char('q' | '?') => None,
                    KeyCode::Char('j') | KeyCode::Down => Some(Overlay::Help {
                        scroll: scroll.saturating_add(1),
                    }),
                    KeyCode::Char('k') | KeyCode::Up => Some(Overlay::Help {
                        scroll: scroll.saturating_sub(1),
                    }),
                    KeyCode::PageDown | KeyCode::Char(' ') => Some(Overlay::Help {
                        scroll: scroll.saturating_add(10),
                    }),
                    KeyCode::PageUp => Some(Overlay::Help {
                        scroll: scroll.saturating_sub(10),
                    }),
                    _ => Some(Overlay::Help { scroll }),
                };
                Vec::new()
            }
            Overlay::Actions { selected } => {
                let actions = keys::actions(self);
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q' | '.') => Vec::new(),
                    KeyCode::Char('j') | KeyCode::Down => {
                        self.overlay = Some(Overlay::Actions {
                            selected: (selected + 1).min(actions.len().saturating_sub(1)),
                        });
                        Vec::new()
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        self.overlay = Some(Overlay::Actions {
                            selected: selected.saturating_sub(1),
                        });
                        Vec::new()
                    }
                    KeyCode::Enter => match actions.get(selected) {
                        Some((k, _)) => self.list_key(keys::action_key(k)),
                        None => Vec::new(),
                    },
                    // A key the list names does what it says.
                    _ => self.list_key(key),
                }
            }
            Overlay::Themes { selected, before } => {
                let last = THEMES.len() - 1;
                let next = match key.code {
                    KeyCode::Char('j') | KeyCode::Down => Some((selected + 1).min(last)),
                    KeyCode::Char('k') | KeyCode::Up => Some(selected.saturating_sub(1)),
                    KeyCode::Char('g') | KeyCode::Home => Some(0),
                    KeyCode::Char('G') | KeyCode::End => Some(last),
                    KeyCode::Enter => {
                        self.keep_theme(selected);
                        self.return_settings();
                        return Vec::new();
                    }
                    KeyCode::Esc | KeyCode::Char('q') => {
                        self.use_theme(before);
                        self.return_settings();
                        return Vec::new();
                    }
                    _ => None,
                };
                let selected = next.unwrap_or(selected);
                self.use_theme(selected);
                self.overlay = Some(Overlay::Themes { selected, before });
                Vec::new()
            }
            Overlay::Compose(c) => self.compose_key(c, key),
            other => self.extra_overlay_key(other, key),
        }
    }

    /// Draw in theme `i`.
    fn use_theme(&mut self, i: usize) {
        self.theme_index = i;
        self.theme = THEMES[i].for_depth(self.depth);
    }

    /// Use theme `i` from now on, and keep it for the next run.
    fn keep_theme(&mut self, i: usize) {
        self.use_theme(i);
        let name = THEMES[i].name;
        match &mut self.settings {
            Some(s) => {
                s.theme = Some(name.to_string());
                self.settings_save = true;
            }
            None => self.say(t(
                "the theme is used until mstdn ends: there is no config directory to keep it in",
            )),
        }
    }

    /// Whether the theme could be kept; the error is said when it could not.
    pub(crate) fn settings_saved(&mut self, saved: Result<()>) {
        if let Err(e) = saved {
            self.fail(e.message().to_string());
        }
    }

    fn compose_key(&mut self, mut c: Box<Compose>, key: KeyEvent) -> Jobs {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if c.sending.is_some() {
            // Waiting for the server: esc closes the box, and the answer
            // still says how it went.
            if key.code != KeyCode::Esc {
                self.overlay = Some(Overlay::Compose(c));
            }
            return Vec::new();
        }
        match key.code {
            KeyCode::Esc => return Vec::new(),
            KeyCode::Char('s') if ctrl => return self.send(c),
            KeyCode::Char('w') if ctrl => {
                if c.warning.take().is_none() {
                    c.warning = Some(TextInput::single(""));
                    c.focus = Field::Warning;
                } else {
                    c.focus = Field::Text;
                }
            }
            KeyCode::Char('p') if ctrl => c.visibility = next_visibility(c.visibility),
            KeyCode::Tab | KeyCode::BackTab if c.warning.is_some() => {
                c.focus = match c.focus {
                    Field::Text => Field::Warning,
                    Field::Warning => Field::Text,
                };
            }
            _ => {
                c.field().handle_key(key);
            }
        }
        self.overlay = Some(Overlay::Compose(c));
        Vec::new()
    }

    /// How long the composer's text is as the server counts it, and the
    /// most it takes. The warning counts too.
    pub(crate) fn counted(&self, c: &Compose) -> (usize, usize) {
        let per_url = self.limits.characters_reserved_per_url;
        let mut n = crate::cli::counted(&c.text.text(), per_url);
        if let Some(w) = &c.warning {
            n += crate::cli::counted(&w.text(), per_url);
        }
        (n, self.limits.max_characters)
    }

    /// Send what the composer holds.
    fn send(&mut self, mut c: Box<Compose>) -> Jobs {
        let text = c.text.text();
        if text.trim().is_empty() {
            self.say(t("write something first"));
            self.overlay = Some(Overlay::Compose(c));
            return Vec::new();
        }
        let (n, max) = self.counted(&c);
        if n > max {
            self.say(tf(
                "too long: {} of {} characters",
                &[&n.to_string(), &max.to_string()],
            ));
            self.overlay = Some(Overlay::Compose(c));
            return Vec::new();
        }
        let draft = Draft {
            text,
            cw: c.warning.as_ref().map(TextInput::text).unwrap_or_default(),
            visibility: c.visibility,
            reply: c.reply.as_ref().map(|(id, _)| id.clone()),
        };
        let (seq, job) = self.job(Job::Post(Box::new(draft)));
        c.sending = Some(seq);
        self.overlay = Some(Overlay::Compose(c));
        self.say(t("posting…"));
        vec![(seq, job)]
    }

    /// Handle the answer to job `seq`; the jobs it starts.
    pub(crate) fn handle_answer(&mut self, seq: u64, ev: Event) -> Jobs {
        if !self.waiting.remove(&seq) {
            return Vec::new();
        }
        match ev {
            Event::Search { next, result } => self.search_answer(seq, next.is_none(), result),
            Event::Profile { more, result } => self.profile_answer(seq, !more, result),
            Event::Downloaded(result) => match result {
                Ok(n) => self.say(tf("saved {} files", &[&n.to_string()])),
                Err(e) => self.fail(e.message().to_string()),
            },
            Event::Timeline {
                kind,
                max_id,
                result,
            } => {
                let first = max_id.is_none();
                let page = result.map(|p| {
                    let next = next_of(&p, |s: &Status| &s.id);
                    (p.items, next)
                });
                self.timelines[timeline_index(kind)].take(seq, first, page);
            }
            Event::Notifications { max_id, result } => {
                let first = max_id.is_none();
                let page = result.map(|p| {
                    let next = next_of(&p, |n: &Notification| &n.id);
                    (p.items, next)
                });
                self.notifications.take(seq, first, page);
            }
            Event::Thread { id, result } => {
                if let Some(th) = self.threads.iter_mut().find(|t| t.id == id) {
                    let rows = result.map(|th| (ThreadView::rows(*th), None));
                    let at = th.rows.current().map(|r| r.status.id.clone());
                    let first_time = !th.rows.loaded;
                    if th.rows.take(seq, true, rows) && first_time {
                        // Opened on the status, wherever it is in its thread.
                        let focus = th.rows.items.iter().position(|r| r.focus);
                        let kept =
                            at.and_then(|id| th.rows.items.iter().position(|r| r.status.id == id));
                        th.rows.selected = focus.or(kept).unwrap_or(0);
                    }
                }
            }
            Event::Favourited { id, on, result } => {
                self.acting.remove(&(id.clone(), Act::Favourite));
                match result {
                    Ok(()) => self.say(if on {
                        t("favourited")
                    } else {
                        t("took the favourite back")
                    }),
                    Err(e) => {
                        self.mark(&id, Act::Favourite, !on);
                        self.fail(e.message().to_string());
                    }
                }
            }
            Event::Boosted { id, on, result } => {
                self.acting.remove(&(id.clone(), Act::Boost));
                match result {
                    Ok(()) => self.say(if on {
                        t("boosted")
                    } else {
                        t("took the boost back")
                    }),
                    Err(e) => {
                        self.mark(&id, Act::Boost, !on);
                        self.fail(e.message().to_string());
                    }
                }
            }
            Event::Posted(result) => {
                let open = matches!(
                    &self.overlay,
                    Some(Overlay::Compose(c)) if c.sending == Some(seq)
                );
                match result {
                    Ok(_) => {
                        if open {
                            self.overlay = None;
                        }
                        self.say(t("posted"));
                    }
                    Err(e) => {
                        if let Some(Overlay::Compose(c)) = &mut self.overlay
                            && c.sending == Some(seq)
                        {
                            c.sending = None;
                        }
                        self.fail(e.message().to_string());
                    }
                }
            }
            Event::Limits(limits) => self.limits = limits,
            Event::Opened(opened) => {
                if !opened {
                    self.say(t("no web browser could be opened"));
                }
            }
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests;
