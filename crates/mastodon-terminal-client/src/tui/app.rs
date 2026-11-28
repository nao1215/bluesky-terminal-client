//! The state of the full-screen client, as bsky's and msky's: keys and
//! answers come in, jobs for the worker go out, and [`super::view`] draws
//! what is here. Nothing here touches the terminal or the network, so the
//! tests drive it with keys and made-up answers.
//!
//! Every page asked for is numbered; an answer under a number the list no
//! longer waits for (the list was refreshed meanwhile) is dropped, so a
//! slow old page never lands on top of a new one.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use terminal_kit::input::TextInput;

use crate::api::types::{Account, Limits, Media, Notification, Status};
mod accounts;
mod browse;
mod chat;
mod deck;
mod edit_profile;
mod read_ahead;
mod settings;
mod write;
use crate::api::types::Relationship;
use crate::cli::{TimelineKind, Visibility};
use crate::config::{Session, Settings};
use crate::error::{Error, Result};
use crate::i18n::{n, t, tf};
use crate::tui::keys;
use crate::tui::list::{List, Row, ThreadView};
use crate::tui::theme::{self, ColorDepth, THEMES, Theme};
use crate::tui::worker::RelationAction;
use crate::tui::worker::{Draft, Event, Job, next_of};
pub(crate) use browse::{Profile, Search};
pub(crate) use edit_profile::EditProfile;
pub(crate) use write::{Attached, Confirmation, Report};

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
    Chat,
}

impl Tab {
    pub(crate) const ALL: [Tab; 7] = [
        Tab::Home,
        Tab::Local,
        Tab::Federated,
        Tab::Notifications,
        Tab::Search,
        Tab::Profile,
        Tab::Chat,
    ];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Tab::Home => n!("Home"),
            Tab::Local => n!("Local"),
            Tab::Federated => n!("Federated"),
            Tab::Notifications => n!("Notifications"),
            Tab::Search => n!("Search"),
            Tab::Profile => n!("Profile"),
            Tab::Chat => n!("Chat"),
        }
    }

    /// The timeline it shows, for the three that show one.
    pub(crate) fn kind(self) -> Option<TimelineKind> {
        match self {
            Tab::Home => Some(TimelineKind::Home),
            Tab::Local => Some(TimelineKind::Local),
            Tab::Federated => Some(TimelineKind::Public),
            Tab::Notifications | Tab::Search | Tab::Profile | Tab::Chat => None,
        }
    }

    /// Whether the server shows it to no visitor.
    pub(crate) fn needs_login(self) -> bool {
        matches!(self, Tab::Home | Tab::Notifications | Tab::Chat)
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
    Alt(usize),
}

/// The composer, for a new status or a reply.
#[derive(Debug, Clone)]
pub(crate) struct Compose {
    pub quote: Option<String>,
    pub quote_policy: Option<String>,
    pub media: Vec<Attached>,
    pub browser: Option<terminal_kit::files::Browser>,
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
            quote: None,
            quote_policy: None,
            media: Vec::new(),
            browser: None,
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
            (Field::Alt(i), _) => &mut self.media[i].alt,
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
    Login {
        input: TextInput,
        url: Option<String>,
        waiting: bool,
    },
    Logout {
        host: String,
    },
    AddColumn {
        selected: usize,
        query: Option<TextInput>,
    },
    CloseColumn(u64),
    EditProfile(Box<edit_profile::EditProfile>),
    Confirm(Confirmation),
    Report(Box<Report>),
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
    SettingText {
        row: usize,
        input: TextInput,
    },
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
    pub chat: chat::Chat,
    pub deck: crate::tui::columns::Deck,
    read_ahead: read_ahead::ReadAhead,
    pub unread: usize,
    pub relationships: HashMap<String, Relationship>,
    relation_pending: HashMap<String, RelationAction>,
    relation_before: HashMap<String, Relationship>,
    deleting: HashSet<String>,
    deleted: HashSet<String>,
    browse_from: Option<std::path::PathBuf>,
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
    login_host: Option<String>,
    login_open: Option<String>,
    logout_host: Option<String>,
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
            chat: Default::default(),
            deck: Default::default(),
            read_ahead: Default::default(),
            unread: 0,
            relationships: HashMap::new(),
            relation_pending: HashMap::new(),
            relation_before: HashMap::new(),
            deleting: HashSet::new(),
            deleted: HashSet::new(),
            browse_from: None,
            session,
            origin: origin.to_string(),
            tab,
            timelines: Default::default(),
            notifications: List::default(),
            search: Search::default(),
            profile: Profile::default(),
            accounts: Vec::new(),
            switch_to: None,
            login_host: None,
            login_open: None,
            logout_host: None,
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
            if app.session.as_ref().is_some_and(|s| s.user_id.is_empty()) {
                jobs.push(app.job(Job::Identity));
            }
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
        if self.deck.columns.is_empty() {
            self.restore_deck();
        }
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
        if job.changes_server() {
            self.invalidate_read_ahead();
        }
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
            Tab::Chat => return self.load_chat(more),
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
        if tab == Tab::Notifications {
            self.count_notifications();
        }
        if tab == Tab::Chat {
            return self.load_chat(false);
        }
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
        if self.in_deck() {
            return self
                .deck
                .columns
                .get(self.deck.focus)
                .and_then(|c| c.selected());
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
            Tab::Chat => {
                return self
                    .chat
                    .open
                    .as_ref()
                    .and_then(|o| o.posts.current())
                    .or_else(|| {
                        self.chat
                            .conversations
                            .current()
                            .and_then(|c| c.last_status.as_ref())
                    });
            }
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
        if self.tab == Tab::Chat {
            return self.step_chat(by);
        }
        if self.in_deck() {
            return self.step_column(by);
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
        self.invalidate_read_ahead();
        if let Some(id) = self.threads.last().map(|t| t.id.clone()) {
            let (seq, job) = self.job(Job::Thread { id });
            if let Some(th) = self.threads.last_mut() {
                th.rows.waiting = Some(seq);
            }
            return vec![(seq, job)];
        }
        if self.in_deck() {
            return self.load_column(self.deck.columns[self.deck.focus].id, false);
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
        if let Some((thread, fresh)) = self.cached_thread(&s.id) {
            rows.items = ThreadView::rows(thread);
            rows.selected = rows.items.iter().position(|r| r.focus).unwrap_or(0);
            rows.loaded = true;
            if fresh {
                self.threads.push(ThreadView { id: s.id, rows });
                return Vec::new();
            }
        }
        let mut jobs = Vec::new();
        let seq = if let Some(seq) = self.reading_thread(&s.id) {
            self.waiting.insert(seq);
            seq
        } else {
            let job = self.job(Job::Thread { id: s.id.clone() });
            let seq = job.0;
            jobs.push(job);
            seq
        };
        rows.waiting = Some(seq);
        self.threads.push(ThreadView { id: s.id, rows });
        jobs
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
        if let Some(o) = &mut self.chat.open {
            o.posts.items.iter_mut().for_each(&mut visit);
        }
        for c in &mut self.chat.conversations.items {
            if let Some(s) = &mut c.last_status {
                visit(s);
            }
        }
        for c in &mut self.deck.columns {
            c.posts.items.iter_mut().for_each(&mut visit);
            for n in &mut c.notifications.items {
                if let Some(s) = &mut n.status {
                    visit(s);
                }
            }
        }
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

    fn quote(&mut self) {
        if self.session.is_none() {
            self.needs_account();
            return;
        }
        let Some(s) = self.selected().cloned() else {
            return;
        };
        // A readable quote field or an explicit extension advertises support;
        // a compatibility version says nothing about it.
        if s.quote_approval.is_none() && !self.limits.extension_quotes {
            self.fail(
                Error::unsupported("quote posts", &self.origin)
                    .message()
                    .to_owned(),
            );
            return;
        }
        if s.visibility == "direct"
            || (!self.own(&s.account)
                && s.quote_approval
                    .as_ref()
                    .is_some_and(|p| p.current_user == "denied"))
        {
            self.say(t("this post does not allow quoting"));
            return;
        }
        self.compose(false);
        if let Some(Overlay::Compose(c)) = &mut self.overlay {
            c.quote = Some(s.id);
            if s.visibility == "private" {
                c.visibility = Visibility::Private;
            }
        }
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
        if let Some(jobs) = self.chat_key(key) {
            return jobs;
        }
        self.list_key(key)
    }

    /// Text pasted: into the composer's field, when it is open.
    pub(crate) fn handle_paste(&mut self, text: &str) {
        if self.overlay.is_none()
            && self.tab == Tab::Chat
            && let Some(o) = &mut self.chat.open
            && o.typing
            && o.sending.is_none()
        {
            o.input.insert_str(text);
        }
        if let Some(Overlay::Login {
            input,
            waiting: false,
            ..
        }) = &mut self.overlay
        {
            input.insert_str(text);
        }
        if let Some(Overlay::SettingText { input, .. }) = &mut self.overlay {
            input.insert_str(text);
        }
        if let Some(Overlay::AddColumn { query: Some(q), .. }) = &mut self.overlay {
            q.insert_str(text);
        }
        if let Some(Overlay::EditProfile(e)) = &mut self.overlay
            && e.waiting.is_none()
            && e.browser.is_none()
            && e.focus < 13
            && !(2..5).contains(&e.focus)
        {
            e.fields[e.focus].insert_str(text);
        }
        if let Some(Overlay::Report(r)) = &mut self.overlay
            && r.writing
            && !r.confirming
            && r.sending.is_none()
        {
            r.comment.insert_str(text);
        }
        if self.overlay.is_none() && self.tab == Tab::Search && self.search.editing {
            self.search.input.insert_str(text);
        }
        if let Some(Overlay::Compose(c)) = &mut self.overlay
            && c.sending.is_none()
            && c.browser.is_none()
        {
            c.field().insert_str(text);
        }
    }

    fn list_key(&mut self, key: KeyEvent) -> Jobs {
        match key.code {
            KeyCode::Char('+') if self.tab == Tab::Home && self.threads.is_empty() => {
                self.open_columns()
            }
            KeyCode::Char('x') if self.in_deck() => {
                self.overlay = Some(Overlay::CloseColumn(self.deck.columns[self.deck.focus].id));
                Vec::new()
            }
            KeyCode::Left | KeyCode::Char('H') if self.in_deck() => {
                self.deck.focus = self.deck.focus.saturating_sub(1);
                Vec::new()
            }
            KeyCode::Right | KeyCode::Char('L') if self.in_deck() => {
                self.deck.focus = (self.deck.focus + 1).min(self.deck.columns.len() - 1);
                Vec::new()
            }
            KeyCode::Char(c @ ('<' | '>')) if self.in_deck() => {
                let to = self
                    .deck
                    .focus
                    .saturating_add_signed(if c == '<' { -1 } else { 1 })
                    .min(self.deck.columns.len() - 1);
                self.deck.columns.swap(self.deck.focus, to);
                self.deck.focus = to;
                self.save_deck();
                Vec::new()
            }
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
            KeyCode::Char(c @ '1'..='7') => {
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
            KeyCode::Char('e') => self.edit_profile(),
            KeyCode::Char('m') => {
                self.message_profile();
                Vec::new()
            }
            KeyCode::Char('Q') => {
                self.quote();
                Vec::new()
            }
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
            KeyCode::Char('f') => self.relation_key(RelationAction::Follow),
            KeyCode::Char('M') => self.relation_key(RelationAction::Mute),
            KeyCode::Char('B') => self.relation_key(RelationAction::Block),
            KeyCode::Char('D') => {
                self.delete_key();
                Vec::new()
            }
            KeyCode::Char('!') => {
                self.open_report();
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
            KeyCode::Char('O') => {
                if let Some(s) = self.selected()
                    && self.text_shown(s)
                    && let Some(card) = &s.card
                {
                    let url = card.url.clone();
                    let program = self.settings.as_ref().and_then(|s| s.browser.clone());
                    vec![self.job(Job::OpenLink { url, program })]
                } else {
                    Vec::new()
                }
            }
            KeyCode::Char('o') => match self.address() {
                Some(url) => {
                    let (seq, job) = self.job(Job::OpenLink {
                        url,
                        program: self.settings.as_ref().and_then(|s| s.browser.clone()),
                    });
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
                if self.account_target().is_some() {
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
            Overlay::AddColumn { selected, query } => self.column_key(selected, query, key),
            Overlay::CloseColumn(id) => {
                if key.code == KeyCode::Char('y') {
                    self.deck.columns.retain(|c| c.id != id);
                    self.deck.focus = self
                        .deck
                        .focus
                        .min(self.deck.columns.len().saturating_sub(1));
                    self.save_deck();
                }
                Vec::new()
            }

            Overlay::EditProfile(e) => self.edit_profile_key(e, key),
            Overlay::Confirm(c) => self.confirm_key(c, key),
            Overlay::Report(r) => self.report_key(r, key),
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
                    KeyCode::PageDown => Some((selected + PAGE_STEP).min(last)),
                    KeyCode::PageUp => Some(selected.saturating_sub(PAGE_STEP)),
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
        if self.attachment_key(&mut c, key) {
            self.overlay = Some(Overlay::Compose(c));
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
            KeyCode::Char('q') if ctrl => {
                c.quote_policy = match c.quote_policy.as_deref() {
                    None => Some("public".into()),
                    Some("public") => Some("followers".into()),
                    Some("followers") => Some("nobody".into()),
                    _ => None,
                };
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let mut fields = vec![Field::Text];
                if c.warning.is_some() {
                    fields.push(Field::Warning);
                }
                fields.extend((0..c.media.len()).map(Field::Alt));
                let index = fields.iter().position(|f| *f == c.focus).unwrap_or(0);
                let next = if key.code == KeyCode::Tab {
                    index + 1
                } else {
                    index + fields.len() - 1
                };
                c.focus = fields[next % fields.len()];
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
        if text.trim().is_empty() && c.media.is_empty() {
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
            quote: c.quote.clone(),
            quote_policy: c.quote_policy.clone(),
            files: c
                .media
                .iter()
                .map(|a| (a.path.clone(), a.alt.text()))
                .collect(),
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
        if matches!(ev, Event::Retrying(_)) {
            if !self.waiting.contains(&seq) {
                return Vec::new();
            }
        } else {
            if self.read_ahead_answer(seq, &ev) {
                return Vec::new();
            }
            if !self.waiting.remove(&seq) {
                return Vec::new();
            }
        }
        let write_finished = matches!(
            &ev,
            Event::Favourited { .. }
                | Event::Boosted { .. }
                | Event::Posted(_)
                | Event::Related { .. }
                | Event::Deleted { .. }
                | Event::Reported(_)
                | Event::ProfileSaved(_)
                | Event::ChatPosted(_)
                | Event::ConversationRead(_)
        );
        match ev {
            Event::Retrying(message) => self.say(message),
            Event::NotificationState(result) => self.notification_state(seq, result),
            Event::NotificationsMarked { id, result } => self.notifications_marked(seq, id, result),
            Event::Conversations { first, result } => self.conversations_answer(seq, first, result),
            Event::ChatThread(result) => return self.chat_thread_answer(seq, result),
            Event::ConversationRead(result) => self.chat_read_answer(seq, result),
            Event::ChatPosted(result) => self.chat_post_answer(seq, result),

            Event::Column { id, first, result } => self.column_answer(seq, id, first, result),
            Event::ColumnChoices(result) => {
                if self.deck.choices_waiting == Some(seq) {
                    self.deck.choices_waiting = None;
                    match result {
                        Ok(choices) => self.deck.choices = choices,
                        Err(e) => self.fail(e.message().to_owned()),
                    }
                }
            }

            Event::ProfileEditor(result) => self.profile_editor_answer(seq, result, false),
            Event::ProfileSaved(result) => self.profile_editor_answer(seq, result, true),
            Event::Identity(result) => match result {
                Ok(a) => {
                    if let Some(session) = &mut self.session {
                        session.user_id = a.id;
                        session.username = a.username;
                    }
                    if self.deck.columns.is_empty() {
                        self.restore_deck();
                        return self.load_deck();
                    }
                }
                Err(e) => self.fail(e.message().to_string()),
            },
            Event::Relationship { id, result } => return self.relation_loaded(id, result),
            Event::Related { id, result } => self.related(id, result),
            Event::Deleted { id, result } => self.deleted(id, result),
            Event::Reported(result) => self.reported(seq, result),
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
                if self.notifications.take(seq, first, page) {
                    self.count_notifications();
                }
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
        self.hide_deleted();
        if write_finished
            && self
                .threads
                .last()
                .is_some_and(|t| !t.rows.loaded && t.rows.waiting.is_none())
        {
            return self.refresh();
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests;
