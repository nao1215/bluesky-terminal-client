//! UI state and what every key does. Nothing here touches the terminal or
//! the network, as in bsky: keys and finished jobs go in, numbered
//! [`Job`]s come out.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::api::types::{DriveFile, Note, Notification};
use crate::cli::TimelineKind;
use crate::config::Session;
use crate::error::Error;
use crate::i18n::n;
use crate::servers::Server;
use crate::tui::keys;
use crate::tui::theme::{self, Theme};
use crate::tui::worker::{Event, Job, Thread};

mod browse;
mod chat;
mod deck;
mod edit_profile;
mod read_ahead;
mod report;
mod settings;
mod write;

#[cfg(test)]
mod tests;

pub(crate) use browse::{Profile, Search, SearchMode};
pub(crate) use chat::{ChatPane, partner, partner_user};
pub(crate) use edit_profile::EditProfile;
pub(crate) use report::Report;
pub(crate) use settings::{Environment, SettingRow};
pub(crate) use write::Compose;

/// The tabs. Each timeline is a tab of its own, and the tabs are always
/// the same ones in the same order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Timeline(TimelineKind),
    Notifications,
    Profile,
    Chat,
    /// The Home tab with columns beside the home timeline, added with `+`.
    Columns,
}

/// The tabs, in the order of their numbers. Social, home and local mixed,
/// is not one of them; `msky timeline social` reads it.
pub(crate) const TABS: [Tab; 6] = [
    Tab::Timeline(TimelineKind::Home),
    Tab::Timeline(TimelineKind::Local),
    Tab::Timeline(TimelineKind::Global),
    Tab::Notifications,
    Tab::Profile,
    Tab::Chat,
];

impl Tab {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Tab::Timeline(TimelineKind::Home) => n!("Home"),
            Tab::Timeline(TimelineKind::Local) => n!("Local"),
            Tab::Timeline(TimelineKind::Social) => n!("Social"),
            Tab::Timeline(TimelineKind::Global) => n!("Global"),
            Tab::Notifications => n!("Notifications"),
            Tab::Profile => n!("Profile"),
            Tab::Chat => n!("Chat"),
            Tab::Columns => n!("Home"),
        }
    }
}

/// Items that are the same item when their keys are equal: a further page
/// can repeat what the last one ended with.
pub(crate) trait Keyed {
    fn key(&self) -> &str;
}

impl Keyed for Note {
    fn key(&self) -> &str {
        &self.id
    }
}

impl Keyed for Notification {
    fn key(&self) -> &str {
        &self.id
    }
}

impl Keyed for ThreadRow {
    fn key(&self) -> &str {
        &self.note.id
    }
}

/// How close to the end the selection gets before the next page is
/// fetched, as in bsky: far enough that it has usually come before the
/// end is reached.
const MORE_AHEAD: usize = 15;

/// A scrollable list with a selection.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct List<T> {
    pub items: Vec<T>,
    pub selected: usize,
    /// First item drawn; kept by the view so the selection stays visible.
    pub offset: usize,
    /// Whether a result (possibly empty) has arrived.
    pub loaded: bool,
    /// A first page has been asked for and not answered yet.
    pub loading: bool,
    /// The ID the next page is asked from (`untilId`); `None` once there is
    /// nothing more.
    pub cursor: Option<String>,
    /// A next page has been asked for and not answered yet.
    pub more_pending: bool,
    /// Why the last load failed; shown instead of "nothing here" while the
    /// list is empty.
    pub error: Option<String>,
    /// Whether R may help with the error: not when the list needs an
    /// account there is none of.
    pub retry: bool,
    /// The number of the job its first page was last asked with: the
    /// answer to an earlier one is of a list loaded again since.
    asked: u64,
}

impl<T> Default for List<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selected: 0,
            offset: 0,
            loaded: false,
            loading: false,
            cursor: None,
            more_pending: false,
            error: None,
            retry: true,
            asked: 0,
        }
    }
}

impl<T: Keyed> List<T> {
    /// Replace the list with a first page. The selection stays on the item
    /// it was on when that item is in the page (R keeps the reader's
    /// place); otherwise it goes to the top. Misskey may answer with fewer
    /// items than asked while there are more, so only an empty page says
    /// there is nothing after it.
    fn set(&mut self, page: Vec<T>) {
        let kept = self
            .current()
            .and_then(|old| page.iter().position(|i| i.key() == old.key()));
        self.cursor = page.last().map(|i| i.key().to_string());
        self.items = page;
        self.more_pending = false;
        self.selected = kept.unwrap_or(0);
        self.offset = self.offset.min(self.selected);
        self.loaded = true;
        self.loading = false;
        self.error = None;
    }

    /// The ID to fetch from when the selection is near the end, a next page
    /// exists, and none is on its way. Marks the page as asked for.
    fn want_more(&mut self) -> Option<String> {
        if !self.loaded || self.more_pending || self.selected + MORE_AHEAD < self.items.len() {
            return None;
        }
        let cursor = self.cursor.clone()?;
        self.more_pending = true;
        Some(cursor)
    }

    /// Add a further page fetched from `requested`, keeping the selection.
    /// A page that no longer continues this list (it was loaded again
    /// meanwhile) is dropped. Items already present are skipped, and a page
    /// with nothing new ends the list: a server that ignores `untilId`
    /// would otherwise be asked for the same page for good.
    fn append(&mut self, requested: &str, page: Vec<T>) {
        if !self.more_pending || self.cursor.as_deref() != Some(requested) {
            return;
        }
        self.more_pending = false;
        let seen: HashSet<String> = self.items.iter().map(|i| i.key().to_string()).collect();
        let before = self.items.len();
        self.items
            .extend(page.into_iter().filter(|i| !seen.contains(i.key())));
        self.cursor = if self.items.len() > before {
            self.items.last().map(|i| i.key().to_string())
        } else {
            None
        };
    }
}

impl<T> List<T> {
    /// A first page has been asked for. A next page still on its way is of
    /// the list before: it is not waited for.
    fn begin(&mut self) {
        self.loaded = false;
        self.loading = true;
        self.error = None;
        self.more_pending = false;
    }

    /// The next page from `requested` could not be loaded: the next move
    /// asks again.
    fn more_failed(&mut self, requested: &str) {
        if self.cursor.as_deref() == Some(requested) {
            self.more_pending = false;
        }
    }

    /// The first page could not be loaded; what was there stays.
    fn failed(&mut self, why: String) {
        self.loaded = true;
        self.loading = false;
        self.error = Some(why);
        self.retry = true;
    }

    /// The list is an account's, and no account is logged in: `why` says
    /// how to log one in.
    fn needs_account(&mut self, why: String) {
        self.failed(why);
        self.retry = false;
    }

    pub(crate) fn current(&self) -> Option<&T> {
        self.items.get(self.selected)
    }

    /// Keep the items `keep` says to. The selection and the scroll stay on
    /// the item they were on, or the next one when it went, as bsky's.
    fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        let (mut selected, mut offset, mut at) = (0, 0, 0);
        let (was_selected, was_offset) = (self.selected, self.offset);
        self.items.retain(|item| {
            let kept = keep(item);
            if kept && at < was_selected {
                selected += 1;
            }
            if kept && at < was_offset {
                offset += 1;
            }
            at += 1;
            kept
        });
        self.selected = selected.min(self.items.len().saturating_sub(1));
        self.offset = offset.min(self.selected);
    }

    pub(crate) fn step(&mut self, delta: isize) {
        if self.items.is_empty() {
            return;
        }
        let last = self.items.len() as isize - 1;
        self.selected = (self.selected as isize + delta).clamp(0, last) as usize;
    }
}

/// A note of a thread, and how deep in its replies it is.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ThreadRow {
    pub note: Note,
    pub depth: u16,
}

/// A thread opened with `v`, shown over the current tab.
#[derive(Debug, Clone, Default)]
pub(crate) struct ThreadView {
    /// The note it was opened on.
    pub id: String,
    pub list: List<ThreadRow>,
}

impl ThreadView {
    /// The rows of `t`: what the note answers, the note, then its replies,
    /// each with the replies to it under it.
    fn rows(t: Thread) -> (Vec<ThreadRow>, usize) {
        let at = t.ancestors.len();
        let mut rows: Vec<ThreadRow> = t
            .ancestors
            .into_iter()
            .map(|note| ThreadRow { note, depth: 0 })
            .collect();
        rows.push(ThreadRow {
            note: t.note,
            depth: 0,
        });
        for (r, under) in t.replies {
            rows.push(ThreadRow { note: r, depth: 1 });
            rows.extend(under.into_iter().map(|note| ThreadRow { note, depth: 2 }));
        }
        (rows, at)
    }
}

/// An account logged in, as the account list shows it. The token stays
/// in its file, which is read again when it is switched to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Account {
    /// The server's host, as the account's file is named.
    pub host: String,
    pub username: String,
}

impl From<&Session> for Account {
    fn from(s: &Session) -> Self {
        Self {
            host: s.host().to_string(),
            username: s.username.clone(),
        }
    }
}

/// The login form, opened when there is no account, and with `a` from the
/// account list: a server chosen from a list, or typed, as bsky's form
/// takes its service; then MiAuth in the web browser.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LoginForm {
    /// The servers offered: the built-in ones until Misskey Hub's list is in.
    pub servers: Vec<Server>,
    pub selected: usize,
    /// The list is on its way.
    pub loading: bool,
    /// A host being typed, for a server not listed.
    pub typed: Option<String>,
    /// The access asked for, waited for while it is allowed in the browser.
    pub waiting: Option<Waiting>,
    /// Why the last try failed, or why the Hub's list is not shown.
    pub error: Option<String>,
}

/// A login waiting to be allowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Waiting {
    pub origin: String,
    /// The MiAuth session the answer comes for.
    pub session: String,
    /// The page to allow it on.
    pub url: String,
}

impl LoginForm {
    fn new() -> Self {
        Self {
            servers: crate::servers::builtin(),
            selected: 0,
            loading: true,
            typed: None,
            waiting: None,
            error: None,
        }
    }
}

/// A file of a note, with the note it is attached to and its place there.
pub(crate) type Attached = (String, usize, DriveFile);

/// A window drawn over the current tab.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Overlay {
    /// The key reference, scrolled `scroll` lines down (the view clamps it).
    Help { scroll: u16 },
    /// A note's files, full screen, `index` the one shown; `replay` counts
    /// `r` presses, each playing the video from the start, as bsky's.
    Viewer {
        files: Vec<Attached>,
        index: usize,
        replay: u32,
    },
    /// The accounts logged in, opened with `A`.
    Accounts { selected: usize },
    /// What the keys of this view do to the selected note, as in bsky.
    Actions { selected: usize },
    /// The login form.
    Login(Box<LoginForm>),
    /// The composer, opened with `n`, `r` or `Q`.
    Compose(Box<Compose>),
    /// The reactions `e` offers: the recent ones and Misskey's default
    /// ones, or the server's custom emoji that match `query`.
    Reactions { selected: usize, query: String },
    /// The settings screen, opened with `s`; `edit` is a text row being
    /// changed.
    Settings {
        selected: usize,
        edit: Option<crate::tui::input::TextInput>,
    },
    /// The themes, opened with `T`: the one at `selected` is shown as a
    /// preview, `previous` comes back on esc.
    Themes { selected: usize, previous: usize },
    /// The languages, from the settings screen.
    Languages { selected: usize },
    /// What `+` adds a column of, or the search being typed for one.
    AddColumn {
        selected: usize,
        query: Option<crate::tui::input::TextInput>,
    },
    /// The profile editor, opened with `e` on your own profile.
    EditProfile(Box<EditProfile>),
    /// A report being made, opened with `!`.
    Report(Box<Report>),
}

/// A one-line message in the status row; it clears after [`STATUS_TTL`]
/// ([`ERROR_TTL`] for an error), as bsky's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Status {
    pub text: String,
    pub error: bool,
    pub at: Instant,
}

const STATUS_TTL: Duration = Duration::from_secs(5);
/// How often the unread count is asked for.
const UNREAD_POLL: Duration = Duration::from_secs(60);
const ERROR_TTL: Duration = Duration::from_secs(10);

/// Jobs, each with the number it is sent and answered under.
pub(crate) type Jobs = Vec<(u64, Job)>;

/// All UI state.
#[derive(Debug, Clone)]
pub(crate) struct App {
    /// The account in use; `None` reads the server as a visitor.
    pub session: Option<Session>,
    /// The server, `https://misskey.io`.
    pub origin: String,
    pub tab: Tab,
    /// The timeline to return to after searching from any list.
    pub last_timeline: Tab,
    /// Every timeline, by [`TimelineKind::index`]: switching back to one
    /// finds it where it was left.
    pub timelines: [List<Note>; 4],
    pub notifications: List<Notification>,
    pub search: Search,
    pub profile: Profile,
    /// The Chat tab.
    pub chat: ChatPane,
    /// The columns of the Home tab.
    pub columns: crate::tui::columns::Columns,
    /// The account's antennas, lists and channels, which `+` offers, and
    /// whether they were asked for.
    pub column_feeds: Vec<crate::tui::columns::Source>,
    column_feeds_asked: bool,
    /// The column `x` asked to remove, waiting for `y`.
    pub confirm_remove_column: Option<u64>,
    /// Threads read ahead of `v`.
    pub read_ahead: read_ahead::ReadAhead,
    /// The notifications not read yet, as the Notifications tab counts
    /// them: the server's count, or `None` with `unread_any` for a server
    /// that only says whether there are any.
    pub unread: Option<u64>,
    pub unread_any: bool,
    /// When the count was last asked for.
    pub unread_polled: Option<Instant>,
    /// Threads opened with `v`, the last on top; Esc closes the top one.
    pub threads: Vec<ThreadView>,
    pub overlay: Option<Overlay>,
    pub status: Option<Status>,
    /// Jobs sent and not yet answered.
    pub pending: usize,
    pub quit: bool,
    /// Whether the terminal shows pictures. Without them the lists are
    /// text, and `space` opens a note in the web browser, as in bsky.
    pub pictures: bool,
    /// The notes whose content warning was opened with `w`.
    pub opened: HashSet<String>,
    pub theme: Theme,
    /// Where the theme is in bsky's list.
    pub theme_index: usize,
    pub color_depth: crate::tui::theme::ColorDepth,
    /// The settings, as `settings.json` keeps them.
    pub settings: crate::config::Settings,
    /// Whether they can be written: a file that could not be read is left
    /// as it is.
    pub settings_writable: bool,
    /// The variables that decide settings for the run.
    pub env: Environment,
    /// Settings to write, for the event loop, and what to say then.
    pub settings_to_save: Option<crate::config::Settings>,
    pub save_note: Option<String>,
    /// The settings row a list opened from, to come back to.
    pub settings_return: Option<usize>,
    /// Every account logged in, by host.
    pub accounts: Vec<Account>,
    /// The account the list asked to switch to, by host, for the event
    /// loop, which reads its file and gives the worker its token.
    account_switch: Option<String>,
    /// The account the list asked to log out, by host.
    account_logout: Option<String>,
    /// The login just made, for the event loop to save and use.
    logged_in: Option<Session>,
    /// Writes waiting for the server, keyed `react:<id>`, `renote:<id>`,
    /// `delete:<id>`: a second press is refused until the first is
    /// answered, or two presses would make two.
    pub in_flight: HashSet<String>,
    /// The notes renoted from here, which `b` takes back.
    pub renoted: HashSet<String>,
    /// The notifications that were unread when the tab read them, marked
    /// with a dot as bsky's are.
    pub fresh: HashSet<String>,
    /// The note `D` asked about, waiting for its `y`.
    pub confirm_delete: Option<String>,
    /// The account `B` asked about, waiting for its `y`.
    pub confirm_block: Option<String>,
    /// Whether the notifications loaded were marked read.
    read_sent: bool,
    /// The server's custom emoji, name to picture address.
    pub emojis: HashMap<String, String>,
    /// The reactions given lately, the newest first.
    pub recent_reactions: Vec<String>,
    /// The folder the file browser last showed, where it opens next.
    pub browse_from: Option<std::path::PathBuf>,
    /// The account `x` asked about, waiting for its `y`.
    pub confirm_logout: Option<String>,
    /// Text `c` put up for the terminal's clipboard, for the event loop.
    to_copy: Option<String>,
    /// Jobs numbered so far.
    sent: u64,
    /// Answers to jobs numbered before this are of the account before a
    /// switch.
    account_since: u64,
    /// The server has no chat for this account: the Chat tab is not shown.
    no_chat: bool,
}

/// What a timeline that needs an account says when there is none.
fn needs_login(host: &str, kind: TimelineKind) -> String {
    format!(
        "{} is your account's timeline, and no account is logged in on {host}: press A then a to log in (or run `msky login --host {host}`); the Local and Global tabs show the server to visitors",
        kind.name()
    )
}

impl App {
    /// A new UI on the server at `origin`, as `session` when there is one,
    /// else as a visitor. It opens on the home timeline, as Misskey's own
    /// client does; without an account home says how to log in, and the
    /// local and global timelines are a key away.
    pub(crate) fn new(session: Option<Session>, origin: &str) -> (Self, Jobs) {
        let mut app = Self {
            session,
            origin: origin.to_string(),
            tab: TABS[0],
            last_timeline: TABS[0],
            timelines: Default::default(),
            notifications: List::default(),
            search: Search::default(),
            profile: Profile::default(),
            chat: ChatPane::default(),
            columns: Default::default(),
            column_feeds: Vec::new(),
            column_feeds_asked: false,
            confirm_remove_column: None,
            read_ahead: Default::default(),
            unread: None,
            unread_any: false,
            unread_polled: None,
            threads: Vec::new(),
            overlay: None,
            status: None,
            pending: 0,
            quit: false,
            pictures: true,
            opened: HashSet::new(),
            theme: theme::chosen(None, theme::ColorDepth::TrueColor).1,
            theme_index: theme::chosen(None, theme::ColorDepth::TrueColor).0,
            color_depth: theme::ColorDepth::TrueColor,
            settings: crate::config::Settings::default(),
            settings_writable: true,
            env: Environment::default(),
            settings_to_save: None,
            save_note: None,
            settings_return: None,
            accounts: Vec::new(),
            account_switch: None,
            account_logout: None,
            logged_in: None,
            in_flight: HashSet::new(),
            renoted: HashSet::new(),
            fresh: HashSet::new(),
            confirm_delete: None,
            confirm_block: None,
            read_sent: false,
            browse_from: None,
            emojis: HashMap::new(),
            recent_reactions: Vec::new(),
            confirm_logout: None,
            to_copy: None,
            sent: 0,
            account_since: 0,
            no_chat: false,
        };
        let jobs = app.start();
        (app, jobs)
    }

    /// The server's host.
    pub(crate) fn host(&self) -> &str {
        crate::api::authority(&self.origin)
    }

    /// The first jobs for the account in use: the home timeline, and the
    /// notifications in the background, so their tab opens without
    /// waiting. Without an account, the timelines of one say so.
    fn start(&mut self) -> Jobs {
        self.timelines = Default::default();
        self.notifications = List::default();
        self.threads.clear();
        self.opened.clear();
        self.renoted.clear();
        self.fresh.clear();
        self.search = Search::default();
        self.last_timeline = Tab::Timeline(TimelineKind::Home);
        self.profile = Profile::default();
        self.chat = ChatPane::default();
        // Another server or account may have chat: it is asked again.
        self.no_chat = false;
        // The columns are the account's: another shows its own.
        self.column_feeds.clear();
        self.column_feeds_asked = false;
        self.confirm_remove_column = None;
        self.columns = Default::default();
        if self.session.is_none() {
            let host = self.host().to_string();
            for kind in TimelineKind::ALL.into_iter().filter(|k| k.needs_login()) {
                self.timelines[kind.index()].needs_account(needs_login(&host, kind));
            }
            self.notifications.needs_account(format!(
                "no account is logged in on {host}: press A then a to log in (or run `msky login --host {host}`)"
            ));
            self.info(format!(
                "not logged in on {host}: the Local and Global tabs show the server to visitors"
            ));
        }
        let mut jobs = match self.tab {
            Tab::Timeline(TimelineKind::Home) | Tab::Columns => self.show_columns(),
            Tab::Timeline(kind) => self.load_timeline(kind),
            _ => Vec::new(),
        };
        self.unread = None;
        self.unread_any = false;
        if self.session.is_some() {
            jobs.extend(self.load_notifications());
            jobs.extend(self.ask_unread(Instant::now()));
            // Asked now, so that on a server without chat its tab goes
            // before it is opened; on one with, the list is then ready.
            jobs.extend(self.load_chat());
        }
        jobs
    }

    /// Number `job`, counting it as waited for.
    fn stamp(&mut self, job: Job) -> (u64, Job) {
        self.sent += 1;
        self.pending += 1;
        if job.writes() {
            self.read_ahead.wrote(self.sent);
        }
        (self.sent, job)
    }

    fn info(&mut self, text: impl Into<String>) {
        self.status = Some(Status {
            // A string marked for translation shows in the language in use.
            text: crate::i18n::t(&text.into()).to_string(),
            error: false,
            at: Instant::now(),
        });
    }

    fn error(&mut self, text: impl Into<String>) {
        self.status = Some(Status {
            // A string marked for translation shows in the language in use.
            text: crate::i18n::t(&text.into()).to_string(),
            error: true,
            at: Instant::now(),
        });
    }

    /// Clear a status message that has been up long enough; whether the
    /// screen changed.
    pub(crate) fn expire_status(&mut self, now: Instant) -> bool {
        match &self.status {
            Some(s)
                if now.saturating_duration_since(s.at)
                    >= if s.error { ERROR_TTL } else { STATUS_TTL } =>
            {
                self.status = None;
                // The question went with its prompt.
                self.confirm_logout = None;
                self.confirm_delete = None;
                self.confirm_block = None;
                self.confirm_remove_column = None;
                true
            }
            _ => false,
        }
    }

    /// Whether a question waits for its `y`.
    fn asking(&self) -> bool {
        self.confirm_delete.is_some()
            || self.confirm_block.is_some()
            || self.confirm_logout.is_some()
            || self.confirm_remove_column.is_some()
    }

    /// Whether files are shown full screen.
    pub(crate) fn viewer_open(&self) -> bool {
        matches!(self.overlay, Some(Overlay::Viewer { .. }))
    }

    /// The text `c` put up for the clipboard, once.
    pub(crate) fn take_copy(&mut self) -> Option<String> {
        self.to_copy.take()
    }

    /// The account the list asked to switch to, by host, once.
    pub(crate) fn take_account_switch(&mut self) -> Option<String> {
        self.account_switch.take()
    }

    /// The account the list asked to log out, by host, once.
    pub(crate) fn take_account_logout(&mut self) -> Option<String> {
        self.account_logout.take()
    }

    /// The login just made, once.
    pub(crate) fn take_logged_in(&mut self) -> Option<Session> {
        self.logged_in.take()
    }

    /// Open the login form, and ask for the servers to offer in it.
    pub(crate) fn open_login(&mut self) -> Jobs {
        self.overlay = Some(Overlay::Login(Box::new(LoginForm::new())));
        vec![self.stamp(Job::Servers)]
    }

    /// Now acting as `session`, on its server: every list is loaded again.
    /// Answers to what was asked before are of the other account.
    pub(crate) fn switched_to(&mut self, session: Option<Session>, origin: &str) -> Jobs {
        self.session = session;
        self.origin = origin.to_string();
        self.account_since = self.sent + 1;
        self.overlay = None;
        self.tab = TABS[0];
        let jobs = self.start();
        if let Some(s) = &self.session {
            let shown = format!("now @{}@{}", s.username, s.host());
            self.info(shown);
        }
        jobs
    }

    /// The account on `host` was logged out; `next` is the one in use now,
    /// if any.
    pub(crate) fn logged_out(&mut self, host: &str, next: Option<Session>) -> Jobs {
        self.accounts.retain(|a| a.host != host);
        let was_current = self.session.as_ref().is_some_and(|s| s.host() == host);
        self.info(format!("logged out of {host}"));
        if !was_current {
            return Vec::new();
        }
        let origin = next
            .as_ref()
            .map_or_else(|| self.origin.clone(), |s| s.service.clone());
        let jobs = self.switched_to(next, &origin);
        if self.session.is_none() {
            self.info(format!("logged out of {host}; reading it as a visitor"));
        }
        jobs
    }

    /// A switch or a logout that could not be done.
    pub(crate) fn account_error(&mut self, why: String) {
        self.error(why);
    }

    /// Ask for the first page of timeline `kind`, unless it is an
    /// account's and there is none.
    fn load_timeline(&mut self, kind: TimelineKind) -> Jobs {
        if kind.needs_login() && self.session.is_none() {
            let host = self.host().to_string();
            self.timelines[kind.index()].needs_account(needs_login(&host, kind));
            return Vec::new();
        }
        let (seq, job) = self.stamp(Job::Timeline { kind, until: None });
        let list = &mut self.timelines[kind.index()];
        list.begin();
        list.asked = seq;
        vec![(seq, job)]
    }

    fn load_notifications(&mut self) -> Jobs {
        if self.session.is_none() {
            return Vec::new();
        }
        let (seq, job) = self.stamp(Job::Notifications { until: None });
        self.read_sent = false;
        self.notifications.begin();
        self.notifications.asked = seq;
        vec![(seq, job)]
    }

    /// Take the answer to job `seq`.
    pub(crate) fn handle_answer(&mut self, seq: u64, event: Event) -> Jobs {
        // A thread read ahead was not counted as pending.
        if let Event::ReadAhead { id, result } = event {
            return self.read_ahead_answer(seq, id, result);
        }
        // Nor was the unread count.
        if let Event::Unread(result) = event {
            if seq >= self.account_since
                && let Ok((count, any)) = result
                // Shown, the tab marks them read: the count is not news.
                && (self.tab != Tab::Notifications || !self.read_sent)
            {
                self.unread = count;
                self.unread_any = any;
            }
            return Vec::new();
        }
        self.pending = self.pending.saturating_sub(1);
        if seq < self.account_since {
            return Vec::new();
        }
        match event {
            Event::Timeline {
                kind,
                until: None,
                result,
            } => {
                let list = &mut self.timelines[kind.index()];
                if list.asked != seq {
                    return Vec::new();
                }
                match result {
                    Ok(notes) => list.set(notes),
                    Err(e) => {
                        let message = said(&e);
                        list.failed(message.clone());
                        if !list.items.is_empty() {
                            self.error(message);
                        }
                    }
                }
            }
            Event::Timeline {
                kind,
                until: Some(from),
                result,
            } => {
                let list = &mut self.timelines[kind.index()];
                match result {
                    Ok(notes) => list.append(&from, notes),
                    Err(e) => {
                        list.more_failed(&from);
                        self.error(said(&e));
                    }
                }
            }
            Event::Notifications {
                until: None,
                result,
            } => {
                if self.notifications.asked != seq {
                    return Vec::new();
                }
                match result {
                    Ok(items) => self.notifications.set(items),
                    Err(e) => self.notifications.failed(said(&e)),
                }
                return self.mark_read();
            }
            Event::Notifications {
                until: Some(from),
                result,
            } => match result {
                Ok(items) => self.notifications.append(&from, items),
                Err(e) => {
                    self.notifications.more_failed(&from);
                    self.error(said(&e));
                }
            },
            Event::Thread { id, result } => {
                let Some(th) = self
                    .threads
                    .iter_mut()
                    .find(|t| t.id == id && t.list.asked == seq)
                else {
                    return Vec::new();
                };
                match result {
                    Ok(t) => {
                        let (rows, at) = ThreadView::rows(*t);
                        let reload = th.list.loaded || !th.list.items.is_empty();
                        let keep = th.list.current().map(|r| r.note.id.clone());
                        th.list.set(rows);
                        // There is no next page of a thread.
                        th.list.cursor = None;
                        if !reload || keep.is_none() {
                            th.list.selected = at;
                        }
                    }
                    Err(e) => th.list.failed(said(&e)),
                }
            }
            Event::Downloaded(Ok(paths)) => match &paths[..] {
                [one] => self.info(format!("saved {}", one.display())),
                many => {
                    let dir = many
                        .first()
                        .and_then(|p| p.parent())
                        .map(|d| d.display().to_string())
                        .unwrap_or_default();
                    self.info(format!("saved {} files in {dir}", many.len()));
                }
            },
            Event::Downloaded(Err(e)) => self.error(said(&e)),
            Event::Posted(result) => self.posted(result),
            Event::Reacted {
                id,
                reaction,
                had,
                result,
            } => self.reacted(&id, reaction, had, result),
            Event::Renoted { id, on, result } => self.renoted(&id, on, result),
            Event::Deleted { id, result } => self.deleted(&id, result),
            e @ (Event::Profile(_)
            | Event::UserNotes { .. }
            | Event::SearchNotes { .. }
            | Event::SearchUsers { .. }) => self.browse_answer(seq, e),
            Event::Followed { user_id, result } => self.followed(&user_id, result),
            Event::Column { id, until, result } => self.column_page(seq, id, until, result),
            Event::ColumnChoices(result) => self.column_choices_answer(result),
            Event::Reported(result) => self.reported(result),
            // Taken before the rest, above.
            Event::ReadAhead { .. } => {}
            e @ (Event::ProfileEditor(_) | Event::ProfileSaved(_)) => {
                return self.edit_profile_answer(e);
            }
            e @ (Event::ChatHistory(_) | Event::ChatMessages { .. } | Event::ChatSent { .. }) => {
                self.chat_answer(seq, e)
            }
            Event::Emojis(Ok(list)) => self.emojis = list.into_iter().collect(),
            // Custom emoji are then shown by name, as before.
            Event::Emojis(Err(_)) => {}
            Event::MarkedRead(Ok(())) => {
                self.unread = Some(0);
                self.unread_any = false;
            }
            // Taken before the rest, above; a failure is not worth a
            // message.
            Event::Unread(_) => {}
            Event::MarkedRead(Err(e)) => self.error(said(&e)),
            Event::Opened(Ok(())) => {}
            // The address of a login is on screen to open by hand.
            Event::Opened(Err(_)) if matches!(&self.overlay, Some(Overlay::Login(f)) if f.waiting.is_some()) =>
                {}
            Event::Opened(Err(e)) => self.error(said(&e)),
            Event::Servers { list, failed } => {
                if let Some(Overlay::Login(f)) = &mut self.overlay {
                    let at = f.servers.get(f.selected).map(|s| s.host.clone());
                    f.loading = false;
                    if !list.is_empty() {
                        f.servers = list;
                    }
                    f.selected = at
                        .and_then(|h| f.servers.iter().position(|s| s.host == h))
                        .unwrap_or(0);
                    if let Some(e) = failed {
                        f.error =
                            Some(format!("{}; these are the servers msky knows", e.message()));
                    }
                }
            }
            Event::LoggedIn { session, result } => {
                let Some(Overlay::Login(f)) = &mut self.overlay else {
                    return Vec::new();
                };
                if f.waiting.as_ref().is_none_or(|w| w.session != session) {
                    return Vec::new();
                }
                f.waiting = None;
                match result {
                    Ok(s) => {
                        self.overlay = None;
                        self.logged_in = Some(s);
                    }
                    Err(e) => f.error = Some(said(&e)),
                }
            }
        }
        Vec::new()
    }

    /// Handle a key press. A key also closes an error on screen, and still
    /// does what it does, as in bsky.
    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> Jobs {
        if self.status.as_ref().is_some_and(|s| s.error) {
            self.status = None;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('c') {
            self.quit = true;
            return Vec::new();
        }
        if self.overlay.is_some() {
            return self.overlay_key(key);
        }
        if self.search.editing {
            return self.search_key(key);
        }
        if self.tab == Tab::Chat
            && self.threads.is_empty()
            && let Some(jobs) = self.chat_key(key)
        {
            return jobs;
        }
        self.main_key(key)
    }

    fn overlay_key(&mut self, key: KeyEvent) -> Jobs {
        let Some(overlay) = self.overlay.as_mut() else {
            return Vec::new();
        };
        match overlay {
            Overlay::Help { scroll } => match key.code {
                KeyCode::Esc | KeyCode::Char('q' | '?') => self.overlay = None,
                KeyCode::Char('j') | KeyCode::Down => *scroll = scroll.saturating_add(1),
                KeyCode::Char('k') | KeyCode::Up => *scroll = scroll.saturating_sub(1),
                KeyCode::PageDown | KeyCode::Char(' ') => *scroll = scroll.saturating_add(10),
                KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
                KeyCode::Char('g') | KeyCode::Home => *scroll = 0,
                KeyCode::Char('G') | KeyCode::End => *scroll = u16::MAX,
                // A stray key must not close the reference opened on purpose.
                _ => {}
            },
            Overlay::Viewer {
                files,
                index,
                replay,
            } => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => self.overlay = None,
                KeyCode::Char('r') => *replay += 1,
                KeyCode::Char('d') => {
                    let Some(file) = files.get(*index).cloned() else {
                        return Vec::new();
                    };
                    self.info("downloading…");
                    return vec![self.stamp(Job::Download {
                        files: vec![file],
                        dir: self.download_dir(),
                    })];
                }
                KeyCode::Char('o') => {
                    let Some(url) = files.get(*index).and_then(|f| f.2.url.clone()) else {
                        return Vec::new();
                    };
                    return vec![self.stamp(Job::OpenLink {
                        url,
                        browser: self.browser(),
                    })];
                }
                KeyCode::Right | KeyCode::Char('l' | 'j') => {
                    *index = (*index + 1).min(files.len().saturating_sub(1))
                }
                KeyCode::Left | KeyCode::Char('h' | 'k') => *index = index.saturating_sub(1),
                _ => {}
            },
            Overlay::Accounts { selected } => {
                let selected = *selected;
                return self.accounts_key(key, selected);
            }
            Overlay::Login(_) => return self.login_key(key),
            Overlay::Compose(_) => return self.compose_key(key),
            Overlay::Reactions { .. } => return self.reactions_key(key),
            Overlay::Settings { selected, edit } => {
                let (selected, edit) = (*selected, edit.clone());
                return self.settings_key(key, selected, edit);
            }
            Overlay::Themes { selected, previous } => {
                let (selected, previous) = (*selected, *previous);
                self.themes_key(key, selected, previous);
                return Vec::new();
            }
            Overlay::EditProfile(_) => return self.edit_profile_key(key),
            Overlay::AddColumn { .. } => return self.add_column_key(key),
            Overlay::Report(_) => return self.report_key(key),
            Overlay::Languages { selected } => {
                let selected = *selected;
                self.languages_key(key, selected);
                return Vec::new();
            }
            Overlay::Actions { selected } => {
                let selected = *selected;
                let entries = keys::actions(self);
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q' | '.') => self.overlay = None,
                    KeyCode::Char('j') | KeyCode::Down if !entries.is_empty() => {
                        self.overlay = Some(Overlay::Actions {
                            selected: (selected + 1) % entries.len(),
                        })
                    }
                    KeyCode::Char('k') | KeyCode::Up if !entries.is_empty() => {
                        self.overlay = Some(Overlay::Actions {
                            selected: (selected + entries.len() - 1) % entries.len(),
                        })
                    }
                    KeyCode::Enter => {
                        self.overlay = None;
                        if let Some((k, _)) = entries.get(selected) {
                            return self.note_action(keys::action_key(k));
                        }
                    }
                    // A key the list names does what it says, from the list too.
                    KeyCode::Char(c)
                        if entries
                            .iter()
                            .any(|(k, _)| keys::action_key(k).code == KeyCode::Char(c)) =>
                    {
                        self.overlay = None;
                        return self.note_action(key);
                    }
                    _ => {}
                }
            }
        }
        Vec::new()
    }

    fn accounts_key(&mut self, key: KeyEvent, selected: usize) -> Jobs {
        // The question x asked takes the next key, whatever it is: y logs
        // out, anything else calls it off.
        if let Some(host) = self.confirm_logout.take() {
            if key.code == KeyCode::Char('y') {
                self.account_logout = Some(host);
                self.overlay = None;
            } else {
                self.info("not logged out");
            }
            return Vec::new();
        }
        let n = self.accounts.len();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.overlay = None,
            KeyCode::Char('j') | KeyCode::Down if n > 0 => {
                self.overlay = Some(Overlay::Accounts {
                    selected: (selected + 1) % n,
                })
            }
            KeyCode::Char('k') | KeyCode::Up if n > 0 => {
                self.overlay = Some(Overlay::Accounts {
                    selected: (selected + n - 1) % n,
                })
            }
            KeyCode::Enter => {
                if let Some(a) = self.accounts.get(selected) {
                    let host = a.host.clone();
                    self.overlay = None;
                    if self.session.as_ref().is_some_and(|s| s.host() == host) {
                        self.info(format!("already using @{}@{host}", a.username));
                    } else {
                        self.account_switch = Some(host);
                    }
                }
            }
            KeyCode::Char('x') => {
                if let Some(a) = self.accounts.get(selected) {
                    let host = a.host.clone();
                    self.info(format!(
                        "press y to log @{}@{host} out, any other key to keep it",
                        a.username
                    ));
                    self.confirm_logout = Some(host);
                }
            }
            KeyCode::Char('a') => return self.open_login(),
            _ => {}
        }
        Vec::new()
    }

    fn login_key(&mut self, key: KeyEvent) -> Jobs {
        let Some(Overlay::Login(f)) = &mut self.overlay else {
            return Vec::new();
        };
        if let Some(w) = &f.waiting {
            match key.code {
                KeyCode::Esc => {
                    f.waiting = None;
                    f.error = Some("the login was called off".into());
                }
                KeyCode::Char('o') => {
                    let url = w.url.clone();
                    return vec![self.stamp(Job::OpenLink {
                        url,
                        browser: self.browser(),
                    })];
                }
                _ => {}
            }
            return Vec::new();
        }
        if let Some(typed) = &mut f.typed {
            match key.code {
                KeyCode::Esc => f.typed = None,
                KeyCode::Backspace => {
                    typed.pop();
                }
                KeyCode::Enter => {
                    let host = typed.trim().to_string();
                    if host.is_empty() {
                        return Vec::new();
                    }
                    return self.start_login(&host);
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => typed.push(c),
                _ => {}
            }
            return Vec::new();
        }
        let n = f.servers.len();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.overlay = None,
            KeyCode::Char('j') | KeyCode::Down if n > 0 => f.selected = (f.selected + 1) % n,
            KeyCode::Char('k') | KeyCode::Up if n > 0 => f.selected = (f.selected + n - 1) % n,
            KeyCode::Char('g') | KeyCode::Home => f.selected = 0,
            KeyCode::Char('G') | KeyCode::End => f.selected = n.saturating_sub(1),
            KeyCode::PageDown if n > 0 => f.selected = (f.selected + 10).min(n - 1),
            KeyCode::PageUp => f.selected = f.selected.saturating_sub(10),
            KeyCode::Char('/' | 'i') => f.typed = Some(String::new()),
            KeyCode::Enter => {
                if let Some(host) = f.servers.get(f.selected).map(|s| s.host.clone()) {
                    return self.start_login(&host);
                }
            }
            _ => {}
        }
        Vec::new()
    }

    /// Ask for access on the server `typed` names: open the page that
    /// allows it, and wait for the answer.
    fn start_login(&mut self, typed: &str) -> Jobs {
        let started = crate::api::normalize_host(typed)
            .and_then(|origin| crate::miauth::session().map(|s| (origin, s)));
        let Some(Overlay::Login(f)) = &mut self.overlay else {
            return Vec::new();
        };
        let (origin, session) = match started {
            Ok(x) => x,
            Err(e) => {
                f.error = Some(said(&e));
                return Vec::new();
            }
        };
        let url = crate::miauth::auth_url(&origin, &session);
        f.typed = None;
        f.error = None;
        f.waiting = Some(Waiting {
            origin: origin.clone(),
            session: session.clone(),
            url: url.clone(),
        });
        vec![
            self.stamp(Job::OpenLink {
                url,
                browser: self.browser(),
            }),
            self.stamp(Job::Login { origin, session }),
        ]
    }

    fn main_key(&mut self, key: KeyEvent) -> Jobs {
        // The question D asked takes the next key, whatever it is, as in
        // bsky: y deletes, anything else calls it off.
        if let Some(id) = self.confirm_delete.take() {
            return self.answer_delete(id, key);
        }
        if let Some(id) = self.confirm_block.take() {
            return self.answer_block(id, key);
        }
        if let Some(id) = self.confirm_remove_column.take() {
            return self.answer_remove_column(id, key);
        }
        let in_thread = !self.threads.is_empty();
        match key.code {
            KeyCode::Char('n') => self.compose(false, false),
            KeyCode::Char('r') => self.compose(true, false),
            KeyCode::Char('Q') => self.compose(false, true),
            KeyCode::Char('l') => return self.like(),
            KeyCode::Char('e')
                if self.tab == Tab::Profile
                    && !in_thread
                    && self.profile.own(self.my_user_id()) =>
            {
                return self.edit_profile();
            }
            KeyCode::Char('e') => self.open_reactions(),
            KeyCode::Char('b') => return self.toggle_renote(),
            KeyCode::Char('D') => self.ask_delete(),
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.overlay = Some(Overlay::Help { scroll: 0 }),
            KeyCode::Char('s') => self.open_settings(),
            KeyCode::Char('+')
                if matches!(self.tab, Tab::Timeline(TimelineKind::Home) | Tab::Columns)
                    && !in_thread =>
            {
                return self.open_add_column();
            }
            KeyCode::Char('x') if self.tab == Tab::Columns && !in_thread => {
                self.ask_remove_column();
            }
            KeyCode::Left | KeyCode::Char('H') if self.tab == Tab::Columns && !in_thread => {
                self.columns.move_focus(-1);
            }
            KeyCode::Right | KeyCode::Char('L') if self.tab == Tab::Columns && !in_thread => {
                self.columns.move_focus(1);
            }
            KeyCode::Char('<') if self.tab == Tab::Columns && !in_thread => {
                self.columns.move_focused(-1);
                self.save_columns();
            }
            KeyCode::Char('>') if self.tab == Tab::Columns && !in_thread => {
                self.columns.move_focused(1);
                self.save_columns();
            }
            KeyCode::Char('!') => self.open_report(),
            KeyCode::Char('T') => self.open_theme_picker(),
            KeyCode::Char('A') => {
                let at = self
                    .session
                    .as_ref()
                    .and_then(|s| self.accounts.iter().position(|a| a.host == s.host()))
                    .unwrap_or(0);
                self.overlay = Some(Overlay::Accounts { selected: at });
            }
            KeyCode::Char(c @ '1'..='9') => {
                let i = c as usize - '1' as usize;
                if let Some(tab) = self.tabs().get(i).copied() {
                    return self.switch_tab(tab);
                }
            }
            KeyCode::Char('/') if !in_thread => self.open_search(),
            KeyCode::Char('m') if self.tab == Tab::Profile && !in_thread => {
                return self.message_shown();
            }
            KeyCode::Char('f') => return self.toggle_follow(),
            KeyCode::Char('M') => return self.toggle_mute(),
            KeyCode::Char('B') => return self.toggle_block(),
            KeyCode::Enter => return self.open_selected_profile(),
            KeyCode::Esc
                if !in_thread && (self.search_shown() || self.search.pending.is_some()) =>
            {
                self.close_search();
            }
            KeyCode::Esc if !in_thread && self.tab == Tab::Profile => return self.profile_back(),
            KeyCode::Tab => return self.switch_tab(self.next_tab(1)),
            KeyCode::BackTab => return self.switch_tab(self.next_tab(-1)),
            KeyCode::Char('j') | KeyCode::Down => return self.step(1),
            KeyCode::Char('k') | KeyCode::Up => return self.step(-1),
            KeyCode::PageDown => return self.step(5),
            KeyCode::PageUp => return self.step(-5),
            KeyCode::Char('g') | KeyCode::Home => return self.step(isize::MIN / 2),
            KeyCode::Char('G') | KeyCode::End => return self.step(isize::MAX / 2),
            KeyCode::Esc if in_thread => {
                self.threads.pop();
            }
            KeyCode::Char('.') => {
                if !keys::actions(self).is_empty() {
                    self.overlay = Some(Overlay::Actions { selected: 0 });
                }
            }
            KeyCode::Char('v') if !in_thread && self.search_accounts() => {
                return self.open_selected_profile();
            }
            KeyCode::Char('v') => return self.open_thread(),
            KeyCode::Char(' ') => return self.open_viewer(),
            KeyCode::Char('o') => return self.open_in_browser(),
            KeyCode::Char('c') => self.copy_link(),
            KeyCode::Char('d') => return self.download(),
            KeyCode::Char('w') => self.toggle_warning(),
            KeyCode::Char('R') | KeyCode::F(5) => return self.refresh(),
            _ => {}
        }
        Vec::new()
    }

    fn switch_tab(&mut self, tab: Tab) -> Jobs {
        self.close_search();
        self.tab = tab;
        // Choosing a tab is a new place to be, not a detour to return from.
        self.threads.clear();
        self.profile.came_from = None;
        if tab == Tab::Chat {
            return self.show_chat();
        }
        if tab == Tab::Profile && self.profile.who.is_none() {
            return self.open_profile(crate::tui::worker::Who::Me);
        }
        if matches!(tab, Tab::Timeline(TimelineKind::Home) | Tab::Columns) {
            return self.settle_timeline();
        }
        if let Tab::Timeline(kind) = tab {
            self.last_timeline = tab;
            let list = &self.timelines[kind.index()];
            if !list.loaded && !list.loading {
                return self.load_timeline(kind);
            }
            return Vec::new();
        }
        if tab == Tab::Notifications && !self.notifications.loaded && !self.notifications.loading {
            return self.load_notifications();
        }
        self.mark_read()
    }

    /// Ask how many notifications are unread, at most once a minute.
    pub(crate) fn ask_unread(&mut self, now: Instant) -> Jobs {
        if self.session.is_none()
            || self
                .unread_polled
                .is_some_and(|t| now.saturating_duration_since(t) < UNREAD_POLL)
        {
            return Vec::new();
        }
        self.unread_polled = Some(now);
        // In the background: the screen does not wait for it.
        self.sent += 1;
        vec![(self.sent, Job::Unread)]
    }

    /// The label of the Notifications tab's count: `(3)`, `(•)` when the
    /// server only says there are some, or nothing.
    pub(crate) fn unread_label(&self) -> Option<String> {
        match self.unread {
            Some(0) => None,
            Some(n) if n > 99 => Some("(99+)".into()),
            Some(n) => Some(format!("({n})")),
            None if self.unread_any => Some("(•)".into()),
            None => None,
        }
    }

    /// Ask for the server's custom emoji, which notes and reactions are
    /// drawn with.
    pub(crate) fn load_emojis(&mut self) -> Jobs {
        vec![self.stamp(Job::Emojis)]
    }

    /// The tabs shown, in the order of their numbers: every one, but Chat
    /// on a server without it.
    pub(crate) fn tabs(&self) -> Vec<Tab> {
        TABS.into_iter()
            .filter(|t| !(self.no_chat && *t == Tab::Chat))
            .collect()
    }

    /// The tab of [`Self::tabs`] that is shown: the Home tab for its
    /// columns.
    pub(crate) fn shown_tab(&self) -> Tab {
        match self.tab {
            Tab::Columns => Tab::Timeline(TimelineKind::Home),
            t => t,
        }
    }

    /// The tab `delta` places away in the order of the tabs, wrapping.
    pub(super) fn next_tab(&self, delta: isize) -> Tab {
        let tabs = self.tabs();
        let n = tabs.len().max(1) as isize;
        let shown = self.shown_tab();
        let i = tabs.iter().position(|t| *t == shown).unwrap_or(0) as isize;
        tabs.get((i + delta).rem_euclid(n) as usize)
            .copied()
            .unwrap_or(self.tab)
    }

    /// The timeline the tab shown is, if it is one.
    #[cfg(test)]
    pub(crate) fn kind(&self) -> Option<TimelineKind> {
        match self.tab {
            Tab::Timeline(k) => Some(k),
            _ => None,
        }
    }

    /// `e` on a note: the reactions to choose from.
    fn open_reactions(&mut self) {
        if self.shown_note().is_some() {
            self.overlay = Some(Overlay::Reactions {
                selected: 0,
                query: String::new(),
            });
        }
    }

    /// A key the actions list runs on the note: `e` there is a reaction,
    /// on your own profile too, where the key alone edits the profile.
    fn note_action(&mut self, key: KeyEvent) -> Jobs {
        if key.code == KeyCode::Char('e') {
            self.open_reactions();
            return Vec::new();
        }
        self.main_key(key)
    }

    /// Move the selection, and ask for the next page when it nears the end.
    fn step(&mut self, delta: isize) -> Jobs {
        if let Some(th) = self.threads.last_mut() {
            th.list.step(delta);
            return Vec::new();
        }
        if self.search_shown() {
            return self.step_browse(delta);
        }
        match self.tab {
            Tab::Timeline(kind) => {
                let list = &mut self.timelines[kind.index()];
                list.step(delta);
                match list.want_more() {
                    Some(until) => vec![self.stamp(Job::Timeline {
                        kind,
                        until: Some(until),
                    })],
                    None => Vec::new(),
                }
            }
            Tab::Notifications => {
                self.notifications.step(delta);
                match self.notifications.want_more() {
                    Some(until) => vec![self.stamp(Job::Notifications { until: Some(until) })],
                    None => Vec::new(),
                }
            }
            Tab::Profile => self.step_browse(delta),
            // The tab's own keys move through it.
            Tab::Chat => Vec::new(),
            Tab::Columns => self.step_column(delta),
        }
    }

    /// R: the view on screen, loaded again.
    fn refresh(&mut self) -> Jobs {
        if let Some(th) = self.threads.last() {
            let id = th.id.clone();
            let (seq, job) = self.stamp(Job::Thread { id });
            if let Some(th) = self.threads.last_mut() {
                th.list.loading = true;
                th.list.asked = seq;
            }
            return vec![(seq, job)];
        }
        if self.search_shown() {
            return self.refresh_browse();
        }
        match self.tab {
            Tab::Timeline(kind) => self.load_timeline(kind),
            Tab::Notifications => self.load_notifications(),
            Tab::Profile => self.refresh_browse(),
            Tab::Chat => Vec::new(),
            Tab::Columns => self.refresh_column(),
        }
    }

    /// The note the selection is on, as it is posted: a renote's own note.
    pub(crate) fn selected_note(&self) -> Option<&Note> {
        if let Some(th) = self.threads.last() {
            return th.list.current().map(|r| &r.note);
        }
        if self.search_shown() {
            return match self.search.result_mode {
                SearchMode::Notes => self.search.notes.current(),
                SearchMode::Accounts => None,
            };
        }
        match self.tab {
            Tab::Timeline(kind) => self.timelines[kind.index()].current(),
            Tab::Notifications => self.notifications.current().and_then(|n| n.note.as_ref()),
            Tab::Profile => self.profile.notes.current(),
            Tab::Chat => None,
            Tab::Columns => self.column_note(),
        }
    }

    /// The note the selection shows: the renoted one for a renote.
    pub(crate) fn shown_note(&self) -> Option<&Note> {
        self.selected_note().map(Note::shown)
    }

    fn open_thread(&mut self) -> Jobs {
        self.search.pending = None;
        let Some(id) = self.shown_note().map(|n| n.id.clone()) else {
            return Vec::new();
        };
        let mut view = ThreadView {
            id: id.clone(),
            list: List::default(),
        };
        if !self.fill_from_read_ahead(&mut view) {
            self.threads.push(view);
            return Vec::new();
        }
        let (seq, job) = self.stamp(Job::Thread { id });
        // Shown as it was read ahead, if it was, while it is read again.
        if !view.list.loaded {
            view.list.begin();
        } else {
            view.list.loading = true;
        }
        view.list.asked = seq;
        self.threads.push(view);
        vec![(seq, job)]
    }

    /// The address of note `n` on the server read.
    fn note_url(&self, n: &Note) -> String {
        format!("{}/notes/{}", self.origin, n.id)
    }

    /// space: the note's files full screen; without pictures, the note in
    /// the web browser, as bsky opens a post on bsky.app.
    fn open_viewer(&mut self) -> Jobs {
        let Some(n) = self.shown_note() else {
            return Vec::new();
        };
        if !self.pictures {
            let url = self.note_url(n);
            return vec![self.stamp(Job::OpenLink {
                url,
                browser: self.browser(),
            })];
        }
        let files = attached(n);
        if files.is_empty() {
            self.info("the note has no files");
            return Vec::new();
        }
        self.overlay = Some(Overlay::Viewer {
            files,
            index: 0,
            replay: 0,
        });
        Vec::new()
    }

    /// o: the note in the web browser.
    fn open_in_browser(&mut self) -> Jobs {
        let Some(n) = self.shown_note() else {
            return Vec::new();
        };
        let url = self.note_url(n);
        vec![self.stamp(Job::OpenLink {
            url,
            browser: self.browser(),
        })]
    }

    /// c: the note's address, for the event loop to put in the clipboard.
    fn copy_link(&mut self) {
        let Some(n) = self.shown_note() else {
            return;
        };
        let url = self.note_url(n);
        self.info(format!("copied {url}"));
        self.to_copy = Some(url);
    }

    /// d: every file of the note, saved as `msky download` saves them.
    fn download(&mut self) -> Jobs {
        let Some(n) = self.shown_note() else {
            return Vec::new();
        };
        let files = attached(n);
        if files.is_empty() {
            self.info("the note has no files");
            return Vec::new();
        }
        self.info("downloading…");
        vec![self.stamp(Job::Download {
            files,
            dir: self.download_dir(),
        })]
    }

    /// w: show the text behind the note's content warning, or hide it
    /// again.
    fn toggle_warning(&mut self) {
        let Some(n) = self.shown_note() else {
            return;
        };
        if n.cw.is_none() {
            return;
        }
        let id = n.id.clone();
        if !self.opened.remove(&id) {
            self.opened.insert(id);
        }
    }

    /// Whether the text of `n` is shown: it has no warning, or `w` opened it.
    pub(crate) fn text_shown(&self, n: &Note) -> bool {
        n.cw.is_none() || self.opened.contains(&n.id)
    }
}

/// The files `n` carries, its own then those of the note it quotes, each
/// with the note it is attached to and its place there.
fn attached(n: &Note) -> Vec<Attached> {
    n.attached()
        .into_iter()
        .map(|(owner, i, f)| (owner.id.clone(), i, f.clone()))
        .collect()
}

/// An error as the status shows it: the message and, after it, the hint.
fn said(e: &Error) -> String {
    match e.hint() {
        Some(h) => format!("{}: {h}", e.message()),
        None => e.message().to_string(),
    }
}
