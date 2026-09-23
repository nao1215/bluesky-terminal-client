//! UI state and what every key does. Nothing here touches the terminal or
//! the network: keys and finished jobs go in, [`Job`]s come out.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::api::post_length_problem;
use crate::api::types::{Media, Post, Profile, ReplyRef, StrongRef};
use crate::config::{Environment, Session, Settings};
use crate::error::Error;
use crate::media::{self, MAX_POST_IMAGES};
use crate::tui::columns::{self, Columns, Rows};
use crate::tui::files::{Action, Browser};
use crate::tui::input::TextInput;
use crate::tui::keys;
use crate::tui::theme::{self, ColorDepth, THEMES, Theme};
use crate::tui::thread::{self as thread_rows, RowKind, ThreadRow};
use crate::tui::worker::{Attachment, Event, Feed, Job, MorePage, NotifItem, Page};

/// The top-level views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Timeline,
    Search,
    Profile,
    Notifications,
    Columns,
}

impl Tab {
    pub const ALL: [Tab; 5] = [
        Tab::Timeline,
        Tab::Search,
        Tab::Notifications,
        Tab::Profile,
        Tab::Columns,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Timeline => "Timeline",
            Tab::Search => "Search",
            Tab::Profile => "Profile",
            Tab::Notifications => "Notifications",
            Tab::Columns => "Columns",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap()
    }

    /// The tab `delta` places away, wrapping around.
    pub fn next(self, delta: isize) -> Tab {
        let n = Self::ALL.len() as isize;
        Self::ALL[(self.index() as isize + delta).rem_euclid(n) as usize]
    }
}

/// What the search box looks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Posts,
    Accounts,
}

/// Items that are the same item when their keys are equal: posts by URI,
/// accounts by DID. A further page can repeat what the last one ended with.
pub trait Keyed {
    fn key(&self) -> &str;
}

impl Keyed for Post {
    fn key(&self) -> &str {
        &self.uri
    }
}

impl Keyed for Profile {
    fn key(&self) -> &str {
        &self.did
    }
}

impl Keyed for NotifItem {
    fn key(&self) -> &str {
        &self.n.uri
    }
}

/// How close to the end the selection gets before the next page is fetched:
/// far enough that the page has usually arrived before the end is reached.
const MORE_AHEAD: usize = 10;

/// A scrollable list with a selection.
#[derive(Debug, Clone)]
pub struct List<T> {
    pub items: Vec<T>,
    pub selected: usize,
    /// First item drawn; kept by the view so the selection stays visible.
    pub offset: usize,
    /// Whether a result (possibly empty) has arrived.
    pub loaded: bool,
    /// Where the next page begins; `None` once there is nothing more.
    pub cursor: Option<String>,
    /// A next page has been asked for and not answered yet.
    pub more_pending: bool,
    /// A first page has been asked for and not answered yet.
    pub loading: bool,
    /// Why the last load failed; shown instead of "nothing here" while the
    /// list is empty.
    pub error: Option<String>,
}

// Not derived: a derive would demand `T: Default`, which an empty list does
// not need.
impl<T> Default for List<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selected: 0,
            offset: 0,
            loaded: false,
            cursor: None,
            more_pending: false,
            loading: false,
            error: None,
        }
    }
}

impl<T: Keyed> List<T> {
    /// Replace the list with a first page. The selection stays on the item
    /// it was on when that item is in the page (a reload after replying, or
    /// R, keeps the reader's place); otherwise it goes to the top.
    fn set(&mut self, page: Page<T>) {
        let kept = self
            .current()
            .and_then(|old| page.items.iter().position(|i| i.key() == old.key()));
        self.items = page.items;
        self.cursor = page.cursor;
        self.more_pending = false;
        self.selected = kept.unwrap_or(0);
        // The view scrolls the selection into sight from here.
        self.offset = self.offset.min(self.selected);
        self.loaded = true;
        self.loading = false;
        self.error = None;
    }

    /// The cursor to fetch from when the selection is near the end, a next
    /// page exists, and none is on its way. Marks the page as asked for.
    fn want_more(&mut self) -> Option<String> {
        if !self.loaded || self.more_pending || self.selected + MORE_AHEAD < self.items.len() {
            return None;
        }
        let cursor = self.cursor.clone()?;
        self.more_pending = true;
        Some(cursor)
    }

    /// Add a further page fetched from `requested`, keeping the selection
    /// and scroll position. A page that no longer continues this list (it
    /// was refreshed meanwhile) is dropped. Items already present are
    /// skipped, and the list ends when the server gives no cursor or the same
    /// one again, which is what stops a server that repeats its last page.
    fn append(&mut self, requested: &str, page: Page<T>) {
        if !self.more_pending || self.cursor.as_deref() != Some(requested) {
            return;
        }
        self.more_pending = false;
        let seen: HashSet<String> = self.items.iter().map(|i| i.key().to_string()).collect();
        self.items
            .extend(page.items.into_iter().filter(|i| !seen.contains(i.key())));
        self.cursor = page.cursor.filter(|c| c != requested);
    }
}

impl<T> List<T> {
    /// A first page has been asked for.
    fn begin(&mut self) {
        self.loaded = false;
        self.loading = true;
        self.error = None;
    }

    /// The first page could not be loaded; what was there stays.
    fn failed(&mut self, e: &Error) {
        self.loaded = true;
        self.loading = false;
        self.error = Some(e.message().to_string());
    }

    pub fn current(&self) -> Option<&T> {
        self.items.get(self.selected)
    }

    pub(crate) fn step(&mut self, delta: isize) {
        if self.items.is_empty() {
            return;
        }
        let last = self.items.len() as isize - 1;
        self.selected = (self.selected as isize + delta).clamp(0, last) as usize;
    }

    fn retain(&mut self, keep: impl FnMut(&T) -> bool) {
        self.items.retain(keep);
        self.selected = self.selected.min(self.items.len().saturating_sub(1));
        self.offset = self.offset.min(self.selected);
    }
}

/// State of the search tab.
#[derive(Debug, Clone)]
pub struct Search {
    pub input: TextInput,
    pub editing: bool,
    pub mode: SearchMode,
    pub posts: List<Post>,
    pub actors: List<Profile>,
    /// The query each result list is for (the box may have been edited
    /// since, and the other list may be for another query).
    pub posts_query: String,
    pub actors_query: String,
}

/// A pinned custom feed and what has been loaded of it.
#[derive(Debug, Clone, Default)]
pub struct CustomFeed {
    pub info: crate::api::types::FeedInfo,
    pub list: List<Post>,
}

/// State of the profile tab.
#[derive(Debug, Clone, Default)]
pub struct ProfilePane {
    /// The actor being shown; `None` is the logged-in account.
    pub actor: Option<String>,
    pub profile: Option<Profile>,
    pub posts: List<Post>,
    /// Why the profile could not be loaded, shown instead of "loading…".
    pub error: Option<String>,
    /// The tab the profile was opened from (Enter on an account or a post),
    /// which Esc goes back to with its results and selection as they were.
    pub came_from: Option<Tab>,
    /// The profile has been asked for and not answered yet.
    pub loading: bool,
}

/// A thread opened with `v`, shown over the current tab.
#[derive(Debug, Clone, Default)]
pub struct ThreadView {
    /// The post it was opened on.
    pub uri: String,
    pub list: List<ThreadRow>,
    pub error: Option<String>,
}

/// The post composer.
#[derive(Debug, Clone)]
pub struct Compose {
    pub input: TextInput,
    /// Reply target, the handle being answered, and an excerpt of the post.
    pub reply: Option<(ReplyRef, String, String)>,
    /// Quoted post, the handle being quoted, and an excerpt of the post.
    pub quote: Option<(StrongRef, String, String)>,
    pub sending: bool,
    /// Pictures (up to four) or one video to attach, in order.
    pub media: Vec<Attached>,
    /// What typing goes to: 0 is the post's text, `i + 1` the alt text of
    /// attachment `i`.
    pub focus: usize,
    /// The picture browser, open over the composer.
    pub browser: Option<Browser>,
}

impl Compose {
    fn new(reply: Option<(ReplyRef, String, String)>) -> Self {
        Self {
            input: TextInput::multi(""),
            reply,
            quote: None,
            sending: false,
            media: Vec::new(),
            focus: 0,
            browser: None,
        }
    }

    /// A new post that quotes `quote`.
    fn quoting(quote: (StrongRef, String, String)) -> Self {
        Self {
            quote: Some(quote),
            ..Self::new(None)
        }
    }

    /// The field typing goes to.
    fn field(&mut self) -> &mut TextInput {
        match self.focus {
            0 => &mut self.input,
            i => &mut self.media[i - 1].alt,
        }
    }
}

/// A picture or video attached in the composer.
#[derive(Debug, Clone)]
pub struct Attached {
    pub path: PathBuf,
    /// A description for people who cannot see it.
    pub alt: TextInput,
    /// What the file is, read when it was attached.
    pub info: media::Info,
}

impl Attached {
    pub fn new(path: PathBuf) -> Self {
        Self {
            info: media::inspect(&path),
            path,
            alt: TextInput::single(""),
        }
    }

    fn is_video(&self) -> bool {
        self.info.kind == media::Kind::Video
    }
}

/// Why `attached` cannot go on one post, if it cannot.
fn media_problem(attached: &[Attached]) -> Option<String> {
    let videos = attached.iter().filter(|a| a.is_video()).count();
    if videos > 0 && attached.len() > 1 {
        Some("a post can have up to 4 pictures or one video, not both".into())
    } else if attached.len() > MAX_POST_IMAGES {
        Some(format!(
            "a post can have at most {MAX_POST_IMAGES} pictures"
        ))
    } else {
        None
    }
}

/// The profile editor.
#[derive(Debug, Clone)]
pub struct EditProfile {
    /// Display name, description, avatar path.
    pub fields: [TextInput; 3],
    pub focus: usize,
    pub loading: bool,
    pub saving: bool,
    /// The picture browser, open to choose the avatar.
    pub browser: Option<Browser>,
    /// The avatar chosen in the browser. The field shows its name; this keeps
    /// the path itself, which may not be text that round-trips.
    pub avatar_chosen: Option<PathBuf>,
}

impl EditProfile {
    pub const LABELS: [&'static str; 3] = [
        "Display name",
        "Description",
        "New avatar (path, or ctrl+o to browse; optional)",
    ];
}

/// The login screen.
#[derive(Debug, Clone)]
pub struct LoginForm {
    /// Service URL, identifier, password (an app password or the account's).
    pub fields: [TextInput; 3],
    pub focus: usize,
    pub pending: bool,
    pub error: Option<String>,
    /// Logging in another account from the account list: Esc goes back to
    /// the account in use rather than quitting.
    pub adding: bool,
}

impl LoginForm {
    pub const LABELS: [&'static str; 3] = ["Service", "Handle or email", "Password"];

    fn new(service: &str) -> Self {
        Self {
            fields: [
                TextInput::single(service),
                TextInput::single(""),
                TextInput::password(),
            ],
            focus: 1,
            pending: false,
            error: None,
            adding: false,
        }
    }
}

/// A logged-in account, as the account list shows it. The tokens stay in
/// its file: the one in use is read again when it is switched to, since a
/// refresh may have rotated them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub did: String,
    pub handle: String,
}

impl From<&Session> for Account {
    fn from(s: &Session) -> Self {
        Self {
            did: s.did.clone(),
            handle: s.handle.clone(),
        }
    }
}

/// A window drawn over the current tab.
#[derive(Debug, Clone)]
pub enum Overlay {
    Compose(Compose),
    EditProfile(EditProfile),
    /// The key reference, scrolled `scroll` lines down (the view clamps it).
    Help {
        scroll: u16,
    },
    /// The theme picker: `selected` is previewed live, `previous` is what Esc
    /// goes back to.
    Themes {
        selected: usize,
        previous: usize,
    },
    /// What the keys of this view do to the selected post, as a list to
    /// choose from: the keys still work on their own, and this is the way
    /// that does not depend on remembering them.
    Actions {
        selected: usize,
        /// The post (or account) it was opened on: the list acts on that
        /// one only.
        about: Option<String>,
    },
    /// The logged-in accounts, opened with `A`.
    Accounts {
        selected: usize,
    },
    /// What a new column can show, opened with `+` on the Columns tab;
    /// `query` is the search being typed for a search column.
    AddColumn {
        selected: usize,
        query: Option<TextInput>,
    },
    /// The settings screen, opened with `s` on your own profile; `edit` is
    /// the setting being changed, when one is.
    Settings {
        selected: usize,
        edit: Option<SettingEdit>,
    },
    /// A post's pictures and video, full screen, `index` the one shown;
    /// `replay` counts `r` presses, each playing the video from the start.
    Viewer {
        media: Vec<Media>,
        index: usize,
        replay: u32,
    },
}

/// One row of the settings screen: what the setting is, what it is set to,
/// and where that comes from or what Enter does to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingRow {
    pub name: &'static str,
    pub value: String,
    pub note: String,
    /// Whether the screen can change it.
    pub editable: bool,
    /// Whether `x` puts it back to the default: its value is in the file.
    pub resettable: bool,
}

/// A setting being changed on the settings screen.
#[derive(Debug, Clone)]
pub enum SettingEdit {
    /// A folder, chosen in the folder browser.
    Folder(Box<Browser>),
    /// A line of text: a web address, or a program.
    Text(TextInput),
}

/// A one-line message in the status row. It is transient: it clears after
/// [`STATUS_TTL`] ([`ERROR_TTL`] for an error), and the key hints have a row
/// of their own, so a message never hides them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub text: String,
    pub error: bool,
    pub at: Instant,
}

/// How long a status message stays on screen.
pub const STATUS_TTL: Duration = Duration::from_secs(5);
/// How long an error stays on screen; longer, since it may explain a failure
/// the user did not see happen.
pub const ERROR_TTL: Duration = Duration::from_secs(10);

/// All UI state.
#[derive(Debug, Clone)]
pub struct App {
    pub session: Option<Session>,
    pub login: Option<LoginForm>,
    pub tab: Tab,
    /// The following timeline: the first feed on the Timeline tab.
    pub timeline: List<Post>,
    /// The custom feeds the account pinned, after the following timeline.
    pub feeds: Vec<CustomFeed>,
    /// Which feed the Timeline tab shows: 0 is the following timeline,
    /// `n` is `feeds[n - 1]`.
    pub feed: usize,
    pub search: Search,
    pub profile: ProfilePane,
    /// Threads opened with `v`, the last on top; Esc closes the top one.
    pub threads: Vec<ThreadView>,
    pub notifications: List<NotifItem>,
    /// Notifications that were unread when the list was loaded and have not
    /// been marked seen yet; shown on the tab.
    pub unread: usize,
    /// The time to mark notifications seen up to, once their tab is visited.
    pub seen_pending: Option<String>,
    pub overlay: Option<Overlay>,
    pub status: Option<Status>,
    /// Jobs sent and not yet answered.
    pub pending: usize,
    /// Likes and follows waiting for the server, keyed `like:<post uri>` and
    /// `follow:<did>`. A second press on the same target is refused until the
    /// first is answered, or two presses would create two records.
    pub in_flight: HashSet<String>,
    /// The post `D` has asked about, waiting for the `y` that deletes it.
    /// Deleting cannot be undone, so it takes a second key.
    pub confirm_delete: Option<String>,
    /// Text `c` has put up for the terminal's clipboard, which the event
    /// loop writes: the state machine has no terminal of its own.
    to_copy: Option<String>,
    /// The colors everything is drawn with: the chosen theme, adapted to what
    /// the terminal can show.
    pub theme: Theme,
    /// Index into [`THEMES`] of the chosen theme.
    pub theme_index: usize,
    pub color_depth: ColorDepth,
    /// The settings as loaded, so saving keeps what bsky did not change.
    pub settings: Settings,
    /// The folder the picture browser last showed, where it opens next.
    pub browse_from: Option<PathBuf>,
    /// Settings waiting to be written by the event loop.
    pub settings_to_save: Option<Settings>,
    /// Whether the settings file may be written: not when it was there but
    /// could not be read, since writing would lose what it holds.
    pub settings_writable: bool,
    pub quit: bool,
    /// Whether the terminal shows pictures and video. Without them the
    /// lists are text, and `Space` opens a post's media on bsky.app.
    pub pictures: bool,
    /// The variables that fix a setting for this run.
    pub env: Environment,
    /// Every logged-in account, by handle.
    pub accounts: Vec<Account>,
    /// The account the list asked to switch to, for the event loop, which
    /// reads its session and gives it to the worker before anything is
    /// loaded for it.
    account_switch: Option<String>,
    /// The account the list asked to log out, for the event loop.
    account_logout: Option<String>,
    /// The account `x` asked about, waiting for the `y` that logs it out.
    pub confirm_logout: Option<String>,
    /// The Columns tab of the account in use.
    pub columns: Columns,
    /// The column `x` asked about, waiting for the `y` that removes it.
    pub confirm_column_remove: Option<u64>,
    /// Pictures turned on (`true`) or off on the settings screen, for the
    /// event loop, which owns the terminal and the pictures, to act on.
    pictures_change: Option<bool>,
    /// The row of the settings screen the theme picker was opened from,
    /// to go back to when it closes.
    settings_return: Option<usize>,
    /// What to say once the settings are written.
    save_note: Option<String>,
    /// Jobs numbered so far by [`App::stamp`].
    sent: u64,
    /// The numbers of the reads sent and not yet answered.
    reads_out: BTreeSet<u64>,
    /// Likes, reposts, and follows confirmed while reads were out, with the
    /// number of the last job sent when each was confirmed.
    written: Vec<(u64, Written)>,
    /// The number of the newest first page applied to each list.
    first_pages: HashMap<String, u64>,
    /// Reads sent before this number belong to the account before a login.
    account_since: u64,
}

/// A write the server confirmed, as it shows on a post or an account.
#[derive(Debug, Clone)]
enum Written {
    Like { post: String, uri: Option<String> },
    Repost { post: String, uri: Option<String> },
    Follow { did: String, uri: Option<String> },
    Deleted { post: String },
}

impl Written {
    fn of(event: &Event) -> Option<Self> {
        Some(match event {
            Event::Liked {
                post_uri,
                result: Ok(uri),
            } => Written::Like {
                post: post_uri.clone(),
                uri: Some(uri.clone()),
            },
            Event::Unliked {
                post_uri,
                result: Ok(()),
            } => Written::Like {
                post: post_uri.clone(),
                uri: None,
            },
            Event::Reposted {
                post_uri,
                result: Ok(uri),
            } => Written::Repost {
                post: post_uri.clone(),
                uri: Some(uri.clone()),
            },
            Event::Unreposted {
                post_uri,
                result: Ok(()),
            } => Written::Repost {
                post: post_uri.clone(),
                uri: None,
            },
            Event::Followed {
                did,
                result: Ok(uri),
            } => Written::Follow {
                did: did.clone(),
                uri: Some(uri.clone()),
            },
            Event::Unfollowed {
                did,
                result: Ok(()),
            } => Written::Follow {
                did: did.clone(),
                uri: None,
            },
            Event::PostDeleted {
                uri,
                result: Ok(()),
            } => Written::Deleted { post: uri.clone() },
            _ => return None,
        })
    }
}

impl App {
    /// A new UI. Without a session it opens on the login screen; with one it
    /// asks for the timeline right away (the returned jobs).
    pub fn new(session: Option<Session>, service: &str) -> (Self, Vec<Job>) {
        let login = session.is_none().then(|| LoginForm::new(service));
        let mut app = Self {
            session,
            login,
            tab: Tab::Timeline,
            timeline: List::default(),
            feeds: Vec::new(),
            feed: 0,
            search: Search {
                input: TextInput::single(""),
                editing: false,
                mode: SearchMode::Posts,
                posts: List::default(),
                actors: List::default(),
                posts_query: String::new(),
                actors_query: String::new(),
            },
            profile: ProfilePane::default(),
            threads: Vec::new(),
            notifications: List::default(),
            unread: 0,
            seen_pending: None,
            overlay: None,
            status: None,
            pending: 0,
            in_flight: HashSet::new(),
            confirm_delete: None,
            to_copy: None,
            theme: THEMES[0],
            theme_index: 0,
            color_depth: ColorDepth::TrueColor,
            settings: Settings::default(),
            browse_from: None,
            settings_to_save: None,
            settings_writable: true,
            quit: false,
            pictures: true,
            env: Environment::default(),
            accounts: Vec::new(),
            account_switch: None,
            account_logout: None,
            confirm_logout: None,
            columns: Columns::default(),
            confirm_column_remove: None,
            pictures_change: None,
            settings_return: None,
            save_note: None,
            sent: 0,
            reads_out: BTreeSet::new(),
            written: Vec::new(),
            first_pages: HashMap::new(),
            account_since: 0,
        };
        let jobs = if app.session.is_some() {
            app.startup_jobs()
        } else {
            Vec::new()
        };
        app.pending += jobs.len();
        (app, jobs)
    }

    /// What a logged-in start loads: the timeline, then the notifications in
    /// the background, so their tab shows the unread count at once and opens
    /// without waiting. They are marked seen only when the tab is visited.
    fn startup_jobs(&mut self) -> Vec<Job> {
        self.notifications.begin();
        vec![Job::Timeline, Job::Notifications, Job::PinnedFeeds]
    }

    /// Take the saved settings and the terminal's color depth into account.
    /// A warning (a settings file that could not be used) is shown.
    pub fn apply_settings(
        &mut self,
        settings: Settings,
        depth: ColorDepth,
        warning: Option<String>,
    ) {
        self.color_depth = depth;
        self.settings_writable = warning.is_none();
        let mut warning = warning;
        let index = match settings.theme.as_deref() {
            Some(name) => theme::index_of(name).unwrap_or_else(|| {
                warning = Some(format!("unknown theme {name:?}; using {}", THEMES[0].name));
                0
            }),
            None => 0,
        };
        self.settings = settings;
        self.load_columns();
        self.set_theme(index);
        if let Some(w) = warning {
            self.error(w);
        }
    }

    fn set_theme(&mut self, index: usize) {
        self.theme_index = index;
        self.theme = THEMES[index].for_depth(self.color_depth);
    }

    /// Settings the user asked to save, for the event loop to write.
    pub fn take_settings_save(&mut self) -> Option<Settings> {
        self.settings_to_save.take()
    }

    /// The outcome of writing the settings.
    pub fn settings_saved(&mut self, result: crate::error::Result<()>) {
        let note = self.save_note.take();
        match result {
            Ok(()) => self
                .info(note.unwrap_or_else(|| format!("theme: {}", THEMES[self.theme_index].name))),
            Err(e) => self.error(e.message().to_string()),
        }
    }

    fn open_theme_picker(&mut self) {
        // Back to the list unless the settings screen, which opens it too,
        // says otherwise after this.
        self.settings_return = None;
        if self.color_depth == ColorDepth::None {
            self.error("colors are off because NO_COLOR is set");
            return;
        }
        self.overlay = Some(Overlay::Themes {
            selected: self.theme_index,
            previous: self.theme_index,
        });
    }

    fn info(&mut self, text: impl Into<String>) {
        self.status = Some(Status {
            text: text.into(),
            error: false,
            at: Instant::now(),
        });
    }

    fn error(&mut self, text: impl Into<String>) {
        self.status = Some(Status {
            text: text.into(),
            error: true,
            at: Instant::now(),
        });
    }

    /// Whether pictures or a video are shown full screen.
    pub fn viewer_open(&self) -> bool {
        matches!(self.overlay, Some(Overlay::Viewer { .. }))
    }

    /// Clear a status message older than [`STATUS_TTL`]. Returns whether the
    /// screen changed.
    pub fn expire_status(&mut self, now: Instant) -> bool {
        match &self.status {
            Some(s)
                if now.saturating_duration_since(s.at)
                    >= if s.error { ERROR_TTL } else { STATUS_TTL } =>
            {
                self.status = None;
                true
            }
            _ => false,
        }
    }

    /// Handle a key press. A key also closes an error on screen, and still
    /// does what it does, so the box never stands in the way.
    pub fn handle_key(&mut self, key: KeyEvent) -> Vec<Job> {
        if self.status.as_ref().is_some_and(|s| s.error) {
            self.status = None;
        }
        let jobs = self.key(key);
        self.pending += jobs.len();
        jobs
    }

    /// Handle pasted text: it goes to whichever field has focus.
    pub fn handle_paste(&mut self, text: &str) {
        if let Some(form) = &mut self.login {
            form.fields[form.focus].insert_str(text);
            return;
        }
        match &mut self.overlay {
            Some(Overlay::Compose(c)) if c.browser.is_none() => c.field().insert_str(text),
            Some(Overlay::EditProfile(e)) if e.browser.is_none() => {
                e.fields[e.focus].insert_str(text)
            }
            Some(Overlay::Compose(_) | Overlay::EditProfile(_)) => {}
            Some(Overlay::Settings {
                edit: Some(SettingEdit::Text(input)),
                ..
            }) => input.insert_str(text),
            Some(Overlay::AddColumn {
                query: Some(input), ..
            }) => input.insert_str(text),
            Some(
                Overlay::Help { .. }
                | Overlay::Themes { .. }
                | Overlay::Viewer { .. }
                | Overlay::Actions { .. }
                | Overlay::Accounts { .. }
                | Overlay::AddColumn { query: None, .. }
                | Overlay::Settings { .. },
            ) => {}
            None if self.tab == Tab::Search && self.search.editing => {
                self.search.input.insert_str(text)
            }
            None => {}
        }
    }

    /// Handle a finished job.
    #[cfg(test)]
    pub fn handle_event(&mut self, event: Event) -> Vec<Job> {
        // As if asked for just now: nothing sent since can be newer.
        self.answer(None, event)
    }

    /// Number a job as it goes to the worker. The answer comes back with
    /// the same number, which tells what was sent after it.
    pub fn stamp(&mut self, job: &Job) -> u64 {
        self.sent += 1;
        if job.reads() {
            self.reads_out.insert(self.sent);
        }
        self.sent
    }

    /// Handle the answer to the job sent as `seq`.
    pub fn handle_answer(&mut self, seq: u64, event: Event) -> Vec<Job> {
        self.answer(Some(seq), event)
    }

    /// Reads run beside each other and beside writes, so answers come in any
    /// order. A read's answer is dropped when it was asked for by an earlier
    /// account, or when a newer load of the same list has already answered;
    /// the likes, reposts, and follows confirmed since it was asked for are
    /// put back on what it brought.
    fn answer(&mut self, seq: Option<u64>, event: Event) -> Vec<Job> {
        self.pending = self.pending.saturating_sub(1);
        let read = seq.filter(|s| self.reads_out.remove(s));
        if let Some(seq) = read
            && self.superseded(seq, &event)
        {
            self.forget_writes();
            return Vec::new();
        }
        let written = Written::of(&event);
        let jobs = self.event(event);
        if let Some(w) = written
            && !self.reads_out.is_empty()
        {
            self.written.push((self.sent, w));
        }
        if let Some(seq) = read {
            let since: Vec<Written> = self
                .written
                .iter()
                .filter(|(at, _)| *at > seq)
                .map(|(_, w)| w.clone())
                .collect();
            since.iter().for_each(|w| self.apply(w));
        }
        self.forget_writes();
        self.pending += jobs.len();
        jobs
    }

    /// Whether the read sent as `seq` has been overtaken: by a login as
    /// another account, or by a newer first page of the same list.
    fn superseded(&mut self, seq: u64, event: &Event) -> bool {
        if seq < self.account_since {
            return true;
        }
        let list = match event {
            Event::Timeline(_) => "timeline".to_string(),
            Event::CustomFeed { uri, .. } => format!("feed {uri}"),
            Event::SearchPosts { .. } => "search posts".to_string(),
            Event::SearchActors { .. } => "search accounts".to_string(),
            Event::Profile(_) => "profile".to_string(),
            Event::Notifications { .. } => "notifications".to_string(),
            Event::Thread { uri, .. } => format!("thread {uri}"),
            _ => return false,
        };
        let newest = self.first_pages.entry(list).or_insert(0);
        if seq < *newest {
            return true;
        }
        *newest = seq;
        false
    }

    /// Writes older than every read still out can no longer be undone by one.
    fn forget_writes(&mut self) {
        match self.reads_out.first().copied() {
            Some(oldest) => self.written.retain(|(at, _)| *at > oldest),
            None => self.written.clear(),
        }
    }

    /// Show a confirmed write again on whatever a late read brought.
    fn apply(&mut self, w: &Written) {
        match w {
            Written::Like { post, uri } => self.set_like(post, uri.clone()),
            Written::Repost { post, uri } => self.set_repost(post, uri.clone()),
            Written::Follow { did, uri } => {
                self.set_following(did, uri.clone());
                if uri.is_none() {
                    // The timeline shows followed accounts only.
                    self.timeline.retain(|p| p.author.did != *did);
                }
            }
            // A page asked for before the delete still carries the post.
            Written::Deleted { post } => self.remove_post(post),
        }
    }

    /// Take a post out of every list it is in. A thread keeps its row, as
    /// the placeholder for a post that is not there any more, so the replies
    /// under it keep their place.
    fn remove_post(&mut self, uri: &str) {
        let feeds = self
            .feeds
            .iter_mut()
            .map(|f| &mut f.list)
            .chain(self.columns.post_lists());
        for list in [
            &mut self.timeline,
            &mut self.search.posts,
            &mut self.profile.posts,
        ]
        .into_iter()
        .chain(feeds)
        {
            list.retain(|p| p.uri != uri);
        }
        for th in &mut self.threads {
            for row in &mut th.list.items {
                if row.post().is_some_and(|p| p.uri == uri) {
                    row.kind = RowKind::NotFound(uri.to_string());
                }
            }
        }
    }

    /// Mark the post liked (`Some`, the like's URI) or not, counting the
    /// change only when its state changes: a reload may show it already.
    fn set_like(&mut self, post: &str, like: Option<String>) {
        self.each_post(post, |p| {
            let v = p.viewer.get_or_insert_with(Default::default);
            match (v.like.is_some(), like.is_some()) {
                (false, true) => p.like_count += 1,
                (true, false) => p.like_count = p.like_count.saturating_sub(1),
                _ => {}
            }
            v.like = like.clone();
        });
    }

    fn set_repost(&mut self, post: &str, repost: Option<String>) {
        self.each_post(post, |p| {
            let v = p.viewer.get_or_insert_with(Default::default);
            match (v.repost.is_some(), repost.is_some()) {
                (false, true) => p.repost_count += 1,
                (true, false) => p.repost_count = p.repost_count.saturating_sub(1),
                _ => {}
            }
            v.repost = repost.clone();
        });
    }

    fn key(&mut self, key: KeyEvent) -> Vec<Job> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('c') {
            self.quit = true;
            return Vec::new();
        }
        if self.login.is_some() {
            return self.login_key(key);
        }
        if self.overlay.is_some() {
            return self.overlay_key(key);
        }
        if self.tab == Tab::Search && self.search.editing {
            return self.search_key(key);
        }
        self.main_key(key)
    }

    fn login_key(&mut self, key: KeyEvent) -> Vec<Job> {
        let form = self.login.as_mut().unwrap();
        if form.pending {
            return Vec::new();
        }
        match key.code {
            // Adding an account goes back to the one in use.
            KeyCode::Esc if form.adding => self.login = None,
            KeyCode::Esc => self.quit = true,
            KeyCode::Tab | KeyCode::Down => form.focus = (form.focus + 1) % 3,
            KeyCode::BackTab | KeyCode::Up => form.focus = (form.focus + 2) % 3,
            KeyCode::Enter if form.focus < 2 => form.focus += 1,
            KeyCode::Enter => {
                let [service, id, pw] = form.fields.clone().map(|f| f.text());
                if service.trim().is_empty() || id.trim().is_empty() || pw.is_empty() {
                    form.error = Some("fill in all three fields".into());
                    form.focus = if service.trim().is_empty() {
                        0
                    } else if id.trim().is_empty() {
                        1
                    } else {
                        2
                    };
                    return Vec::new();
                }
                form.pending = true;
                form.error = None;
                return vec![Job::Login {
                    service,
                    identifier: id,
                    password: pw,
                }];
            }
            _ => {
                form.fields[form.focus].handle_key(key);
            }
        }
        Vec::new()
    }

    fn overlay_key(&mut self, key: KeyEvent) -> Vec<Job> {
        // Taken first: the entries are read from the whole app, which the
        // match below borrows.
        if let Some(Overlay::Actions { selected, about }) = &self.overlay {
            let (selected, about) = (*selected, about.clone());
            return self.actions_key(key, selected, about);
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        // Read before the overlay is borrowed: a post with a video needs it.
        let video_service = self.video_service();
        match self.overlay.as_mut().unwrap() {
            // Taken above, before this borrow.
            Overlay::Actions { .. } => {}
            Overlay::Settings { selected, .. } => {
                let selected = *selected;
                return self.settings_key(key, selected);
            }
            Overlay::Accounts { selected } => {
                let selected = *selected;
                self.accounts_key(key, selected);
            }
            Overlay::AddColumn { .. } => return self.add_column_key(key),
            Overlay::Help { scroll } => match key.code {
                KeyCode::Esc | KeyCode::Char('q' | '?') => self.overlay = None,
                KeyCode::Char('j') | KeyCode::Down => *scroll = scroll.saturating_add(1),
                KeyCode::Char('k') | KeyCode::Up => *scroll = scroll.saturating_sub(1),
                KeyCode::PageDown | KeyCode::Char(' ') => *scroll = scroll.saturating_add(10),
                KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
                KeyCode::Char('g') | KeyCode::Home => *scroll = 0,
                KeyCode::Char('G') | KeyCode::End => *scroll = u16::MAX,
                // Anything else is ignored: a stray key must not close the
                // reference the user opened on purpose.
                _ => {}
            },
            Overlay::Viewer {
                media,
                index,
                replay,
            } => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => self.overlay = None,
                KeyCode::Char('r') => *replay += 1,
                KeyCode::Char('d') => {
                    let item = media[*index].clone();
                    self.info("downloading…");
                    let dir = self.download_dir();
                    return vec![Job::Download { media: item, dir }];
                }
                KeyCode::Right | KeyCode::Char('l' | 'j') => {
                    *index = (*index + 1).min(media.len() - 1)
                }
                KeyCode::Left | KeyCode::Char('h' | 'k') => *index = index.saturating_sub(1),
                _ => {}
            },
            Overlay::Themes { selected, previous } => {
                let (selected, previous) = (*selected, *previous);
                let n = THEMES.len();
                let pick = match key.code {
                    KeyCode::Char('j') | KeyCode::Down => Some((selected + 1) % n),
                    KeyCode::Char('k') | KeyCode::Up => Some((selected + n - 1) % n),
                    KeyCode::PageDown => Some((selected + 10).min(n - 1)),
                    KeyCode::PageUp => Some(selected.saturating_sub(10)),
                    KeyCode::Char('g') | KeyCode::Home => Some(0),
                    KeyCode::Char('G') | KeyCode::End => Some(n - 1),
                    KeyCode::Enter => {
                        self.overlay =
                            self.settings_return
                                .take()
                                .map(|selected| Overlay::Settings {
                                    selected,
                                    edit: None,
                                });
                        self.settings.theme = Some(THEMES[selected].name.to_string());
                        if self.settings_writable {
                            self.settings_to_save = Some(self.settings.clone());
                        } else {
                            self.error(format!(
                                "theme: {} for this session only; settings.json could not be \
                                 read, so it is not overwritten (fix or remove it to save)",
                                THEMES[selected].name
                            ));
                        }
                        None
                    }
                    KeyCode::Esc | KeyCode::Char('q') => {
                        self.overlay =
                            self.settings_return
                                .take()
                                .map(|selected| Overlay::Settings {
                                    selected,
                                    edit: None,
                                });
                        self.set_theme(previous);
                        None
                    }
                    _ => None,
                };
                if let Some(i) = pick {
                    // Moving previews: the whole screen redraws in the theme.
                    self.set_theme(i);
                    self.overlay = Some(Overlay::Themes {
                        selected: i,
                        previous,
                    });
                }
            }
            Overlay::Compose(c) => {
                if c.sending {
                    return Vec::new();
                }
                if let Some(b) = &mut c.browser {
                    match b.key(key) {
                        Action::None => {}
                        Action::Close => {
                            self.browse_from = Some(b.dir.clone());
                            c.browser = None;
                        }
                        Action::Choose(paths) => {
                            self.browse_from = Some(b.dir.clone());
                            c.browser = None;
                            let mut all = c.media.clone();
                            all.extend(paths.into_iter().map(Attached::new));
                            match media_problem(&all) {
                                Some(why) => self.error(why),
                                None => c.media = all,
                            }
                        }
                    }
                    return Vec::new();
                }
                let fields = c.media.len() + 1;
                match key.code {
                    KeyCode::Esc => self.overlay = None,
                    KeyCode::Tab => c.focus = (c.focus + 1) % fields,
                    KeyCode::BackTab => c.focus = (c.focus + fields - 1) % fields,
                    KeyCode::Char('o') if ctrl => {
                        let room = MAX_POST_IMAGES.saturating_sub(c.media.len());
                        if c.media.iter().any(Attached::is_video) {
                            self.error("a post with a video can have nothing else attached");
                        } else if room == 0 {
                            self.error(format!(
                                "a post can have at most {MAX_POST_IMAGES} pictures"
                            ));
                        } else {
                            c.browser = Some(Browser::open(
                                &browse_start(&self.browse_from),
                                room,
                                c.media.is_empty(),
                            ));
                        }
                    }
                    // The picture whose alt text is being typed, or the last.
                    KeyCode::Char('x') if ctrl && !c.media.is_empty() => {
                        let i = c.focus.checked_sub(1).unwrap_or(c.media.len() - 1);
                        c.media.remove(i);
                        c.focus = c.focus.min(c.media.len());
                    }
                    KeyCode::Char('s') if ctrl => {
                        let text = c.input.text();
                        if text.trim().is_empty() && c.media.is_empty() {
                            self.error("the post is empty");
                        } else if let Some(why) = post_length_problem(text.trim_end()) {
                            self.error(why);
                        } else {
                            c.sending = true;
                            let reply = c.reply.as_ref().map(|(r, _, _)| r.clone());
                            let quote = c.quote.as_ref().map(|(r, _, _)| r.clone());
                            let media = c
                                .media
                                .iter()
                                .map(|a| Attachment {
                                    path: a.path.clone(),
                                    alt: a.alt.text(),
                                })
                                .collect();
                            return vec![Job::Post {
                                text,
                                reply,
                                quote,
                                media,
                                video_service,
                            }];
                        }
                    }
                    _ => {
                        c.field().handle_key(key);
                    }
                }
            }
            Overlay::EditProfile(e) => {
                if e.loading || e.saving {
                    if key.code == KeyCode::Esc && e.loading {
                        self.overlay = None;
                    }
                    return Vec::new();
                }
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
                            if let Some(p) = paths.first() {
                                e.fields[2] = TextInput::single(&p.display().to_string());
                                e.avatar_chosen = Some(p.clone());
                                e.focus = 2;
                            }
                        }
                    }
                    return Vec::new();
                }
                match key.code {
                    KeyCode::Esc => self.overlay = None,
                    KeyCode::Tab => e.focus = (e.focus + 1) % 3,
                    KeyCode::BackTab => e.focus = (e.focus + 2) % 3,
                    KeyCode::Char('o') if ctrl => {
                        e.browser = Some(Browser::open(&browse_start(&self.browse_from), 1, false));
                    }
                    KeyCode::Char('s') if ctrl => {
                        e.saving = true;
                        let [display_name, description, avatar_text] =
                            e.fields.clone().map(|f| f.text());
                        let avatar = match &e.avatar_chosen {
                            Some(p) if avatar_text == p.display().to_string() => Some(p.clone()),
                            _ if avatar_text.trim().is_empty() => None,
                            _ => Some(PathBuf::from(avatar_text.trim())),
                        };
                        return vec![Job::SaveProfile {
                            display_name,
                            description,
                            avatar,
                        }];
                    }
                    _ => {
                        e.fields[e.focus].handle_key(key);
                    }
                }
            }
        }
        Vec::new()
    }

    fn search_key(&mut self, key: KeyEvent) -> Vec<Job> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => self.search.editing = false,
            // Tabs switch even while typing, so the box never traps the user.
            KeyCode::Tab => {
                self.search.editing = false;
                return self.switch_tab(self.tab.next(1));
            }
            KeyCode::BackTab => {
                self.search.editing = false;
                return self.switch_tab(self.tab.next(-1));
            }
            KeyCode::Char('t') if ctrl => self.toggle_search_mode(),
            KeyCode::Enter => {
                self.search.editing = false;
                return self.run_search();
            }
            _ => {
                self.search.input.handle_key(key);
            }
        }
        Vec::new()
    }

    fn toggle_search_mode(&mut self) {
        self.search.mode = match self.search.mode {
            SearchMode::Posts => SearchMode::Accounts,
            SearchMode::Accounts => SearchMode::Posts,
        };
    }

    fn run_search(&mut self) -> Vec<Job> {
        let q = self.search.input.text().trim().to_string();
        if q.is_empty() {
            return Vec::new();
        }
        match self.search.mode {
            SearchMode::Posts => {
                self.search.posts_query = q.clone();
                self.search.posts.begin();
                vec![Job::SearchPosts(q)]
            }
            SearchMode::Accounts => {
                self.search.actors_query = q.clone();
                self.search.actors.begin();
                vec![Job::SearchActors(q)]
            }
        }
    }

    fn switch_tab(&mut self, tab: Tab) -> Vec<Job> {
        self.tab = tab;
        // Choosing a tab is a new place to be, not a detour to return from.
        self.profile.came_from = None;
        self.threads.clear();
        // Arriving at an empty Search tab means wanting to type: letters go to
        // the box, not to the commands they are bound to on the result list.
        // With a query already there, the results keep the keys (/ or i types).
        self.search.editing = tab == Tab::Search && self.search.input.is_empty();
        if tab == Tab::Profile
            && self.profile.profile.is_none()
            && self.profile.actor.is_none()
            && !self.profile.loading
        {
            return self.open_profile(None);
        }
        if tab == Tab::Notifications && !self.notifications.loaded && !self.notifications.loading {
            return self.load_notifications();
        }
        if tab == Tab::Columns {
            let waiting: Vec<u64> = self
                .columns
                .items
                .iter()
                .filter(|c| !c.asked())
                .map(|c| c.id)
                .collect();
            return waiting
                .into_iter()
                .flat_map(|id| self.load_column(id))
                .collect();
        }
        if tab == Tab::Notifications
            && let Some(at) = self.seen_pending.take()
        {
            return vec![Job::UpdateSeen(at)];
        }
        Vec::new()
    }

    fn load_notifications(&mut self) -> Vec<Job> {
        self.notifications.begin();
        vec![Job::Notifications]
    }

    fn open_profile(&mut self, actor: Option<String>) -> Vec<Job> {
        let Some(session) = &self.session else {
            return Vec::new();
        };
        let target = actor.clone().unwrap_or_else(|| session.did.clone());
        let own = target == session.did || target == session.handle;
        // Opened from another tab: remember it. Reloaded on the Profile tab
        // itself: keep what it was opened from.
        let came_from = if self.tab == Tab::Profile {
            self.profile.came_from
        } else {
            Some(self.tab)
        };
        self.tab = Tab::Profile;
        self.profile = ProfilePane {
            actor: if own { None } else { actor },
            came_from,
            loading: true,
            ..ProfilePane::default()
        };
        vec![Job::OpenProfile(target)]
    }

    /// The notifications shown, when the view is a list of them: the
    /// Notifications tab, or a column of notifications. The keys act on a
    /// notification the same way in both.
    pub fn shown_notifications(&self) -> Option<&List<NotifItem>> {
        if !self.threads.is_empty() {
            return None;
        }
        match self.tab {
            Tab::Notifications => Some(&self.notifications),
            Tab::Columns => match self.columns.focused().map(|c| &c.rows) {
                Some(Rows::Notifications(l)) => Some(l),
                _ => None,
            },
            _ => None,
        }
    }

    /// The post list of the current view, if it shows posts.
    pub fn current_posts(&mut self) -> Option<&mut List<Post>> {
        match self.tab {
            Tab::Timeline => Some(self.feed_list()),
            Tab::Search if self.search.mode == SearchMode::Posts => Some(&mut self.search.posts),
            Tab::Search => None,
            Tab::Profile => Some(&mut self.profile.posts),
            Tab::Notifications => None,
            Tab::Columns => match self.columns.focused_mut().map(|c| &mut c.rows) {
                Some(Rows::Posts(l)) => Some(l),
                _ => None,
            },
        }
    }

    pub(crate) fn selected_post(&mut self) -> Option<Post> {
        if let Some(th) = self.threads.last() {
            return th.list.current().and_then(ThreadRow::post).cloned();
        }
        if let Some(l) = self.shown_notifications() {
            // Only a reply, mention, or quote is a post to act on.
            return l.current()?.post.clone();
        }
        self.current_posts()?.current().cloned()
    }

    /// The account `f` and Enter act on in the current view.
    fn selected_account(&mut self) -> Option<Profile> {
        if !self.threads.is_empty() {
            return self.selected_post().map(|p| p.author);
        }
        if let Some(l) = self.shown_notifications() {
            return l.current().map(|i| i.n.author.clone());
        }
        match self.tab {
            Tab::Search if self.search.mode == SearchMode::Accounts => {
                self.search.actors.current().cloned()
            }
            Tab::Profile => self.profile.profile.clone(),
            _ => self.selected_post().map(|p| p.author),
        }
    }

    fn main_key(&mut self, key: KeyEvent) -> Vec<Job> {
        // The question D asked takes the next key, whatever it is: y
        // deletes, and anything else calls it off rather than doing what
        // that key usually does.
        if let Some(uri) = self.confirm_delete.take() {
            if key.code != KeyCode::Char('y') {
                self.info("not deleted");
                return Vec::new();
            }
            if !self.claim(format!("delete:{uri}")) {
                return Vec::new();
            }
            self.info("deleting…");
            return vec![Job::DeletePost { uri }];
        }
        // As D's: the next key answers x's question, whatever it is.
        if let Some(id) = self.confirm_column_remove.take() {
            if key.code == KeyCode::Char('y') && self.columns.focused().is_some_and(|c| c.id == id)
            {
                self.columns.remove_focused();
                self.save_columns();
            } else {
                self.info("the column stays");
            }
            return Vec::new();
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.overlay = Some(Overlay::Help { scroll: 0 }),
            KeyCode::Char('T') => self.open_theme_picker(),
            KeyCode::Char('A') => {
                let at = self.current_account_index().unwrap_or(0);
                self.overlay = Some(Overlay::Accounts { selected: at });
            }
            KeyCode::Char('1') => return self.switch_tab(Tab::Timeline),
            KeyCode::Char('2') => return self.switch_tab(Tab::Search),
            KeyCode::Char('3') => return self.switch_tab(Tab::Notifications),
            KeyCode::Char('4') => return self.switch_tab(Tab::Profile),
            KeyCode::Char('5') => return self.switch_tab(Tab::Columns),
            KeyCode::Left | KeyCode::Char('H')
                if self.tab == Tab::Columns && self.threads.is_empty() =>
            {
                self.columns.move_focus(-1);
            }
            KeyCode::Right | KeyCode::Char('L')
                if self.tab == Tab::Columns && self.threads.is_empty() =>
            {
                self.columns.move_focus(1);
            }
            KeyCode::Char('<') if self.tab == Tab::Columns && self.threads.is_empty() => {
                self.columns.move_focused(-1);
                self.save_columns();
            }
            KeyCode::Char('>') if self.tab == Tab::Columns && self.threads.is_empty() => {
                self.columns.move_focused(1);
                self.save_columns();
            }
            KeyCode::Char('+') if self.tab == Tab::Columns && self.threads.is_empty() => {
                self.overlay = Some(Overlay::AddColumn {
                    selected: 0,
                    query: None,
                });
            }
            KeyCode::Char('x') if self.tab == Tab::Columns && self.threads.is_empty() => {
                if let Some(c) = self.columns.focused() {
                    let (id, title) = (c.id, c.source.title());
                    self.info(format!(
                        "press y to remove the column {title}, any other key to keep it"
                    ));
                    self.confirm_column_remove = Some(id);
                }
            }
            KeyCode::Tab => return self.switch_tab(self.tab.next(1)),
            KeyCode::BackTab => return self.switch_tab(self.tab.next(-1)),
            KeyCode::Char('j') | KeyCode::Down => return self.step(1),
            KeyCode::Char('k') | KeyCode::Up => return self.step(-1),
            KeyCode::PageDown => return self.step(5),
            KeyCode::PageUp => return self.step(-5),
            KeyCode::Char('g') | KeyCode::Home => return self.step(isize::MIN / 2),
            KeyCode::Char('G') | KeyCode::End => return self.step(isize::MAX / 2),
            KeyCode::Char('/') => {
                let jobs = self.switch_tab(Tab::Search);
                self.search.editing = true;
                return jobs;
            }
            KeyCode::Char('i') if self.tab == Tab::Search && self.threads.is_empty() => {
                self.search.editing = true
            }
            KeyCode::Char('t') if self.tab == Tab::Search && self.threads.is_empty() => {
                self.toggle_search_mode();
                return self.run_search();
            }
            KeyCode::Char('n') => {
                self.overlay = Some(Overlay::Compose(Compose::new(None)));
            }
            KeyCode::Char('r') => return self.reply(),
            KeyCode::Char('l') => return self.toggle_like(),
            KeyCode::Char('b') => return self.toggle_repost(),
            KeyCode::Char('f') => return self.toggle_follow(),
            KeyCode::Char('e') if self.tab == Tab::Profile => return self.edit_profile(),
            KeyCode::Char('s')
                if self.tab == Tab::Profile
                    && self.profile.actor.is_none()
                    && self.threads.is_empty() =>
            {
                self.overlay = Some(Overlay::Settings {
                    selected: 0,
                    edit: None,
                });
            }
            KeyCode::Enter if !self.threads.is_empty() => {
                if let Some(author) = self.selected_post().map(|p| p.author) {
                    self.threads.clear();
                    return self.open_profile(Some(author.did));
                }
            }
            KeyCode::Enter => {
                if self.tab != Tab::Profile
                    && let Some(account) = self.selected_account()
                {
                    return self.open_profile(Some(account.did));
                }
            }
            KeyCode::Esc if !self.threads.is_empty() => {
                self.threads.pop();
            }
            KeyCode::Char('.') => self.open_actions(),
            KeyCode::Char('c') => self.copy_link(),
            KeyCode::Char('Q') => self.quote(),
            KeyCode::Char('D') => self.ask_delete(),
            KeyCode::Char('v') => return self.open_thread(),
            KeyCode::Char(' ') => return self.open_viewer(),
            KeyCode::Char('o') => return self.open_link(true),
            KeyCode::Esc if self.tab == Tab::Profile => return self.go_back(),
            KeyCode::Char('R') | KeyCode::F(5) => return self.refresh(),
            KeyCode::Char('[') if self.tab == Tab::Timeline && self.threads.is_empty() => {
                return self.switch_feed(-1);
            }
            KeyCode::Char(']') if self.tab == Tab::Timeline && self.threads.is_empty() => {
                return self.switch_feed(1);
            }
            _ => {}
        }
        Vec::new()
    }

    /// Move the selection, and ask for the next page when it nears the end.
    /// Esc on the Profile tab: back to the tab the profile was opened from,
    /// untouched; failing that, from someone else's profile to your own.
    fn go_back(&mut self) -> Vec<Job> {
        if let Some(tab) = self.profile.came_from.take() {
            self.tab = tab;
            // Back to the results, not to typing a new query.
            self.search.editing = false;
            // The next visit to the Profile tab shows your own profile.
            self.profile = ProfilePane::default();
            return Vec::new();
        }
        if self.profile.actor.is_some() {
            return self.open_profile(None);
        }
        Vec::new()
    }

    fn step(&mut self, delta: isize) -> Vec<Job> {
        if let Some(th) = self.threads.last_mut() {
            // A thread arrives whole (to the depth asked for): no pages.
            th.list.step(delta);
            return Vec::new();
        }
        let more = match self.tab {
            Tab::Search if self.search.mode == SearchMode::Accounts => {
                self.search.actors.step(delta);
                self.search
                    .actors
                    .want_more()
                    .map(|c| (Feed::SearchActors(self.search.actors_query.clone()), c))
            }
            Tab::Search => {
                self.search.posts.step(delta);
                self.search
                    .posts
                    .want_more()
                    .map(|c| (Feed::SearchPosts(self.search.posts_query.clone()), c))
            }
            Tab::Timeline => {
                let feed = self.current_feed();
                let list = self.feed_list();
                list.step(delta);
                list.want_more().map(|c| (feed, c))
            }
            Tab::Notifications => {
                self.notifications.step(delta);
                self.notifications
                    .want_more()
                    .map(|c| (Feed::Notifications, c))
            }
            Tab::Columns => return self.step_column(delta),
            Tab::Profile => {
                self.profile.posts.step(delta);
                let did = self.profile.profile.as_ref().map(|p| p.did.clone());
                match did {
                    Some(did) => self
                        .profile
                        .posts
                        .want_more()
                        .map(|c| (Feed::Author(did), c)),
                    None => None,
                }
            }
        };
        more.map(|(feed, cursor)| Job::More { feed, cursor })
            .into_iter()
            .collect()
    }

    /// The post Space and `o` act on: the selected one, or on the
    /// Notifications tab the post a notification is about.
    fn post_to_view(&mut self) -> Option<Post> {
        match self.shown_notifications() {
            Some(l) => l
                .current()
                .and_then(|i| i.post.clone().or_else(|| i.subject.clone())),
            None => self.selected_post(),
        }
    }

    /// Run as text, for a terminal that cannot show pictures or video.
    pub fn without_pictures(&mut self) {
        self.pictures = false;
        if self.status.is_none() {
            self.info("this terminal cannot show pictures; bsky runs without them");
        }
    }

    /// Where the account in use is in the account list.
    fn current_account_index(&self) -> Option<usize> {
        let did = &self.session.as_ref()?.did;
        self.accounts.iter().position(|a| a.did == *did)
    }

    /// Put `session`'s account in the list, or bring its handle up to date.
    fn remember_account(&mut self, session: &Session) {
        let account = Account::from(session);
        match self.accounts.iter_mut().find(|a| a.did == account.did) {
            Some(a) => *a = account,
            None => self.accounts.push(account),
        }
        self.accounts.sort_by(|a, b| {
            (a.handle.to_lowercase(), &a.did).cmp(&(b.handle.to_lowercase(), &b.did))
        });
    }

    /// A key on the account list.
    fn accounts_key(&mut self, key: KeyEvent, selected: usize) {
        let n = self.accounts.len().max(1);
        let selected = selected.min(n - 1);
        // The question x asked takes the next key, as D's does.
        if let Some(did) = self.confirm_logout.take() {
            if key.code == KeyCode::Char('y') {
                self.overlay = None;
                self.info("logging out…");
                self.account_logout = Some(did);
            } else {
                self.info("still logged in");
            }
            return;
        }
        let at = |selected| Some(Overlay::Accounts { selected });
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.overlay = at((selected + 1) % n),
            KeyCode::Char('k') | KeyCode::Up => self.overlay = at((selected + n - 1) % n),
            KeyCode::Esc | KeyCode::Char('q' | 'A') => self.overlay = None,
            KeyCode::Enter => {
                self.overlay = None;
                if let Some(a) = self.accounts.get(selected).cloned()
                    && Some(selected) != self.current_account_index()
                {
                    self.info(format!("switching to @{}…", a.handle));
                    self.account_switch = Some(a.did);
                }
            }
            KeyCode::Char('a') => {
                self.overlay = None;
                let service = self
                    .session
                    .as_ref()
                    .map(|s| s.service.clone())
                    .unwrap_or_else(|| crate::api::DEFAULT_SERVICE.to_string());
                let mut form = LoginForm::new(&service);
                form.adding = true;
                self.login = Some(form);
            }
            KeyCode::Char('x') => {
                if let Some(a) = self.accounts.get(selected).cloned() {
                    self.info(format!(
                        "press y to log out @{}, any other key to stay logged in",
                        a.handle
                    ));
                    self.confirm_logout = Some(a.did);
                }
            }
            _ => {}
        }
    }

    /// Why an account could not be switched to or logged out.
    pub fn account_error(&mut self, why: String) {
        self.error(why);
    }

    /// The columns `settings.json` keeps for the account in use.
    fn load_columns(&mut self) {
        let did = self
            .session
            .as_ref()
            .map(|s| s.did.clone())
            .unwrap_or_default();
        let sources = self.settings.columns.get(&did).cloned().unwrap_or_default();
        self.columns = Columns::from_sources(&sources);
    }

    /// Keep the columns of the account in use in `settings.json`.
    fn save_columns(&mut self) {
        let Some(did) = self.session.as_ref().map(|s| s.did.clone()) else {
            return;
        };
        let sources = self.columns.sources();
        if sources.is_empty() {
            self.settings.columns.remove(&did);
        } else {
            self.settings.columns.insert(did, sources);
        }
        let n = self.columns.items.len();
        self.save_settings(format!("{n} column{}", if n == 1 { "" } else { "s" }));
    }

    /// Ask for the first page of the column `id`, dropping any answer to an
    /// earlier ask.
    fn load_column(&mut self, id: u64) -> Vec<Job> {
        let Some(c) = self.columns.by_id(id) else {
            return Vec::new();
        };
        c.generation += 1;
        match &mut c.rows {
            Rows::Posts(l) => l.begin(),
            Rows::Notifications(l) => l.begin(),
        }
        vec![Job::Column {
            id,
            generation: c.generation,
            feed: columns::feed_of(&c.source),
            cursor: None,
        }]
    }

    /// Move in the focused column, and ask for its next page near the end.
    fn step_column(&mut self, delta: isize) -> Vec<Job> {
        let Some(c) = self.columns.focused_mut() else {
            return Vec::new();
        };
        let cursor = match &mut c.rows {
            Rows::Posts(l) => {
                l.step(delta);
                l.want_more()
            }
            Rows::Notifications(l) => {
                l.step(delta);
                l.want_more()
            }
        };
        cursor
            .map(|cursor| Job::Column {
                id: c.id,
                generation: c.generation,
                feed: columns::feed_of(&c.source),
                cursor: Some(cursor),
            })
            .into_iter()
            .collect()
    }

    /// A page for the column `id`. It lands only in that column, and a first
    /// page only for the column's latest load; a removed column's is dropped.
    fn column_page(
        &mut self,
        id: u64,
        generation: u64,
        cursor: Option<String>,
        result: crate::error::Result<MorePage>,
    ) {
        let Some(c) = self.columns.by_id(id) else {
            return;
        };
        if cursor.is_none() && generation != c.generation {
            return;
        }
        let failed = match (&mut c.rows, &cursor, result) {
            (Rows::Posts(l), None, Ok(MorePage::Posts(page))) => {
                l.set(page);
                None
            }
            (Rows::Posts(l), Some(at), Ok(MorePage::Posts(page))) => {
                l.append(at, page);
                None
            }
            (Rows::Notifications(l), None, Ok(MorePage::Notifications(page))) => {
                l.set(page);
                None
            }
            (Rows::Notifications(l), Some(at), Ok(MorePage::Notifications(page))) => {
                l.append(at, page);
                None
            }
            (rows, _, Err(e)) => {
                match (rows, &cursor) {
                    (Rows::Posts(l), None) => l.failed(&e),
                    (Rows::Notifications(l), None) => l.failed(&e),
                    (Rows::Posts(l), Some(_)) => l.more_pending = false,
                    (Rows::Notifications(l), Some(_)) => l.more_pending = false,
                }
                Some(e)
            }
            _ => None,
        };
        if let Some(e) = failed {
            self.fail(&e);
        }
    }

    /// What `+` offers: the sources a column can have, the search last.
    pub fn column_choices(&self) -> Vec<columns::Source> {
        let mut v = vec![columns::Source::Following];
        v.extend(self.feeds.iter().map(|f| columns::Source::Feed {
            uri: f.info.uri.clone(),
            name: f.info.name.clone(),
        }));
        v.push(columns::Source::Notifications);
        if let Some(s) = &self.session {
            v.push(columns::Source::Author {
                did: s.did.clone(),
                handle: s.handle.clone(),
            });
        }
        v.push(columns::Source::Search {
            query: String::new(),
        });
        v
    }

    /// A key on the list `+` opened, or on the search being typed there.
    fn add_column_key(&mut self, key: KeyEvent) -> Vec<Job> {
        let choices = self.column_choices();
        let Some(Overlay::AddColumn { selected, query }) = &mut self.overlay else {
            return Vec::new();
        };
        if let Some(input) = query {
            match key.code {
                KeyCode::Esc => *query = None,
                KeyCode::Enter => {
                    let q = input.text().trim().to_string();
                    if q.is_empty() {
                        return Vec::new();
                    }
                    return self.add_column(columns::Source::Search { query: q });
                }
                _ => {
                    input.handle_key(key);
                }
            }
            return Vec::new();
        }
        let n = choices.len();
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => *selected = (*selected + 1) % n,
            KeyCode::Char('k') | KeyCode::Up => *selected = (*selected + n - 1) % n,
            KeyCode::Esc | KeyCode::Char('q' | '+') => self.overlay = None,
            KeyCode::Enter => {
                let chosen = choices[(*selected).min(n - 1)].clone();
                if matches!(chosen, columns::Source::Search { .. }) {
                    *query = Some(TextInput::single(""));
                    return Vec::new();
                }
                return self.add_column(chosen);
            }
            _ => {}
        }
        Vec::new()
    }

    /// Add a column of `source`, focus it, keep it, and load it.
    fn add_column(&mut self, source: columns::Source) -> Vec<Job> {
        self.overlay = None;
        let id = self.columns.add(source);
        self.save_columns();
        self.load_column(id)
    }

    /// The account the list asked to switch to, for the event loop.
    pub fn take_account_switch(&mut self) -> Option<String> {
        self.account_switch.take()
    }

    /// The account the list asked to log out, for the event loop.
    pub fn take_account_logout(&mut self) -> Option<String> {
        self.account_logout.take()
    }

    /// Use `session`'s account from now on: what was loaded for the one
    /// before goes, as it does when another account logs in, and the new
    /// one's lists are asked for. The event loop has given the worker the
    /// session already, so nothing is asked of the account before.
    pub fn switched_to(&mut self, session: Session) -> Vec<Job> {
        let other = self.session.as_ref().is_none_or(|s| s.did != session.did);
        self.remember_account(&session);
        self.info(format!("now @{}", session.handle));
        self.login = None;
        if !other {
            self.session = Some(session);
            return Vec::new();
        }
        self.forget_account();
        self.account_since = self.sent + 1;
        self.session = Some(session);
        self.load_columns();
        let jobs = self.startup_jobs();
        self.pending += jobs.len();
        jobs
    }

    /// The account `did` was logged out; `next` is the one in use now, if
    /// any is left. Without one, the login form comes back.
    pub fn logged_out(&mut self, did: &str, next: Option<Session>) -> Vec<Job> {
        let handle = self
            .accounts
            .iter()
            .find(|a| a.did == did)
            .map(|a| a.handle.clone())
            .unwrap_or_default();
        self.accounts.retain(|a| a.did != did);
        let was_current = self.session.as_ref().is_some_and(|s| s.did == did);
        if !was_current {
            self.info(format!("logged out @{handle}"));
            return Vec::new();
        }
        match next {
            Some(s) => {
                let jobs = self.switched_to(s);
                let now = self
                    .session
                    .as_ref()
                    .map(|s| s.handle.clone())
                    .unwrap_or_default();
                self.info(format!("logged out @{handle}; now @{now}"));
                jobs
            }
            None => {
                let service = self.session.take().map(|s| s.service).unwrap_or_default();
                self.forget_account();
                self.account_since = self.sent + 1;
                self.login = Some(LoginForm::new(&service));
                Vec::new()
            }
        }
    }

    /// The answer to pictures turned back on: whether the terminal draws
    /// them after all.
    pub fn pictures_back(&mut self, shown: bool) {
        self.pictures = shown;
        if !shown {
            self.info("this terminal cannot show pictures; bsky runs without them");
        }
    }

    /// Pictures turned on or off on the settings screen, for the event loop.
    pub fn take_pictures_change(&mut self) -> Option<bool> {
        self.pictures_change.take()
    }

    /// Where `d` saves, as the environment and the settings decide.
    pub fn download_dir(&self) -> Option<PathBuf> {
        crate::config::download_dir(&self.env, &self.settings).0
    }

    /// Where pictures are cached, or `None` for no cache.
    pub fn cache_dir(&self) -> Option<PathBuf> {
        crate::config::cache_dir(&self.env, &self.settings).0
    }

    /// The video service uploads go to.
    fn video_service(&self) -> String {
        crate::config::video_service(&self.env, &self.settings).0
    }

    /// The program that opens links, or `None` for the system's.
    fn browser(&self) -> Option<String> {
        crate::config::browser(&self.env, &self.settings).0
    }

    /// A job that opens `url` in the browser the settings name.
    fn open_url(&self, url: String) -> Job {
        Job::OpenLink {
            url,
            browser: self.browser(),
        }
    }

    /// What the settings screen lists, in order.
    pub fn settings_rows(&self) -> Vec<SettingRow> {
        use crate::config::{self, Source};
        let fixed = |var: &str| format!("set by {var} for this run");
        let path = |p: Option<PathBuf>, none: &str| {
            p.map_or_else(|| none.to_string(), |p| p.display().to_string())
        };
        let theme = if self.color_depth == ColorDepth::None {
            SettingRow {
                name: "Theme",
                value: THEMES[self.theme_index].name.to_string(),
                note: "colors are off because NO_COLOR is set".into(),
                editable: false,
                resettable: false,
            }
        } else {
            SettingRow {
                name: "Theme",
                value: THEMES[self.theme_index].name.to_string(),
                note: "enter chooses one from the list, as T does".into(),
                editable: true,
                resettable: false,
            }
        };
        let pictures = match &self.env.graphics {
            Some(v) => SettingRow {
                name: "Pictures",
                value: v.clone(),
                note: fixed(crate::terminal::GRAPHICS_ENV),
                editable: false,
                resettable: false,
            },
            None if self.settings.pictures_off() => SettingRow {
                name: "Pictures",
                value: "off".into(),
                note: "enter draws them again where the terminal can".into(),
                editable: true,
                resettable: false,
            },
            None => SettingRow {
                name: "Pictures",
                value: "auto".into(),
                note: if self.pictures {
                    "enter turns them off: posts say what they carry".into()
                } else {
                    "this terminal cannot show them".into()
                },
                editable: true,
                resettable: false,
            },
        };
        // A row whose value may come from a variable, the file, or bsky.
        let row = |name, var: &'static str, value: String, from: Source, change: &str| {
            let note = match from {
                Source::Env => fixed(var),
                Source::File => format!("{change}; x goes back to the default"),
                Source::Default => format!("the default; {change}"),
            };
            SettingRow {
                name,
                value,
                note,
                editable: from != Source::Env,
                resettable: from == Source::File,
            }
        };
        let (download, download_from) = config::download_dir(&self.env, &self.settings);
        let (cache, cache_from) = config::cache_dir(&self.env, &self.settings);
        let (video, video_from) = config::video_service(&self.env, &self.settings);
        let (browser, browser_from) = config::browser(&self.env, &self.settings);
        vec![
            theme,
            pictures,
            row(
                "Download folder",
                config::DOWNLOAD_DIR_ENV,
                path(download, "none"),
                download_from,
                "enter chooses another folder",
            ),
            row(
                "Picture cache",
                config::CACHE_DIR_ENV,
                path(cache, "off"),
                cache_from,
                "enter chooses another folder",
            ),
            row(
                "Video service",
                config::VIDEO_SERVICE_ENV,
                video,
                video_from,
                "enter types another address",
            ),
            row(
                "Browser",
                crate::browser::BROWSER_ENV,
                browser.unwrap_or_else(|| crate::browser::system_opener().to_string()),
                browser_from,
                "enter types the program that opens links",
            ),
        ]
    }

    /// A key on the settings screen, or on the folder browser or the line
    /// of text it opened.
    fn settings_key(&mut self, key: KeyEvent, selected: usize) -> Vec<Job> {
        if let Some(Overlay::Settings {
            edit: Some(edit), ..
        }) = &mut self.overlay
        {
            let chosen = match edit {
                SettingEdit::Folder(b) => match b.key(key) {
                    Action::None => return Vec::new(),
                    Action::Close => None,
                    Action::Choose(mut dirs) => Some(dirs.pop().map(|d| d.display().to_string())),
                },
                SettingEdit::Text(input) => match key.code {
                    KeyCode::Esc => None,
                    KeyCode::Enter => Some(Some(input.text())),
                    _ => {
                        input.handle_key(key);
                        return Vec::new();
                    }
                },
            };
            match chosen {
                Some(value) => self.set_setting(selected, value.unwrap_or_default()),
                None => self.close_edit(selected),
            }
            return Vec::new();
        }
        let n = self.settings_rows().len();
        let moved = |selected| {
            Some(Overlay::Settings {
                selected,
                edit: None,
            })
        };
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.overlay = moved((selected + 1) % n),
            KeyCode::Char('k') | KeyCode::Up => self.overlay = moved((selected + n - 1) % n),
            KeyCode::Esc | KeyCode::Char('q' | 's') => self.overlay = None,
            KeyCode::Enter | KeyCode::Char(' ') => self.change_setting(selected),
            KeyCode::Char('x') => self.reset_setting(selected),
            _ => {}
        }
        Vec::new()
    }

    /// Back to the list, on the row that was being changed.
    fn close_edit(&mut self, selected: usize) {
        self.overlay = Some(Overlay::Settings {
            selected,
            edit: None,
        });
    }

    /// Enter on a row of the settings screen.
    fn change_setting(&mut self, selected: usize) {
        let Some(row) = self.settings_rows().into_iter().nth(selected) else {
            return;
        };
        if !row.editable {
            self.info(row.note);
            return;
        }
        let edit = match row.name {
            "Theme" => {
                self.open_theme_picker();
                self.settings_return = Some(selected);
                return;
            }
            "Pictures" => {
                let off = !self.settings.pictures_off();
                let value = if off { "off" } else { "auto" };
                self.settings.pictures = Some(value.into());
                self.pictures_change = Some(!off);
                if off {
                    self.pictures = false;
                }
                self.save_settings(format!("pictures: {value}"));
                return;
            }
            "Download folder" => SettingEdit::Folder(Box::new(Browser::folder(
                &self.folder_start(self.download_dir()),
            ))),
            "Picture cache" => SettingEdit::Folder(Box::new(Browser::folder(
                &self.folder_start(self.cache_dir()),
            ))),
            "Video service" => SettingEdit::Text(TextInput::single(&self.video_service())),
            "Browser" => SettingEdit::Text(TextInput::single(&self.browser().unwrap_or_default())),
            _ => return,
        };
        self.overlay = Some(Overlay::Settings {
            selected,
            edit: Some(edit),
        });
    }

    /// Where the folder browser opens: at the folder set now, or the
    /// nearest folder above it that is there, else where pictures are
    /// browsed from.
    fn folder_start(&self, now: Option<PathBuf>) -> PathBuf {
        now.as_deref()
            .and_then(|p| p.ancestors().find(|a| a.is_dir()))
            .map(PathBuf::from)
            .unwrap_or_else(|| browse_start(&self.browse_from))
    }

    /// `x`: a setting kept in `settings.json` goes back to its default.
    fn reset_setting(&mut self, selected: usize) {
        let Some(row) = self.settings_rows().into_iter().nth(selected) else {
            return;
        };
        if !row.resettable {
            return;
        }
        self.set_setting(selected, String::new());
    }

    /// Keep `value` for the setting on row `selected`; an empty value is
    /// the default. A folder is refused, with the reason, when it cannot be
    /// written, and a video service that is not a web address is refused.
    fn set_setting(&mut self, selected: usize, value: String) {
        let Some(row) = self.settings_rows().into_iter().nth(selected) else {
            return;
        };
        let value = value.trim().to_string();
        let folder = matches!(row.name, "Download folder" | "Picture cache");
        if folder
            && !value.is_empty()
            && let Err(why) = crate::config::check_writable(std::path::Path::new(&value))
        {
            // The browser stays open for another choice.
            self.error(why);
            return;
        }
        if row.name == "Video service"
            && !value.is_empty()
            && !(value.starts_with("https://") || value.starts_with("http://"))
        {
            self.error("the video service is a web address, such as https://video.bsky.app");
            return;
        }
        let kept = (!value.is_empty()).then(|| value.clone());
        match row.name {
            "Download folder" => self.settings.download_dir = kept,
            "Picture cache" => {
                self.settings.cache_dir = kept;
                // The pictures are kept in the new folder from now on.
                if self.pictures {
                    self.pictures_change = Some(true);
                }
            }
            "Video service" => self.settings.video_service = kept,
            "Browser" => self.settings.browser = kept,
            _ => return,
        }
        self.close_edit(selected);
        let shown = self
            .settings_rows()
            .into_iter()
            .nth(selected)
            .map(|r| r.value)
            .unwrap_or_default();
        let name = row.name.to_lowercase();
        self.save_settings(if value.is_empty() {
            format!("{name}: back to the default, {shown}")
        } else {
            format!("{name}: {shown}")
        });
    }

    /// Save the settings as they are now, saying `note` once they are
    /// written. An unreadable `settings.json` is not overwritten: the change
    /// holds for this run, and the reason is shown.
    fn save_settings(&mut self, note: String) {
        if self.settings_writable {
            self.settings_to_save = Some(self.settings.clone());
            self.save_note = Some(note);
        } else {
            self.error(format!(
                "{note} for this session only; settings.json could not be read, so it is \
                 not overwritten (fix or remove it to save)"
            ));
        }
    }

    /// Open the selected post's pictures (or video) full screen; a post
    /// with none but a link opens the link. A terminal that cannot show
    /// them opens the post on bsky.app, where they can be seen.
    fn open_viewer(&mut self) -> Vec<Job> {
        let Some(post) = self.post_to_view() else {
            return Vec::new();
        };
        let media = post.embed.as_ref().map(|e| e.media()).unwrap_or_default();
        if media.is_empty() {
            return self.open_link(false);
        }
        if !self.pictures {
            return match post.web_url() {
                Some(url) => vec![self.open_url(url)],
                None => self.open_link(false),
            };
        }
        self.overlay = Some(Overlay::Viewer {
            media,
            index: 0,
            replay: 0,
        });
        Vec::new()
    }

    /// Open the selected post's first link in the web browser. `or_post`
    /// falls back to the post's own page on bsky.app, where its replies
    /// are: that is what `o` does, so the key always leads somewhere.
    /// Space does not, since a browser is not what it offers.
    fn open_link(&mut self, or_post: bool) -> Vec<Job> {
        let Some(post) = self.post_to_view() else {
            return Vec::new();
        };
        let url = post
            .links()
            .into_iter()
            .next()
            .or_else(|| or_post.then(|| post.web_url()).flatten());
        match url {
            Some(url) => vec![self.open_url(url)],
            None => {
                self.info("this post has no pictures, video, or link");
                Vec::new()
            }
        }
    }

    fn open_thread(&mut self) -> Vec<Job> {
        // A like or repost opens the thread of the post it is about.
        let post = self.post_to_view();
        let Some(post) = post else {
            return Vec::new();
        };
        self.threads.push(ThreadView {
            uri: post.uri.clone(),
            ..ThreadView::default()
        });
        vec![Job::Thread(post.uri)]
    }

    fn refresh(&mut self) -> Vec<Job> {
        if let Some(th) = self.threads.last_mut() {
            th.list.loaded = false;
            th.error = None;
            return vec![Job::Thread(th.uri.clone())];
        }
        match self.tab {
            Tab::Timeline => {
                self.info("refreshing…");
                match self.current_feed() {
                    Feed::Custom(uri) => {
                        self.feed_list().begin();
                        vec![Job::CustomFeed(uri)]
                    }
                    _ => vec![Job::Timeline],
                }
            }
            Tab::Search => self.run_search(),
            Tab::Profile => self.open_profile(self.profile.actor.clone()),
            Tab::Notifications => self.load_notifications(),
            Tab::Columns => {
                let id = self.columns.focused().map(|c| c.id);
                id.map(|id| self.load_column(id))
                    .into_iter()
                    .flatten()
                    .collect()
            }
        }
    }

    fn reply(&mut self) -> Vec<Job> {
        if let Some(post) = self.selected_post() {
            let excerpt = post.record().text.lines().next().unwrap_or("").to_string();
            self.overlay = Some(Overlay::Compose(Compose::new(Some((
                post.reply_ref(),
                post.author.handle.clone(),
                excerpt,
            )))));
        }
        Vec::new()
    }

    /// Claim `key` for a like or follow; false when one is already in flight.
    fn claim(&mut self, key: String) -> bool {
        if self.in_flight.insert(key) {
            true
        } else {
            self.info("still waiting for the server…");
            false
        }
    }

    fn toggle_like(&mut self) -> Vec<Job> {
        let Some(post) = self.selected_post() else {
            return Vec::new();
        };
        if !self.claim(format!("like:{}", post.uri)) {
            return Vec::new();
        }
        match post.like_uri() {
            Some(like) => vec![Job::Unlike {
                post_uri: post.uri.clone(),
                like_uri: like.to_string(),
            }],
            None => vec![Job::Like {
                subject: post.strong_ref(),
            }],
        }
    }

    /// A key while the actions list is open: move, run the chosen entry, or
    /// run the key itself, which is what it would have done anyway.
    fn actions_key(&mut self, key: KeyEvent, selected: usize, about: Option<String>) -> Vec<Job> {
        // The selection moved off the post the list was opened on (a reload
        // without it, say): what it offers is not about that post any more.
        if self.actions_subject() != about {
            self.overlay = None;
            self.info("the post the list was about is no longer selected; press . again");
            return Vec::new();
        }
        let entries = keys::actions(self);
        let n = entries.len().max(1);
        let selected = selected.min(n - 1);
        let run = |app: &mut Self, name: &'static str| {
            app.overlay = None;
            app.main_key(keys::action_key(name))
        };
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.overlay = Some(Overlay::Actions {
                    selected: (selected + 1) % n,
                    about,
                });
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.overlay = Some(Overlay::Actions {
                    selected: (selected + n - 1) % n,
                    about,
                });
            }
            KeyCode::Esc | KeyCode::Char('.' | 'q') => self.overlay = None,
            KeyCode::Enter => {
                return match entries.get(selected) {
                    Some(&(name, _)) => run(self, name),
                    None => {
                        self.overlay = None;
                        Vec::new()
                    }
                };
            }
            code => {
                if let Some(&(name, _)) = entries
                    .iter()
                    .find(|(name, _)| keys::action_key(name).code == code)
                {
                    return run(self, name);
                }
            }
        }
        Vec::new()
    }

    /// `.`: what the keys of this view do to the selected post, as a list.
    /// Nothing to act on means nothing to show.
    fn open_actions(&mut self) {
        if keys::actions(self).is_empty() {
            return;
        }
        self.overlay = Some(Overlay::Actions {
            selected: 0,
            about: self.actions_subject(),
        });
    }

    /// What the actions list acts on: the selected post, or, where the keys
    /// act on an account alone, that account.
    fn actions_subject(&self) -> Option<String> {
        self.shown_post()
            .map(|p| p.uri.clone())
            .or_else(|| self.shown_account().map(|a| a.did.clone()))
    }

    /// The account the keys act on here, without taking a copy of it.
    pub fn shown_account(&self) -> Option<&Profile> {
        if !self.threads.is_empty() {
            return self.shown_post().map(|p| &p.author);
        }
        if let Some(l) = self.shown_notifications() {
            return l.current().map(|i| &i.n.author);
        }
        match self.tab {
            Tab::Search if self.search.mode == SearchMode::Accounts => self.search.actors.current(),
            Tab::Profile => self.profile.profile.as_ref(),
            _ => self.shown_post().map(|p| &p.author),
        }
    }

    /// `Q`: a new post that quotes the selected one.
    fn quote(&mut self) {
        if let Some(post) = self.selected_post() {
            let excerpt = post.record().text.lines().next().unwrap_or("").to_string();
            self.overlay = Some(Overlay::Compose(Compose::quoting((
                post.strong_ref(),
                post.author.handle.clone(),
                excerpt,
            ))));
        }
    }

    /// `c`: the selected post's address on bsky.app, for the terminal's
    /// clipboard. It is the address `o` opens, so the two keys agree.
    fn copy_link(&mut self) {
        let Some(post) = self.post_to_view() else {
            return;
        };
        match post.web_url() {
            Some(url) => {
                self.info(format!("copied {url}"));
                self.to_copy = Some(url);
            }
            None => self.info("this post has no address to copy"),
        }
    }

    /// Text waiting to go to the terminal's clipboard, taken by the event
    /// loop that owns the terminal.
    pub fn take_copy(&mut self) -> Option<String> {
        self.to_copy.take()
    }

    /// `D`: ask before deleting the selected post. Only your own, and only
    /// with a second key, since a deleted post cannot be brought back.
    fn ask_delete(&mut self) {
        let Some(post) = self.selected_post() else {
            return;
        };
        let mine = self
            .session
            .as_ref()
            .is_some_and(|s| s.did == post.author.did);
        if !mine {
            self.info("you can only delete your own posts");
            return;
        }
        if self.in_flight.contains(&format!("delete:{}", post.uri)) {
            self.info("still waiting for the server…");
            return;
        }
        self.info("press y to delete this post, any other key to keep it");
        self.confirm_delete = Some(post.uri);
    }

    fn toggle_repost(&mut self) -> Vec<Job> {
        let Some(post) = self.selected_post() else {
            return Vec::new();
        };
        if !self.claim(format!("repost:{}", post.uri)) {
            return Vec::new();
        }
        match post.repost_uri() {
            Some(uri) => vec![Job::Unrepost {
                post_uri: post.uri.clone(),
                repost_uri: uri.to_string(),
            }],
            None => vec![Job::Repost {
                subject: post.strong_ref(),
            }],
        }
    }

    fn toggle_follow(&mut self) -> Vec<Job> {
        let Some(account) = self.selected_account() else {
            return Vec::new();
        };
        if self.session.as_ref().is_some_and(|s| s.did == account.did) {
            self.error("you cannot follow yourself");
            return Vec::new();
        }
        if !self.claim(format!("follow:{}", account.did)) {
            return Vec::new();
        }
        match account.following_uri() {
            Some(uri) => vec![Job::Unfollow {
                did: account.did.clone(),
                follow_uri: uri.to_string(),
            }],
            None => vec![Job::Follow {
                did: account.did.clone(),
            }],
        }
    }

    fn edit_profile(&mut self) -> Vec<Job> {
        if self.profile.actor.is_some() {
            self.error("you can only edit your own profile (Esc returns to it)");
            return Vec::new();
        }
        self.overlay = Some(Overlay::EditProfile(EditProfile {
            fields: [
                TextInput::single(""),
                TextInput::multi(""),
                TextInput::single(""),
            ],
            focus: 0,
            loading: true,
            saving: false,
            browser: None,
            avatar_chosen: None,
        }));
        vec![Job::LoadProfileEditor]
    }

    /// Apply `f` to every copy of the post `uri` on screen.
    /// The list of the feed the Timeline tab shows.
    /// Whether the selected post is the account's own: what `D` deletes,
    /// and what the hint row offers it on.
    pub fn own_post_selected(&self) -> bool {
        let Some(did) = self.session.as_ref().map(|s| s.did.as_str()) else {
            return false;
        };
        self.shown_post().is_some_and(|p| p.author.did == did)
    }

    /// The selected post, without taking a copy of it: what the hint row
    /// asks about while it is drawn.
    pub fn shown_post(&self) -> Option<&Post> {
        if let Some(th) = self.threads.last() {
            return th.list.current().and_then(ThreadRow::post);
        }
        if let Some(l) = self.shown_notifications() {
            return l.current().and_then(|i| i.post.as_ref());
        }
        let list = match self.tab {
            Tab::Timeline => Some(self.shown_feed()),
            Tab::Search if self.search.mode == SearchMode::Posts => Some(&self.search.posts),
            Tab::Profile => Some(&self.profile.posts),
            Tab::Columns => match self.columns.focused().map(|c| &c.rows) {
                Some(Rows::Posts(l)) => Some(l),
                _ => None,
            },
            _ => None,
        };
        list.and_then(List::current)
    }

    /// The list the Timeline tab shows: the following timeline, or the
    /// pinned feed it has moved to.
    fn shown_feed(&self) -> &List<Post> {
        match self.feed.checked_sub(1).and_then(|i| self.feeds.get(i)) {
            Some(f) => &f.list,
            None => &self.timeline,
        }
    }

    pub fn feed_list(&mut self) -> &mut List<Post> {
        match self.feed.checked_sub(1).and_then(|i| self.feeds.get_mut(i)) {
            Some(f) => &mut f.list,
            None => &mut self.timeline,
        }
    }

    /// The feed the Timeline tab shows, as the worker names it.
    pub fn current_feed(&self) -> Feed {
        match self.feed.checked_sub(1).and_then(|i| self.feeds.get(i)) {
            Some(f) => Feed::Custom(f.info.uri.clone()),
            None => Feed::Timeline,
        }
    }

    /// Show the next (`1`) or previous (`-1`) feed, going round, and load
    /// it the first time it is shown.
    fn switch_feed(&mut self, delta: isize) -> Vec<Job> {
        if self.feeds.is_empty() {
            return Vec::new();
        }
        let count = self.feeds.len() as isize + 1;
        self.feed = (self.feed as isize + delta).rem_euclid(count) as usize;
        let feed = self.current_feed();
        let list = self.feed_list();
        match feed {
            Feed::Custom(uri) if !list.loaded && !list.loading => {
                list.begin();
                vec![Job::CustomFeed(uri)]
            }
            _ => Vec::new(),
        }
    }

    /// Take the pinned feeds, keeping what was loaded of a feed still
    /// pinned, and the feed shown when it still is.
    fn set_pinned_feeds(&mut self, infos: Vec<crate::api::types::FeedInfo>) {
        let shown = self.current_feed();
        let mut old: Vec<CustomFeed> = std::mem::take(&mut self.feeds);
        self.feeds = infos
            .into_iter()
            .map(
                |info| match old.iter().position(|f| f.info.uri == info.uri) {
                    Some(i) => CustomFeed {
                        info,
                        list: std::mem::take(&mut old[i].list),
                    },
                    None => CustomFeed {
                        info,
                        list: List::default(),
                    },
                },
            )
            .collect();
        self.feed = match shown {
            Feed::Custom(uri) => self
                .feeds
                .iter()
                .position(|f| f.info.uri == uri)
                .map_or(0, |i| i + 1),
            _ => 0,
        };
    }

    fn each_post(&mut self, uri: &str, mut f: impl FnMut(&mut Post)) {
        let feeds = self
            .feeds
            .iter_mut()
            .map(|f| &mut f.list)
            .chain(self.columns.post_lists());
        for list in [
            &mut self.timeline,
            &mut self.search.posts,
            &mut self.profile.posts,
        ]
        .into_iter()
        .chain(feeds)
        {
            list.items
                .iter_mut()
                .filter(|p| p.uri == uri)
                .for_each(&mut f);
        }
        for th in &mut self.threads {
            th.list
                .items
                .iter_mut()
                .filter_map(ThreadRow::post_mut)
                .filter(|p| p.uri == uri)
                .for_each(&mut f);
        }
        let notifications =
            std::iter::once(&mut self.notifications).chain(self.columns.notification_lists());
        for list in notifications {
            for item in &mut list.items {
                item.post
                    .iter_mut()
                    .chain(item.subject.iter_mut())
                    .filter(|p| p.uri == uri)
                    .for_each(&mut f);
            }
        }
    }

    /// Set the follow state of `did` everywhere it is shown.
    fn set_following(&mut self, did: &str, uri: Option<String>) {
        let apply = |p: &mut Profile| {
            if p.did == did {
                p.viewer.get_or_insert_with(Default::default).following = uri.clone();
            }
        };
        let feeds = self
            .feeds
            .iter_mut()
            .map(|f| &mut f.list)
            .chain(self.columns.post_lists());
        for list in [
            &mut self.timeline,
            &mut self.search.posts,
            &mut self.profile.posts,
        ]
        .into_iter()
        .chain(feeds)
        {
            list.items.iter_mut().for_each(|p| apply(&mut p.author));
        }
        self.search.actors.items.iter_mut().for_each(apply);
        for th in &mut self.threads {
            th.list
                .items
                .iter_mut()
                .filter_map(ThreadRow::post_mut)
                .for_each(|p| apply(&mut p.author));
        }
        let notifications =
            std::iter::once(&mut self.notifications).chain(self.columns.notification_lists());
        for list in notifications {
            for item in &mut list.items {
                apply(&mut item.n.author);
            }
        }
        if let Some(p) = &mut self.profile.profile {
            apply(p);
        }
    }

    /// Show a failed job's error. A refresh token the server no longer
    /// accepts cannot be recovered in place, so it brings back the login form,
    /// filled in with the account that was logged in.
    fn fail(&mut self, e: &Error) {
        if e.message().starts_with("com.atproto.server.refreshSession") {
            let (service, handle) = self
                .session
                .as_ref()
                .map(|s| (s.service.clone(), s.handle.clone()))
                .unwrap_or_default();
            let mut form = LoginForm::new(&service);
            form.fields[1] = TextInput::single(&handle);
            form.focus = 2;
            form.error = Some("the session has expired; log in again".into());
            self.login = Some(form);
            // A question asked before is not answered by the first key after
            // logging in again, and a theme being previewed was not chosen.
            self.confirm_delete = None;
            if let Some(Overlay::Themes { previous, .. }) = self.overlay {
                self.set_theme(previous);
            }
            // The composer and the profile editor hold text the user typed,
            // which is theirs to send after logging in again; they stay
            // behind the login form, which takes every key while it is up.
            // Nothing else behind it is worth keeping, and a video must not
            // go on playing there.
            if !matches!(
                self.overlay,
                Some(Overlay::Compose(_) | Overlay::EditProfile(_))
            ) {
                self.overlay = None;
            }
            return;
        }
        // The hint is the part that says what to do; the error box shows it.
        match e.hint() {
            Some(hint) => self.error(format!("{}\nhint: {hint}", e.message())),
            None => self.error(e.message().to_string()),
        }
    }

    /// Drop everything loaded for the account that was logged in, when
    /// another one logs in: its notifications, its profile, its threads.
    fn forget_account(&mut self) {
        // Whatever was being typed belonged to the account left behind.
        self.overlay = None;
        self.confirm_delete = None;
        self.confirm_logout = None;
        self.confirm_column_remove = None;
        self.columns = Columns::default();
        self.timeline = List::default();
        self.feeds.clear();
        self.feed = 0;
        self.search.posts = List::default();
        self.search.actors = List::default();
        self.profile = ProfilePane::default();
        self.threads.clear();
        self.notifications = List::default();
        self.unread = 0;
        self.seen_pending = None;
        self.in_flight.clear();
        self.tab = Tab::Timeline;
    }

    /// Whether a loaded profile is the one the Profile tab is waiting for;
    /// an answer for a profile opened earlier is dropped.
    fn wanted_profile(&self, p: &Profile) -> bool {
        match (&self.profile.actor, &self.session) {
            (Some(actor), _) => *actor == p.did || *actor == p.handle,
            (None, Some(s)) => s.did == p.did,
            (None, None) => false,
        }
    }

    /// Put a further page where it belongs, or drop it when that list has
    /// moved on (a new query, another profile, a refresh).
    fn more(&mut self, feed: Feed, cursor: &str, result: crate::error::Result<MorePage>) {
        let own_did = self.profile.profile.as_ref().map(|p| p.did.clone());
        match (feed, result) {
            (Feed::Timeline, Ok(MorePage::Posts(page))) => self.timeline.append(cursor, page),
            (Feed::Custom(uri), Ok(MorePage::Posts(page))) => {
                if let Some(f) = self.feeds.iter_mut().find(|f| f.info.uri == uri) {
                    f.list.append(cursor, page);
                }
            }
            (Feed::SearchPosts(q), Ok(MorePage::Posts(page))) if q == self.search.posts_query => {
                self.search.posts.append(cursor, page)
            }
            (Feed::SearchActors(q), Ok(MorePage::Actors(page)))
                if q == self.search.actors_query =>
            {
                self.search.actors.append(cursor, page)
            }
            (Feed::Author(did), Ok(MorePage::Posts(page))) if Some(&did) == own_did.as_ref() => {
                self.profile.posts.append(cursor, page)
            }
            (Feed::Notifications, Ok(MorePage::Notifications(page))) => {
                self.notifications.append(cursor, page)
            }
            (feed, Err(e)) => {
                // Let the next move try again.
                match feed {
                    Feed::Notifications => self.notifications.more_pending = false,
                    Feed::Timeline => self.timeline.more_pending = false,
                    Feed::Custom(uri) => {
                        if let Some(f) = self.feeds.iter_mut().find(|f| f.info.uri == uri) {
                            f.list.more_pending = false;
                        }
                    }
                    Feed::SearchPosts(_) => self.search.posts.more_pending = false,
                    Feed::SearchActors(_) => self.search.actors.more_pending = false,
                    Feed::Author(_) => self.profile.posts.more_pending = false,
                }
                self.fail(&e);
            }
            _ => {}
        }
    }

    fn event(&mut self, event: Event) -> Vec<Job> {
        match &event {
            Event::Liked { post_uri, .. } | Event::Unliked { post_uri, .. } => {
                self.in_flight.remove(&format!("like:{post_uri}"));
            }
            Event::Reposted { post_uri, .. } | Event::Unreposted { post_uri, .. } => {
                self.in_flight.remove(&format!("repost:{post_uri}"));
            }
            Event::Followed { did, .. } | Event::Unfollowed { did, .. } => {
                self.in_flight.remove(&format!("follow:{did}"));
            }
            Event::PostDeleted { uri, .. } => {
                self.in_flight.remove(&format!("delete:{uri}"));
            }
            _ => {}
        }
        match event {
            Event::LoggedIn(Ok(session)) => {
                self.info(format!("logged in as @{}", session.handle));
                self.remember_account(&session);
                let other = self.session.as_ref().is_none_or(|s| s.did != session.did);
                if self.session.as_ref().is_some_and(|s| s.did != session.did) {
                    self.forget_account();
                    self.account_since = self.sent + 1;
                }
                self.session = Some(session);
                self.login = None;
                if other {
                    self.load_columns();
                }
                return self.startup_jobs();
            }
            Event::LoggedIn(Err(e)) => {
                if let Some(form) = &mut self.login {
                    form.pending = false;
                    form.error = Some(e.message().to_string());
                }
            }
            Event::Timeline(Ok(posts)) => {
                if self
                    .status
                    .as_ref()
                    .is_some_and(|s| s.text == "refreshing…")
                {
                    self.status = None;
                }
                self.timeline.set(posts);
            }
            Event::PinnedFeeds(Ok(infos)) => self.set_pinned_feeds(infos),
            // The following timeline still works; the feeds just do not show.
            Event::PinnedFeeds(Err(_)) => {}
            Event::CustomFeed { uri, result } => {
                let shown = self.current_feed() == Feed::Custom(uri.clone());
                if let Some(f) = self.feeds.iter_mut().find(|f| f.info.uri == uri) {
                    match result {
                        Ok(page) => {
                            f.list.set(page);
                            if shown
                                && self
                                    .status
                                    .as_ref()
                                    .is_some_and(|s| s.text == "refreshing…")
                            {
                                self.status = None;
                            }
                        }
                        Err(e) => {
                            f.list.failed(&e);
                            if shown {
                                self.fail(&e);
                            }
                        }
                    }
                }
            }
            // Searches run beside each other: an answer for a query that
            // has since been replaced is dropped.
            Event::SearchPosts { query, .. } if query != self.search.posts_query => {}
            Event::SearchActors { query, .. } if query != self.search.actors_query => {}
            Event::SearchPosts {
                result: Ok(posts), ..
            } => self.search.posts.set(posts),
            Event::SearchActors {
                result: Ok(actors), ..
            } => self.search.actors.set(actors),
            Event::Profile(Ok((profile, posts))) => {
                if self.wanted_profile(&profile) {
                    self.profile.profile = Some(profile);
                    self.profile.posts.set(posts);
                    self.profile.error = None;
                    self.profile.loading = false;
                }
            }
            Event::Liked {
                post_uri,
                result: Ok(like),
            } => {
                self.set_like(&post_uri, Some(like));
                self.info("liked");
            }
            Event::Unliked {
                post_uri,
                result: Ok(()),
            } => {
                self.set_like(&post_uri, None);
                self.info("like removed");
            }
            Event::Reposted {
                post_uri,
                result: Ok(repost),
            } => {
                self.set_repost(&post_uri, Some(repost));
                self.info("reposted");
            }
            Event::Unreposted {
                post_uri,
                result: Ok(()),
            } => {
                self.set_repost(&post_uri, None);
                self.info("repost removed");
            }
            Event::More {
                feed,
                cursor,
                result,
            } => self.more(feed, &cursor, result),
            Event::Column {
                id,
                generation,
                cursor,
                result,
            } => self.column_page(id, generation, cursor, result),
            Event::Notifications { seen_at, result } => match result {
                Ok(page) => {
                    self.notifications.set(page);
                    self.unread = self
                        .notifications
                        .items
                        .iter()
                        .filter(|i| !i.n.is_read)
                        .count();
                    if self.unread > 0 {
                        if self.tab == Tab::Notifications {
                            return vec![Job::UpdateSeen(seen_at)];
                        }
                        // Loaded in the background: seen when looked at.
                        self.seen_pending = Some(seen_at);
                    }
                }
                Err(e) => {
                    self.notifications.failed(&e);
                    // A background load that fails says so on its tab, not
                    // over the timeline (an expired session still says so).
                    if self.tab == Tab::Notifications
                        || e.message().starts_with("com.atproto.server.refreshSession")
                    {
                        self.fail(&e);
                    }
                }
            },
            Event::Seen(Ok(())) => {
                self.notifications
                    .items
                    .iter_mut()
                    .for_each(|i| i.n.is_read = true);
                self.unread = 0;
            }
            Event::Seen(Err(e)) => self.fail(&e),
            Event::Thread { uri, result } => {
                // Only a thread still waiting for it takes the answer; it
                // need not be on top (one can be opened over a reload).
                let Some(th) = self
                    .threads
                    .iter_mut()
                    .rev()
                    .find(|t| t.uri == uri && !t.list.loaded)
                else {
                    return Vec::new();
                };
                match result {
                    Ok(node) => {
                        // A reload keeps the row that was selected, as every
                        // other list does; only a first load jumps to the
                        // post the thread was opened on.
                        let was = th
                            .list
                            .items
                            .get(th.list.selected)
                            .map(|r| r.key().to_string());
                        let (rows, focus) = thread_rows::flatten(node);
                        th.list.selected = was
                            .and_then(|key| rows.iter().position(|r| r.key() == key))
                            .unwrap_or(focus);
                        th.list.items = rows;
                        // The view scrolls the selection into sight from here.
                        th.list.offset = th.list.offset.min(th.list.selected);
                        th.list.loaded = true;
                    }
                    Err(e) => {
                        th.list.loaded = true;
                        th.error = Some(e.message().to_string());
                        self.fail(&e);
                    }
                }
            }
            Event::Followed {
                did,
                result: Ok(uri),
            } => {
                self.set_following(&did, Some(uri));
                self.info("followed");
            }
            Event::Unfollowed {
                did,
                result: Ok(()),
            } => {
                self.set_following(&did, None);
                // The timeline shows followed accounts only.
                self.timeline.retain(|p| p.author.did != did);
                self.info("unfollowed");
            }
            Event::Posted {
                reply_to,
                result: Ok(()),
            } => {
                self.overlay = None;
                match reply_to {
                    Some(uri) => {
                        self.each_post(&uri, |p| p.reply_count += 1);
                        self.info("reply sent");
                    }
                    None => self.info("posted"),
                }
                // Your own posts are on the timeline: load it again so the
                // new one is there.
                return vec![Job::Timeline];
            }
            Event::Posted { result: Err(e), .. } => {
                if let Some(Overlay::Compose(c)) = &mut self.overlay {
                    c.sending = false;
                }
                self.fail(&e);
            }
            Event::PostDeleted {
                uri,
                result: Ok(()),
            } => {
                self.remove_post(&uri);
                self.info("post deleted");
            }
            Event::PostDeleted { result: Err(e), .. } => self.fail(&e),
            Event::ProfileEditor(result) => {
                // Only an editor still waiting takes the answer: a late one
                // must not overwrite what the user has typed since.
                if let Some(Overlay::EditProfile(e)) = &mut self.overlay
                    && e.loading
                {
                    match result {
                        Ok(fields) => {
                            e.fields[0] = TextInput::single(&fields.display_name);
                            e.fields[1] = TextInput::multi(&fields.description);
                            e.loading = false;
                        }
                        Err(err) => {
                            self.overlay = None;
                            self.fail(&err);
                        }
                    }
                }
            }
            Event::Downloaded(Ok(path)) => self.info(format!("saved {}", path.display())),
            Event::Opened {
                url,
                result: Ok(()),
            } => self.info(format!("opened {url}")),
            Event::Opened { result: Err(e), .. } => self.fail(&e),
            Event::Downloaded(Err(e)) => self.fail(&e),
            Event::ProfileSaved(Ok(())) => {
                self.overlay = None;
                self.info("profile updated");
                if self.profile.actor.is_none() {
                    return self.open_profile(None);
                }
            }
            Event::ProfileSaved(Err(e)) => {
                if let Some(Overlay::EditProfile(ed)) = &mut self.overlay {
                    ed.saving = false;
                }
                if e.message().contains("InvalidSwap") {
                    self.error(
                        "the profile was changed elsewhere since the editor opened; \
                         press Esc and e to start from the current version",
                    );
                } else {
                    self.fail(&e);
                }
            }
            // Each failure settles the view that was waiting for it, so no
            // list is left showing "loading…" after its answer has come.
            Event::Timeline(Err(e)) => {
                self.timeline.failed(&e);
                self.fail(&e);
            }
            Event::SearchPosts { result: Err(e), .. } => {
                self.search.posts.failed(&e);
                self.fail(&e);
            }
            Event::SearchActors { result: Err(e), .. } => {
                self.search.actors.failed(&e);
                self.fail(&e);
            }
            Event::Profile(Err(e)) => {
                self.profile.error = Some(e.message().to_string());
                self.profile.loading = false;
                self.fail(&e);
            }
            Event::Liked { result: Err(e), .. }
            | Event::Unliked { result: Err(e), .. }
            | Event::Reposted { result: Err(e), .. }
            | Event::Unreposted { result: Err(e), .. }
            | Event::Followed { result: Err(e), .. }
            | Event::Unfollowed { result: Err(e), .. } => self.fail(&e),
        }
        Vec::new()
    }
}

/// Where the picture browser opens: the folder it last showed, else the
/// current directory, else the home directory.
fn browse_start(last: &Option<PathBuf>) -> PathBuf {
    last.clone()
        .or_else(|| std::env::current_dir().ok())
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use serde_json::json;

    fn session() -> Session {
        Session {
            service: "https://pds.test".into(),
            did: "did:plc:me".into(),
            handle: "me.test".into(),
            access_jwt: "a".into(),
            refresh_jwt: "r".into(),
        }
    }

    fn post(uri: &str, author: &str, following: bool) -> Post {
        let mut v = json!({
            "uri": uri, "cid": format!("cid-{uri}"),
            "author": {"did": author, "handle": format!("{author}.test")},
            "record": {"text": format!("text of {uri}")},
            "likeCount": 2,
        });
        if following {
            v["author"]["viewer"] =
                json!({"following": format!("at://did:plc:me/app.bsky.graph.follow/{author}")});
        }
        serde_json::from_value(v).unwrap()
    }

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn code(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn logged_in() -> App {
        let (mut app, jobs) = App::new(Some(session()), "https://bsky.social");
        assert!(matches!(
            jobs[..],
            [Job::Timeline, Job::Notifications, Job::PinnedFeeds]
        ));
        app.handle_event(Event::Timeline(Ok(vec![
            post("at://a/p/1", "did:plc:alice", true),
            post("at://b/p/2", "did:plc:bob", true),
        ]
        .into())));
        app
    }

    fn type_str(app: &mut App, s: &str) {
        for c in s.chars() {
            app.handle_key(key(c));
        }
    }

    #[test]
    fn without_a_session_the_login_form_submits_all_fields() {
        let (mut app, jobs) = App::new(None, "https://bsky.social");
        assert!(jobs.is_empty());
        type_str(&mut app, "alice.test");
        app.handle_key(code(KeyCode::Enter));
        type_str(&mut app, "pw-1234");
        let jobs = app.handle_key(code(KeyCode::Enter));
        match &jobs[..] {
            [
                Job::Login {
                    service,
                    identifier,
                    password,
                },
            ] => {
                assert_eq!(
                    (service.as_str(), identifier.as_str(), password.as_str()),
                    ("https://bsky.social", "alice.test", "pw-1234")
                );
            }
            other => panic!("{other:?}"),
        }
        let jobs = app.handle_event(Event::LoggedIn(Ok(session())));
        assert!(app.login.is_none());
        assert!(matches!(
            jobs[..],
            [Job::Timeline, Job::Notifications, Job::PinnedFeeds]
        ));
    }

    #[test]
    fn login_with_an_empty_field_does_not_send() {
        let (mut app, _) = App::new(None, "https://bsky.social");
        app.handle_key(code(KeyCode::Enter));
        let jobs = app.handle_key(code(KeyCode::Enter));
        assert!(jobs.is_empty());
        assert_eq!(app.login.as_ref().unwrap().focus, 1);
        assert!(app.login.as_ref().unwrap().error.is_some());
    }

    #[test]
    fn failed_login_shows_the_server_message_and_allows_retry() {
        let (mut app, _) = App::new(None, "https://bsky.social");
        app.login.as_mut().unwrap().pending = true;
        app.pending = 1;
        app.handle_event(Event::LoggedIn(Err(Error::api("bad password"))));
        let form = app.login.as_ref().unwrap();
        assert_eq!(form.error.as_deref(), Some("bad password"));
        assert!(!form.pending);
    }

    #[test]
    fn like_toggles_between_like_and_unlike() {
        let mut app = logged_in();
        let jobs = app.handle_key(key('l'));
        let Job::Like { subject } = &jobs[0] else {
            panic!("{jobs:?}")
        };
        assert_eq!(subject.uri, "at://a/p/1");
        app.handle_event(Event::Liked {
            post_uri: "at://a/p/1".into(),
            result: Ok("at://did:plc:me/app.bsky.feed.like/x".into()),
        });
        assert_eq!(app.timeline.items[0].like_count, 3);
        let jobs = app.handle_key(key('l'));
        assert!(matches!(&jobs[0], Job::Unlike { like_uri, .. } if like_uri.ends_with("/x")));
        app.handle_event(Event::Unliked {
            post_uri: "at://a/p/1".into(),
            result: Ok(()),
        });
        assert_eq!(app.timeline.items[0].like_count, 2);
        assert!(app.timeline.items[0].like_uri().is_none());
    }

    /// The post as a reload fetched after the server took the like or repost.
    fn reloaded(liked: bool, reposted: bool) -> Post {
        let mut p = post("at://a/p/1", "did:plc:alice", true);
        p.viewer = Some(crate::api::types::PostViewer {
            like: liked.then(|| "at://did:plc:me/app.bsky.feed.like/x".into()),
            repost: reposted.then(|| "at://did:plc:me/app.bsky.feed.repost/y".into()),
        });
        p.like_count = if liked { 3 } else { 2 };
        p.repost_count = u64::from(reposted);
        p
    }

    /// Press a key and number its jobs the way the event loop does as it
    /// sends them.
    fn press(app: &mut App, k: KeyEvent) -> Vec<u64> {
        let jobs = app.handle_key(k);
        jobs.iter().map(|j| app.stamp(j)).collect()
    }

    // A reload sent before a like, answered after it, holds the post as it
    // was: shown as is, the like would look undone and the next l would like
    // the post a second time.
    #[test]
    fn a_reload_sent_before_a_like_does_not_undo_it() {
        let mut app = logged_in();
        let reload = press(&mut app, key('R'))[0];
        let like = press(&mut app, key('l'))[0];
        app.handle_answer(
            like,
            Event::Liked {
                post_uri: "at://a/p/1".into(),
                result: Ok("at://did:plc:me/app.bsky.feed.like/x".into()),
            },
        );
        app.handle_answer(
            reload,
            Event::Timeline(Ok(vec![reloaded(false, false)].into())),
        );
        let p = &app.timeline.items[0];
        assert_eq!(p.like_uri(), Some("at://did:plc:me/app.bsky.feed.like/x"));
        assert_eq!(p.like_count, 3);
        let jobs = app.handle_key(key('l'));
        assert!(matches!(&jobs[..], [Job::Unlike { .. }]), "{jobs:?}");
    }

    // The same for a repost: the reload's copy of the post, from before it,
    // is shown reposted, so the next b removes the repost rather than
    // reposting a second time.
    #[test]
    fn a_reload_sent_before_a_repost_does_not_undo_it() {
        let mut app = logged_in();
        let reload = press(&mut app, key('R'))[0];
        let repost = press(&mut app, key('b'))[0];
        app.handle_answer(
            repost,
            Event::Reposted {
                post_uri: "at://a/p/1".into(),
                result: Ok("at://did:plc:me/app.bsky.feed.repost/y".into()),
            },
        );
        app.handle_answer(
            reload,
            Event::Timeline(Ok(vec![reloaded(false, false)].into())),
        );
        let p = &app.timeline.items[0];
        assert_eq!(
            p.repost_uri(),
            Some("at://did:plc:me/app.bsky.feed.repost/y")
        );
        assert_eq!(p.repost_count, 1);
        let jobs = app.handle_key(key('b'));
        assert!(matches!(&jobs[..], [Job::Unrepost { .. }]), "{jobs:?}");
    }

    // A post deleted while a reload was out does not come back with the
    // reload, which was read before the delete: shown again, it could be
    // deleted a second time, or answered as if it were still there.
    #[test]
    fn a_reload_sent_before_a_delete_does_not_bring_the_post_back() {
        let mine = || post("at://did:plc:me/app.bsky.feed.post/m1", "did:plc:me", false);
        let (mut app, _) = App::new(Some(session()), "https://bsky.social");
        app.handle_event(Event::Timeline(Ok(vec![mine()].into())));
        let thread = press(&mut app, key('v'))[0];
        app.handle_answer(
            thread,
            Event::Thread {
                uri: "at://did:plc:me/app.bsky.feed.post/m1".into(),
                result: Ok(serde_json::from_value(json!({
                    "$type": "app.bsky.feed.defs#threadViewPost",
                    "post": {"uri": "at://did:plc:me/app.bsky.feed.post/m1", "cid": "c",
                             "author": {"did": "did:plc:me", "handle": "me.test"},
                             "record": {"text": "mine"}},
                    "replies": [],
                }))
                .unwrap()),
            },
        );
        app.handle_key(code(KeyCode::Esc));
        let reload = press(&mut app, key('R'))[0];
        press(&mut app, key('D'));
        let delete = press(&mut app, key('y'))[0];
        app.handle_answer(
            delete,
            Event::PostDeleted {
                uri: "at://did:plc:me/app.bsky.feed.post/m1".into(),
                result: Ok(()),
            },
        );
        assert!(app.timeline.items.is_empty());
        app.handle_answer(reload, Event::Timeline(Ok(vec![mine()].into())));
        assert!(
            app.timeline.items.is_empty(),
            "the deleted post came back: {:?}",
            app.timeline.items
        );
    }

    // In an open thread, the deleted post keeps its row as a placeholder,
    // so the replies under it keep their place.
    #[test]
    fn a_post_deleted_in_a_thread_leaves_a_placeholder() {
        let mine = post("at://did:plc:me/app.bsky.feed.post/m1", "did:plc:me", false);
        let (mut app, _) = App::new(Some(session()), "https://bsky.social");
        app.handle_event(Event::Timeline(Ok(vec![mine.clone()].into())));
        app.handle_key(key('v'));
        app.handle_event(Event::Thread {
            uri: mine.uri.clone(),
            result: Ok(serde_json::from_value(json!({
                "$type": "app.bsky.feed.defs#threadViewPost",
                "post": {"uri": mine.uri, "cid": "c",
                         "author": {"did": "did:plc:me", "handle": "me.test"},
                         "record": {"text": "mine"}},
                "replies": [{
                    "$type": "app.bsky.feed.defs#threadViewPost",
                    "post": {"uri": "at://b/p/r", "cid": "c",
                             "author": {"did": "did:plc:bob", "handle": "bob.test"},
                             "record": {"text": "a reply"}},
                    "replies": [],
                }],
            }))
            .unwrap()),
        });
        app.handle_key(key('D'));
        let jobs = app.handle_key(key('y'));
        assert!(matches!(&jobs[..], [Job::DeletePost { .. }]), "{jobs:?}");
        app.handle_event(Event::PostDeleted {
            uri: mine.uri.clone(),
            result: Ok(()),
        });
        let rows = &app.threads[0].list.items;
        assert_eq!(rows.len(), 2);
        assert!(
            matches!(&rows[0].kind, RowKind::NotFound(u) if *u == mine.uri),
            "{:?}",
            rows[0].kind
        );
        assert!(rows[1].post().is_some_and(|p| p.uri == "at://b/p/r"));
    }

    #[test]
    fn a_reload_sent_before_an_unfollow_does_not_bring_the_account_back() {
        let mut app = logged_in();
        let reload = press(&mut app, key('R'))[0];
        let unfollow = press(&mut app, key('f'))[0];
        app.handle_answer(
            unfollow,
            Event::Unfollowed {
                did: "did:plc:alice".into(),
                result: Ok(()),
            },
        );
        app.handle_answer(
            reload,
            Event::Timeline(Ok(vec![
                post("at://a/p/1", "did:plc:alice", true),
                post("at://b/p/2", "did:plc:bob", true),
            ]
            .into())),
        );
        let authors: Vec<_> = app
            .timeline
            .items
            .iter()
            .map(|p| p.author.did.as_str())
            .collect();
        assert_eq!(authors, ["did:plc:bob"]);
    }

    // Two loads of one list in flight: the one sent last is what shows,
    // whichever answers last.
    #[test]
    fn an_older_first_page_does_not_replace_a_newer_one() {
        let mut app = logged_in();
        let first = press(&mut app, key('R'))[0];
        let second = press(&mut app, key('R'))[0];
        app.handle_answer(
            second,
            Event::Timeline(Ok(vec![
                post("at://me/p/new", "did:plc:me", false),
                post("at://a/p/1", "did:plc:alice", true),
            ]
            .into())),
        );
        app.handle_answer(
            first,
            Event::Timeline(Ok(vec![post("at://a/p/1", "did:plc:alice", true)].into())),
        );
        assert_eq!(app.timeline.items[0].uri, "at://me/p/new");
    }

    #[test]
    fn a_page_asked_for_by_the_last_account_is_dropped() {
        let mut app = logged_in();
        let reload = press(&mut app, key('R'))[0];
        let mut other = session();
        other.did = "did:plc:other".into();
        let login = app.stamp(&Job::UpdateSeen(String::new()));
        app.handle_answer(login, Event::LoggedIn(Ok(other)));
        app.handle_answer(
            reload,
            Event::Timeline(Ok(vec![post("at://a/p/1", "did:plc:alice", true)].into())),
        );
        assert!(app.timeline.items.is_empty());
    }

    // Reads run beside writes, so a reload can show a like or repost before
    // its own answer arrives; the answer must not count it a second time.
    #[test]
    fn a_like_or_repost_a_reload_already_shows_is_counted_once() {
        let mut app = logged_in();
        app.handle_key(key('l'));
        app.handle_key(key('b'));
        app.handle_event(Event::Timeline(Ok(vec![reloaded(true, true)].into())));
        app.handle_event(Event::Liked {
            post_uri: "at://a/p/1".into(),
            result: Ok("at://did:plc:me/app.bsky.feed.like/x".into()),
        });
        app.handle_event(Event::Reposted {
            post_uri: "at://a/p/1".into(),
            result: Ok("at://did:plc:me/app.bsky.feed.repost/y".into()),
        });
        let p = &app.timeline.items[0];
        assert_eq!((p.like_count, p.repost_count), (3, 1));

        app.handle_key(key('l'));
        app.handle_key(key('b'));
        app.handle_event(Event::Timeline(Ok(vec![reloaded(false, false)].into())));
        app.handle_event(Event::Unliked {
            post_uri: "at://a/p/1".into(),
            result: Ok(()),
        });
        app.handle_event(Event::Unreposted {
            post_uri: "at://a/p/1".into(),
            result: Ok(()),
        });
        let p = &app.timeline.items[0];
        assert_eq!((p.like_count, p.repost_count), (2, 0));
        assert!(p.like_uri().is_none());
    }

    #[test]
    fn results_for_an_earlier_search_are_dropped() {
        let mut app = logged_in();
        app.handle_key(key('/'));
        type_str(&mut app, "cats");
        app.handle_key(code(KeyCode::Enter));
        app.handle_key(key('/'));
        for _ in 0..4 {
            app.handle_key(code(KeyCode::Backspace));
        }
        type_str(&mut app, "dogs");
        let jobs = app.handle_key(code(KeyCode::Enter));
        assert!(
            matches!(&jobs[..], [Job::SearchPosts(q)] if q == "dogs"),
            "{jobs:?}"
        );
        app.handle_event(Event::SearchPosts {
            query: "cats".into(),
            result: Ok(vec![post("at://c/p/1", "did:plc:cat", false)].into()),
        });
        assert!(!app.search.posts.loaded);
        app.handle_event(Event::SearchPosts {
            query: "dogs".into(),
            result: Ok(vec![post("at://d/p/1", "did:plc:dog", false)].into()),
        });
        assert_eq!(app.search.posts.items[0].uri, "at://d/p/1");

        app.search.mode = SearchMode::Accounts;
        app.search.actors_query = "dogs".into();
        app.search.actors.begin();
        app.handle_event(Event::SearchActors {
            query: "cats".into(),
            result: Err(Error::api("late failure")),
        });
        assert!(!app.search.actors.loaded);
        assert!(app.status.is_none(), "{:?}", app.status);
    }

    /// Q quotes the selected post: the composer says whose post it quotes,
    /// and what goes out carries that post by URI and CID.
    #[test]
    fn quote_sends_the_quoted_post_s_reference() {
        let mut app = logged_in();
        app.handle_key(key('j'));
        app.handle_key(key('Q'));
        let Some(Overlay::Compose(c)) = &app.overlay else {
            panic!("{:?}", app.overlay)
        };
        assert_eq!(
            c.quote
                .as_ref()
                .map(|(r, h, _)| (r.uri.as_str(), h.as_str())),
            Some(("at://b/p/2", "did:plc:bob.test"))
        );
        assert!(c.reply.is_none());
        type_str(&mut app, "worth reading");
        let jobs = app.handle_key(ctrl('s'));
        match &jobs[..] {
            [
                Job::Post {
                    text, reply, quote, ..
                },
            ] => {
                assert_eq!(text, "worth reading");
                assert!(reply.is_none());
                let q = quote.as_ref().expect("the quoted post");
                assert_eq!(q.uri, "at://b/p/2");
                assert_eq!(q.cid, "cid-at://b/p/2");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn reply_sends_the_thread_reference() {
        let mut app = logged_in();
        app.handle_key(key('j'));
        app.handle_key(key('r'));
        type_str(&mut app, "hello");
        let jobs = app.handle_key(ctrl('s'));
        match &jobs[..] {
            [
                Job::Post {
                    text,
                    reply: Some(r),
                    media,
                    ..
                },
            ] => {
                assert_eq!(text, "hello");
                assert_eq!(r.parent.uri, "at://b/p/2");
                assert_eq!(r.root.uri, "at://b/p/2");
                assert!(media.is_empty());
            }
            other => panic!("{other:?}"),
        }
        app.handle_event(Event::Posted {
            reply_to: Some("at://b/p/2".into()),
            result: Ok(()),
        });
        assert!(app.overlay.is_none());
        assert_eq!(app.timeline.items[1].reply_count, 1);
    }

    #[test]
    fn composer_refuses_empty_and_too_long_posts() {
        let mut app = logged_in();
        app.handle_key(key('n'));
        assert!(app.handle_key(ctrl('s')).is_empty());
        assert!(app.status.as_ref().unwrap().error);
        type_str(&mut app, &"あ".repeat(crate::api::MAX_POST_GRAPHEMES + 1));
        assert!(app.handle_key(ctrl('s')).is_empty());
        assert!(app.status.as_ref().unwrap().text.contains("301"));
    }

    #[test]
    fn composer_refuses_a_post_of_emoji_over_the_byte_limit() {
        let mut app = logged_in();
        app.handle_key(key('n'));
        // 121 families: 121 characters, 3025 bytes.
        app.handle_paste(&"👨\u{200d}👩\u{200d}👧\u{200d}👦".repeat(121));
        assert!(app.handle_key(ctrl('s')).is_empty());
        let status = app.status.as_ref().unwrap();
        assert!(
            status.error && status.text.contains("3025 bytes"),
            "{}",
            status.text
        );
        // One fewer fits both limits and is sent.
        app.handle_key(KeyEvent::from(KeyCode::Backspace));
        assert!(matches!(app.handle_key(ctrl('s'))[..], [Job::Post { .. }]));
    }

    /// Replying reloads the timeline so your own post shows up; the
    /// selection stays on the post it was on, wherever that post now is,
    /// instead of jumping back to the first post.
    #[test]
    fn a_reload_after_replying_keeps_the_selected_post() {
        let mut app = logged_in();
        app.handle_key(key('j'));
        assert_eq!(app.timeline.current().unwrap().uri, "at://b/p/2");
        app.handle_key(key('r'));
        type_str(&mut app, "Good point");
        let jobs = app.handle_event(Event::Posted {
            reply_to: Some("at://b/p/2".into()),
            result: Ok(()),
        });
        assert!(matches!(jobs[..], [Job::Timeline]));
        // The reload has your reply on top and Bob's post one further down.
        app.handle_event(Event::Timeline(Ok(vec![
            post("at://me/p/3", "did:plc:me", false),
            post("at://a/p/1", "did:plc:alice", true),
            post("at://b/p/2", "did:plc:bob", true),
        ]
        .into())));
        assert_eq!(app.timeline.current().unwrap().uri, "at://b/p/2");
        // A post that is gone leaves the selection at the top.
        app.handle_event(Event::Timeline(Ok(vec![post(
            "at://a/p/1",
            "did:plc:alice",
            true,
        )]
        .into())));
        assert_eq!(app.timeline.selected, 0);
    }

    fn feed_info(name: &str) -> crate::api::types::FeedInfo {
        crate::api::types::FeedInfo {
            uri: format!("at://did:plc:f/app.bsky.feed.generator/{name}"),
            name: name.to_string(),
        }
    }

    fn feed_uri(name: &str) -> String {
        format!("at://did:plc:f/app.bsky.feed.generator/{name}")
    }

    /// The Timeline tab shows the following timeline first, then each
    /// pinned feed; [ and ] go through them, round, and a feed is loaded the
    /// first time it is shown.
    #[test]
    fn brackets_go_through_the_pinned_feeds_and_load_each_once() {
        let mut app = logged_in();
        assert!(app.handle_key(key(']')).is_empty(), "no feeds pinned yet");
        app.handle_event(Event::PinnedFeeds(Ok(vec![
            feed_info("discover"),
            feed_info("science"),
        ])));
        let jobs = app.handle_key(key(']'));
        assert!(
            matches!(&jobs[..], [Job::CustomFeed(u)] if *u == feed_uri("discover")),
            "{jobs:?}"
        );
        assert_eq!(app.current_feed(), Feed::Custom(feed_uri("discover")));
        // Pressed again while it loads: not asked twice.
        app.handle_key(key('['));
        assert!(app.handle_key(key(']')).is_empty());
        app.handle_event(Event::CustomFeed {
            uri: feed_uri("discover"),
            result: Ok(vec![post("at://x/p/9", "did:plc:x", false)].into()),
        });
        // Posts of a feed are acted on like any other.
        let jobs = app.handle_key(key('l'));
        assert!(matches!(&jobs[..], [Job::Like { subject }] if subject.uri == "at://x/p/9"));
        let jobs = app.handle_key(key(']'));
        assert!(matches!(&jobs[..], [Job::CustomFeed(u)] if *u == feed_uri("science")));
        // Round to the following timeline, which is already there.
        assert!(app.handle_key(key(']')).is_empty());
        assert_eq!(app.current_feed(), Feed::Timeline);
        assert_eq!(app.current_posts().unwrap().items.len(), 2);
        // The like's answer arrives while another feed is shown.
        app.handle_event(Event::Liked {
            post_uri: "at://x/p/9".into(),
            result: Ok("at://did:plc:me/app.bsky.feed.like/9".into()),
        });
        // Back to Discover: loaded, so nothing is asked; the like shows.
        assert!(app.handle_key(key('[')).is_empty());
        assert!(app.handle_key(key('[')).is_empty());
        let p = app.current_posts().unwrap().current().unwrap();
        assert!(p.like_uri().is_some());
        // R reloads the feed shown.
        let jobs = app.handle_key(key('R'));
        assert!(matches!(&jobs[..], [Job::CustomFeed(u)] if *u == feed_uri("discover")));
    }

    /// A page of a feed goes to that feed whatever is shown when it
    /// arrives, and an answer for a feed no longer pinned is dropped.
    #[test]
    fn feed_pages_land_in_their_own_feed() {
        let mut app = logged_in();
        app.handle_event(Event::PinnedFeeds(Ok(vec![feed_info("discover")])));
        app.handle_key(key(']'));
        let posts: Vec<Post> = (0..MORE_AHEAD + 1)
            .map(|i| post(&format!("at://x/p/{i}"), "did:plc:x", false))
            .collect();
        app.handle_event(Event::CustomFeed {
            uri: feed_uri("discover"),
            result: Ok(page(posts, Some("d1"))),
        });
        let mut asked = Vec::new();
        for _ in 0..3 {
            asked.extend(app.handle_key(key('j')));
        }
        assert!(
            matches!(&asked[..], [Job::More { feed: Feed::Custom(u), cursor }] if *u == feed_uri("discover") && cursor == "d1"),
            "{asked:?}"
        );
        // The reader goes back to the following timeline before it arrives.
        app.handle_key(key('['));
        app.handle_event(Event::More {
            feed: Feed::Custom(feed_uri("discover")),
            cursor: "d1".into(),
            result: Ok(MorePage::Posts(
                vec![post("at://x/p/next", "did:plc:x", false)].into(),
            )),
        });
        assert_eq!(
            app.timeline.items.len(),
            2,
            "the following timeline is untouched"
        );
        assert_eq!(app.feeds[0].list.items.len(), MORE_AHEAD + 2);
        // Unpinned meanwhile: its late answer goes nowhere.
        app.handle_event(Event::PinnedFeeds(Ok(vec![feed_info("science")])));
        app.handle_event(Event::CustomFeed {
            uri: feed_uri("discover"),
            result: Ok(vec![post("at://x/p/late", "did:plc:x", false)].into()),
        });
        assert!(app.feeds.iter().all(|f| f.list.items.is_empty()));
        assert_eq!(app.current_feed(), Feed::Timeline);
    }

    /// The actions list is a way to reach the keys of a post without
    /// remembering them: what it runs is exactly what the key runs.
    #[test]
    fn the_actions_list_runs_the_key_it_names() {
        let mut app = logged_in();
        assert!(app.handle_key(key('.')).is_empty());
        let Some(Overlay::Actions { selected: 0, .. }) = app.overlay else {
            panic!("{:?}", app.overlay)
        };
        // The entries say what each key would do to this post now.
        let entries = crate::tui::keys::actions(&app);
        assert_eq!(entries[0], ("r", "reply to it"));
        assert_eq!(entries[1], ("l", "like it"));
        // Moving to the like and choosing it sends the like the key sends.
        app.handle_key(key('j'));
        let jobs = app.handle_key(code(KeyCode::Enter));
        assert!(matches!(&jobs[..], [Job::Like { subject }] if subject.uri == "at://a/p/1"));
        assert!(app.overlay.is_none(), "the list closes when it has run");
        app.handle_event(Event::Liked {
            post_uri: "at://a/p/1".into(),
            result: Ok("at://did:plc:me/app.bsky.feed.like/l".into()),
        });
        // With the post liked, the list says what l would do now.
        app.handle_key(key('.'));
        assert_eq!(
            crate::tui::keys::actions(&app)[1],
            ("l", "remove your like")
        );
        // A key of the list works from inside it, and Esc leaves everything
        // as it was.
        app.handle_event(Event::Timeline(Ok(vec![post(
            "at://did:plc:bob/app.bsky.feed.post/p9",
            "did:plc:bob",
            true,
        )]
        .into())));
        app.handle_key(key('.'));
        app.handle_key(key('c'));
        assert!(app.overlay.is_none());
        assert_eq!(
            app.take_copy().as_deref(),
            Some("https://bsky.app/profile/did:plc:bob/post/p9")
        );
        app.handle_key(key('.'));
        app.handle_key(code(KeyCode::Esc));
        assert!(app.overlay.is_none());
        assert!(app.status.is_none() || !app.status.as_ref().unwrap().error);
    }

    /// Nothing selected, nothing to offer: `.` does not open an empty box.
    #[test]
    fn the_actions_list_does_not_open_on_an_empty_list() {
        let mut app = logged_in();
        app.handle_event(Event::Timeline(Ok(Vec::new().into())));
        assert!(app.handle_key(key('.')).is_empty());
        assert!(app.overlay.is_none(), "{:?}", app.overlay);
    }

    /// c puts the post's address where anything else can paste it.    /// c puts the post's address where anything else can paste it. The
    /// address is the one o opens, so the two keys agree.
    #[test]
    fn c_copies_the_post_s_address() {
        let mut app = logged_in();
        app.handle_event(Event::Timeline(Ok(vec![post(
            "at://did:plc:bob/app.bsky.feed.post/theirs",
            "did:plc:bob",
            true,
        )]
        .into())));
        assert!(app.handle_key(key('c')).is_empty());
        assert_eq!(
            app.take_copy().as_deref(),
            Some("https://bsky.app/profile/did:plc:bob/post/theirs")
        );
        // Taken once: the loop does not write it again on the next frame.
        assert!(app.take_copy().is_none());
        assert!(
            app.status
                .as_ref()
                .is_some_and(|s| s.text.contains("copied https://bsky.app/profile")),
            "{:?}",
            app.status
        );
    }

    /// Deleting cannot be undone, so it takes two keys: D asks, y sends,
    /// and one delete leaves the server. The post goes from every list it
    /// is in once the server confirms it.
    #[test]
    fn deleting_your_own_post_asks_first_and_sends_one_delete() {
        let mut app = logged_in();
        let mine = "at://did:plc:me/app.bsky.feed.post/mine";
        app.handle_event(Event::Timeline(Ok(vec![
            post(mine, "did:plc:me", false),
            post("at://b/p/2", "did:plc:bob", true),
        ]
        .into())));
        // Asking sends nothing.
        let jobs = app.handle_key(key('D'));
        assert!(jobs.is_empty(), "{jobs:?}");
        assert!(
            app.status
                .as_ref()
                .is_some_and(|s| s.text.contains("press y to delete")),
            "{:?}",
            app.status
        );
        let jobs = app.handle_key(key('y'));
        assert!(
            matches!(&jobs[..], [Job::DeletePost { uri }] if uri == mine),
            "{jobs:?}"
        );
        // A second D while the first is still out sends nothing more.
        assert!(app.handle_key(key('D')).is_empty());
        assert!(app.handle_key(key('y')).is_empty());
        app.handle_event(Event::PostDeleted {
            uri: mine.to_string(),
            result: Ok(()),
        });
        assert_eq!(app.timeline.items.len(), 1);
        assert_eq!(app.timeline.items[0].uri, "at://b/p/2");
        assert!(
            app.status
                .as_ref()
                .is_some_and(|s| s.text.contains("deleted")),
            "{:?}",
            app.status
        );
    }

    /// Another account's post is not deletable, and any key but y calls the
    /// question off without doing what that key usually does.
    #[test]
    fn only_your_own_post_is_deleted_and_any_other_key_calls_it_off() {
        let mut app = logged_in();
        let mine = "at://did:plc:me/app.bsky.feed.post/mine";
        app.handle_event(Event::Timeline(Ok(vec![
            post("at://b/p/2", "did:plc:bob", true),
            post(mine, "did:plc:me", false),
        ]
        .into())));
        // On Bob's post: nothing is asked and nothing is sent.
        let jobs = app.handle_key(key('D'));
        assert!(jobs.is_empty(), "{jobs:?}");
        assert!(
            app.status
                .as_ref()
                .is_some_and(|s| s.text.contains("only delete your own posts")),
            "{:?}",
            app.status
        );
        // On my own post, j calls the question off and does not move the
        // selection: the key that answers is y and nothing else.
        app.handle_key(key('j'));
        let before = app.timeline.selected;
        assert!(app.handle_key(key('D')).is_empty());
        let jobs = app.handle_key(key('j'));
        assert!(jobs.is_empty(), "{jobs:?}");
        assert_eq!(app.timeline.selected, before);
        assert!(
            app.status
                .as_ref()
                .is_some_and(|s| s.text.contains("not deleted")),
            "{:?}",
            app.status
        );
        // The next j moves as usual.
        app.handle_key(key('k'));
        assert_ne!(app.timeline.selected, before);
    }

    #[test]
    fn a_failed_post_keeps_the_draft() {
        let mut app = logged_in();
        app.handle_key(key('n'));
        type_str(&mut app, "draft");
        app.handle_key(ctrl('s'));
        app.handle_event(Event::Posted {
            reply_to: None,
            result: Err(Error::api("boom")),
        });
        let Some(Overlay::Compose(c)) = &app.overlay else {
            panic!()
        };
        assert_eq!(c.input.text(), "draft");
        assert!(!c.sending);
    }

    // was: the login form the expired session brings up dropped the
    // composer, so the draft was gone after logging in again.
    #[test]
    fn a_draft_survives_the_session_expiring_while_it_is_sent() {
        let mut app = logged_in();
        app.handle_key(key('n'));
        type_str(&mut app, "a long draft");
        app.handle_key(ctrl('s'));
        app.handle_event(Event::Posted {
            reply_to: None,
            result: Err(Error::api(
                "com.atproto.server.refreshSession failed: ExpiredToken: Token has expired",
            )),
        });
        assert!(app.login.is_some(), "the login form is up");
        let Some(Overlay::Compose(c)) = &app.overlay else {
            panic!("the composer was dropped")
        };
        assert_eq!(c.input.text(), "a long draft");
        assert!(!c.sending);
        // The login form takes the keys while it is up.
        app.handle_key(key('x'));
        let Some(Overlay::Compose(c)) = &app.overlay else {
            panic!()
        };
        assert_eq!(c.input.text(), "a long draft");
        app.handle_event(Event::LoggedIn(Ok(session())));
        let Some(Overlay::Compose(c)) = &app.overlay else {
            panic!("the draft was lost on logging in again")
        };
        assert_eq!(c.input.text(), "a long draft");
        // And it can be sent again.
        let jobs = app.handle_key(ctrl('s'));
        assert!(matches!(&jobs[..], [Job::Post { .. }]), "{jobs:?}");
    }

    #[test]
    fn a_profile_editor_survives_the_session_expiring_while_it_is_saved() {
        let mut app = logged_in();
        app.overlay = Some(Overlay::EditProfile(EditProfile {
            fields: [
                TextInput::single("My new name"),
                TextInput::multi("About me"),
                TextInput::single(""),
            ],
            focus: 0,
            loading: false,
            saving: true,
            browser: None,
            avatar_chosen: None,
        }));
        app.handle_event(Event::ProfileSaved(Err(Error::api(
            "com.atproto.server.refreshSession failed: ExpiredToken: Token has expired",
        ))));
        assert!(app.login.is_some());
        let Some(Overlay::EditProfile(e)) = &app.overlay else {
            panic!("the editor was dropped")
        };
        assert_eq!(e.fields[0].text(), "My new name");
        assert!(!e.saving);
        app.handle_event(Event::LoggedIn(Ok(session())));
        let Some(Overlay::EditProfile(e)) = &app.overlay else {
            panic!("the editor was lost on logging in again")
        };
        assert_eq!(e.fields[1].text(), "About me");
    }

    // Another account has nothing to do with the draft: it goes, as the rest
    // of the first account's state does.
    #[test]
    fn logging_in_as_someone_else_drops_the_draft() {
        let mut app = logged_in();
        app.handle_key(key('n'));
        type_str(&mut app, "a long draft");
        app.handle_key(ctrl('s'));
        app.handle_event(Event::Posted {
            reply_to: None,
            result: Err(Error::api(
                "com.atproto.server.refreshSession failed: ExpiredToken: Token has expired",
            )),
        });
        app.handle_event(Event::LoggedIn(Ok(Session {
            did: "did:plc:other".into(),
            handle: "other.test".into(),
            ..session()
        })));
        assert!(app.overlay.is_none());
    }

    /// On the Columns tab with the columns of `sources`, each answered with
    /// `posts` (notifications columns with none).
    fn columns_with(sources: &[columns::Source], posts: Vec<Post>) -> App {
        let mut app = logged_in();
        let mut settings = Settings::default();
        settings
            .columns
            .insert("did:plc:me".into(), sources.to_vec());
        app.apply_settings(settings, ColorDepth::TrueColor, None);
        let jobs = app.handle_key(key('5'));
        assert_eq!(jobs.len(), sources.len(), "{jobs:?}");
        for job in jobs {
            let Job::Column {
                id,
                generation,
                feed,
                cursor,
            } = job
            else {
                panic!("{job:?}")
            };
            assert!(cursor.is_none());
            let page = match feed {
                Feed::Notifications => MorePage::Notifications(Vec::new().into()),
                _ => MorePage::Posts(posts.clone().into()),
            };
            app.handle_event(Event::Column {
                id,
                generation,
                cursor: None,
                result: Ok(page),
            });
        }
        app
    }

    #[test]
    fn a_column_is_added_from_the_list_kept_and_loaded() {
        let mut app = logged_in();
        app.handle_key(key('5'));
        assert!(app.columns.items.is_empty());
        app.handle_key(key('+'));
        let choices = app.column_choices();
        let at = choices
            .iter()
            .position(|c| *c == columns::Source::Notifications)
            .unwrap();
        for _ in 0..at {
            app.handle_key(key('j'));
        }
        let jobs = app.handle_key(code(KeyCode::Enter));
        assert!(
            matches!(
                &jobs[..],
                [Job::Column {
                    feed: Feed::Notifications,
                    cursor: None,
                    ..
                }]
            ),
            "{jobs:?}"
        );
        let saved = app.take_settings_save().expect("settings to save");
        assert_eq!(
            saved.columns["did:plc:me"],
            [columns::Source::Notifications]
        );
        // A search column asks for its query first.
        app.handle_key(key('+'));
        app.handle_key(key('k'));
        app.handle_key(code(KeyCode::Enter));
        assert!(matches!(
            app.overlay,
            Some(Overlay::AddColumn { query: Some(_), .. })
        ));
        type_str(&mut app, "猫🐈‍⬛ 1️⃣");
        let jobs = app.handle_key(code(KeyCode::Enter));
        assert!(
            matches!(&jobs[..], [Job::Column { feed: Feed::SearchPosts(q), .. }] if q == "猫🐈‍⬛ 1️⃣"),
            "{jobs:?}"
        );
        assert_eq!(app.columns.focus, 1);
        assert!(app.overlay.is_none());
    }

    #[test]
    fn the_keys_of_a_post_act_on_the_focused_columns_selection() {
        let mut app = columns_with(
            &[
                columns::Source::Following,
                columns::Source::Search { query: "x".into() },
            ],
            vec![
                post("at://a/p/1", "did:plc:alice", true),
                post("at://b/p/2", "did:plc:bob", true),
            ],
        );
        assert_eq!(app.columns.focus, 0);
        app.handle_key(code(KeyCode::Right));
        app.handle_key(key('j'));
        let jobs = app.handle_key(key('l'));
        assert!(
            matches!(&jobs[..], [Job::Like { subject }] if subject.uri == "at://b/p/2"),
            "{jobs:?}"
        );
        app.handle_event(Event::Liked {
            post_uri: "at://b/p/2".into(),
            result: Ok("at://did:plc:me/app.bsky.feed.like/n".into()),
        });
        // Liked in both columns and on the Timeline tab.
        for c in &app.columns.items {
            let Rows::Posts(l) = &c.rows else { panic!() };
            assert!(l.items[1].like_uri().is_some(), "{:?}", c.source);
        }
        assert!(app.timeline.items[1].like_uri().is_some());
        // H goes back left; the first column's selection is where it was.
        app.handle_key(key('H'));
        assert_eq!(app.columns.focus, 0);
        assert_eq!(app.shown_post().unwrap().uri, "at://a/p/1");
    }

    #[test]
    fn a_page_lands_only_in_its_column_and_not_after_a_reload_or_a_removal() {
        let mut app = columns_with(
            &[columns::Source::Following, columns::Source::Following],
            vec![post("at://a/p/1", "did:plc:alice", true)],
        );
        let first = app.columns.items[0].id;
        let generation = app.columns.items[0].generation;
        // R asks again; the answer to the earlier ask is dropped.
        let jobs = app.handle_key(key('R'));
        assert!(
            matches!(&jobs[..], [Job::Column { id, .. }] if *id == first),
            "{jobs:?}"
        );
        app.handle_event(Event::Column {
            id: first,
            generation,
            cursor: None,
            result: Ok(MorePage::Posts(
                vec![post("at://z/p/9", "did:plc:z", true)].into(),
            )),
        });
        let Rows::Posts(l) = &app.columns.items[0].rows else {
            panic!()
        };
        assert!(!l.loaded, "still waiting for its own answer");
        let Rows::Posts(other) = &app.columns.items[1].rows else {
            panic!()
        };
        assert_eq!(other.items.len(), 1, "the other column is untouched");
        // Removed after x and y: its answer goes nowhere.
        app.handle_key(key('x'));
        app.handle_key(key('n'));
        assert_eq!(app.columns.items.len(), 2, "any key but y keeps it");
        app.handle_key(key('x'));
        app.handle_key(key('y'));
        assert_eq!(app.columns.items.len(), 1);
        app.handle_event(Event::Column {
            id: first,
            generation: generation + 1,
            cursor: None,
            result: Ok(MorePage::Posts(Vec::new().into())),
        });
        assert_eq!(app.columns.items.len(), 1);
        assert_ne!(app.columns.items[0].id, first);
    }

    #[test]
    fn a_deleted_post_leaves_every_column_and_each_account_has_its_own_columns() {
        let mine = post("at://did:plc:me/app.bsky.feed.post/m1", "did:plc:me", false);
        let mut app = columns_with(&[columns::Source::Following], vec![mine.clone()]);
        app.handle_key(key('D'));
        app.handle_key(key('y'));
        app.handle_event(Event::PostDeleted {
            uri: mine.uri.clone(),
            result: Ok(()),
        });
        let Rows::Posts(l) = &app.columns.items[0].rows else {
            panic!()
        };
        assert!(l.items.is_empty());
        // Another account has none of these columns.
        app.accounts = vec![Account::from(&session()), Account::from(&work())];
        app.switched_to(work());
        assert!(app.columns.items.is_empty());
        app.switched_to(session());
        assert_eq!(app.columns.sources(), [columns::Source::Following]);
    }

    fn work() -> Session {
        Session {
            did: "did:plc:work".into(),
            handle: "work.example".into(),
            ..session()
        }
    }

    /// Logged in as two accounts, using the first.
    fn two_accounts() -> App {
        let mut app = logged_in();
        app.accounts = vec![Account::from(&session()), Account::from(&work())];
        app
    }

    #[test]
    fn the_account_list_switches_to_another_account_and_drops_the_first_ones_lists() {
        let mut app = two_accounts();
        let reload = press(&mut app, key('R'))[0];
        app.handle_key(key('A'));
        assert!(matches!(
            app.overlay,
            Some(Overlay::Accounts { selected: 0 })
        ));
        // Enter on the one in use changes nothing.
        app.handle_key(code(KeyCode::Enter));
        assert_eq!(app.take_account_switch(), None);
        app.handle_key(key('A'));
        app.handle_key(key('j'));
        app.handle_key(code(KeyCode::Enter));
        assert_eq!(app.take_account_switch().as_deref(), Some("did:plc:work"));
        // The event loop gives the worker the session, then:
        let jobs = app.switched_to(work());
        assert!(
            matches!(
                jobs[..],
                [Job::Timeline, Job::Notifications, Job::PinnedFeeds]
            ),
            "{jobs:?}"
        );
        assert_eq!(app.session.as_ref().unwrap().did, "did:plc:work");
        assert!(
            app.timeline.items.is_empty(),
            "the first account's timeline is gone"
        );
        // A reload the first account asked for is not shown for the second.
        app.handle_answer(
            reload,
            Event::Timeline(Ok(vec![post("at://a/p/1", "did:plc:alice", true)].into())),
        );
        assert!(app.timeline.items.is_empty());
    }

    #[test]
    fn x_logs_an_account_out_only_after_a_y() {
        let mut app = two_accounts();
        app.handle_key(key('A'));
        app.handle_key(key('j'));
        app.handle_key(key('x'));
        app.handle_key(key('n'));
        assert_eq!(app.take_account_logout(), None);
        assert!(
            app.status
                .as_ref()
                .unwrap()
                .text
                .contains("still logged in")
        );
        app.handle_key(key('x'));
        app.handle_key(key('y'));
        assert_eq!(app.take_account_logout().as_deref(), Some("did:plc:work"));
        assert!(app.overlay.is_none());
        // Not the one in use: nothing else changes.
        let jobs = app.logged_out("did:plc:work", Some(session()));
        assert!(jobs.is_empty());
        assert_eq!(app.accounts, vec![Account::from(&session())]);
        assert_eq!(app.timeline.items.len(), 2);
    }

    #[test]
    fn logging_out_the_account_in_use_moves_to_the_next_or_to_the_login_form() {
        let mut app = two_accounts();
        let jobs = app.logged_out("did:plc:me", Some(work()));
        assert!(matches!(jobs[..], [Job::Timeline, ..]), "{jobs:?}");
        assert_eq!(app.session.as_ref().unwrap().did, "did:plc:work");
        assert!(
            app.status
                .as_ref()
                .unwrap()
                .text
                .contains("logged out @me.test; now @work.example")
        );
        let jobs = app.logged_out("did:plc:work", None);
        assert!(jobs.is_empty());
        assert!(app.session.is_none());
        let form = app.login.as_ref().expect("login form");
        assert!(
            !form.adding,
            "the last account out: esc quits, as at the start"
        );
    }

    #[test]
    fn another_account_is_logged_in_from_the_list_and_esc_goes_back() {
        let mut app = two_accounts();
        app.handle_key(key('A'));
        app.handle_key(key('a'));
        let form = app.login.as_ref().expect("login form");
        assert!(form.adding);
        assert_eq!(form.fields[0].text(), "https://pds.test");
        app.handle_key(code(KeyCode::Esc));
        assert!(app.login.is_none());
        assert!(!app.quit);
        // Logged in: the account joins the list, and is the one in use.
        app.handle_key(key('A'));
        app.handle_key(key('a'));
        app.handle_event(Event::LoggedIn(Ok(Session {
            did: "did:plc:third".into(),
            handle: "aaa.test".into(),
            ..session()
        })));
        let handles: Vec<_> = app.accounts.iter().map(|a| a.handle.as_str()).collect();
        assert_eq!(handles, ["aaa.test", "me.test", "work.example"]);
        assert_eq!(app.session.as_ref().unwrap().did, "did:plc:third");
    }

    fn expire(app: &mut App) {
        app.handle_event(Event::Timeline(Err(Error::api(
            "com.atproto.server.refreshSession failed: ExpiredToken: Token has expired",
        ))));
        assert!(app.login.is_some());
    }

    // D asks, and the question belongs to that moment: when the session
    // expires before the answer, the login form takes the keys, and the
    // first y after logging in again used to delete the post.
    #[test]
    fn a_delete_asked_before_the_session_expired_is_called_off() {
        let mine = post("at://did:plc:me/app.bsky.feed.post/m1", "did:plc:me", false);
        let (mut app, _) = App::new(Some(session()), "https://bsky.social");
        app.handle_event(Event::Timeline(Ok(vec![mine].into())));
        app.handle_key(key('D'));
        assert!(app.confirm_delete.is_some());
        expire(&mut app);
        app.handle_event(Event::LoggedIn(Ok(session())));
        let jobs = app.handle_key(key('y'));
        assert!(jobs.is_empty(), "deleted after logging in again: {jobs:?}");
    }

    // A theme being previewed is not chosen: when the picker is closed by
    // the session expiring, the theme goes back to the one in use, as Esc
    // would. It used to stay on the previewed one, which settings.json did
    // not hold.
    #[test]
    fn a_theme_previewed_when_the_session_expired_is_not_kept() {
        let mut app = logged_in();
        app.handle_key(key('T'));
        app.handle_key(key('j'));
        app.handle_key(key('j'));
        assert_eq!(app.theme_index, 2);
        expire(&mut app);
        assert!(app.overlay.is_none());
        assert_eq!(app.theme_index, 0);
        assert_eq!(app.theme.name, THEMES[0].name);
    }

    // The actions list is about the post it was opened on. When that post
    // leaves the list meanwhile (a reload without it), the selection moves
    // to another post, and enter used to act on that one.
    #[test]
    fn the_actions_list_does_not_act_on_a_post_it_was_not_opened_on() {
        let mut app = logged_in();
        app.handle_key(key('j'));
        app.handle_key(key('.'));
        assert!(matches!(app.overlay, Some(Overlay::Actions { .. })));
        // A reload that no longer has the second post.
        app.handle_event(Event::Timeline(Ok(vec![post(
            "at://a/p/1",
            "did:plc:alice",
            true,
        )]
        .into())));
        // Enter on "reply to it", and l, would act on at://a/p/1.
        let jobs = app.handle_key(key('l'));
        assert!(jobs.is_empty(), "{jobs:?}");
        assert!(app.overlay.is_none(), "{:?}", app.overlay);
        assert!(
            app.status
                .as_ref()
                .is_some_and(|s| s.text.contains("no longer")),
            "{:?}",
            app.status
        );
        // Opened again, it is about the post selected now, and works.
        app.handle_key(key('.'));
        let jobs = app.handle_key(key('l'));
        assert!(
            matches!(&jobs[..], [Job::Like { subject }] if subject.uri == "at://a/p/1"),
            "{jobs:?}"
        );
    }

    // The help, the theme picker, and the viewer hold nothing to keep, and a
    // playing video must not go on behind the login form.
    #[test]
    fn an_expired_session_closes_the_help_over_it() {
        let mut app = logged_in();
        app.overlay = Some(Overlay::Help { scroll: 0 });
        app.handle_event(Event::Timeline(Err(Error::api(
            "com.atproto.server.refreshSession failed: ExpiredToken: Token has expired",
        ))));
        assert!(app.login.is_some());
        assert!(app.overlay.is_none());
    }

    #[test]
    fn unfollow_removes_the_author_from_the_timeline() {
        let mut app = logged_in();
        let jobs = app.handle_key(key('f'));
        let Job::Unfollow { did, .. } = &jobs[0] else {
            panic!("{jobs:?}")
        };
        assert_eq!(did, "did:plc:alice");
        app.handle_event(Event::Unfollowed {
            did: "did:plc:alice".into(),
            result: Ok(()),
        });
        assert_eq!(app.timeline.items.len(), 1);
        assert_eq!(app.timeline.items[0].author.did, "did:plc:bob");
    }

    #[test]
    fn search_accounts_and_follow() {
        let mut app = logged_in();
        app.handle_key(key('/'));
        app.handle_key(ctrl('t'));
        type_str(&mut app, "carol");
        let jobs = app.handle_key(code(KeyCode::Enter));
        assert!(matches!(&jobs[..], [Job::SearchActors(q)] if q == "carol"));
        let carol: Profile =
            serde_json::from_value(json!({"did": "did:plc:carol", "handle": "carol.test"}))
                .unwrap();
        app.handle_event(Event::SearchActors {
            query: "carol".into(),
            result: Ok(vec![carol].into()),
        });
        let jobs = app.handle_key(key('f'));
        assert!(matches!(&jobs[..], [Job::Follow { did }] if did == "did:plc:carol"));
        app.handle_event(Event::Followed {
            did: "did:plc:carol".into(),
            result: Ok("at://f".into()),
        });
        assert_eq!(app.search.actors.items[0].following_uri(), Some("at://f"));
        let jobs = app.handle_key(code(KeyCode::Enter));
        assert!(matches!(&jobs[..], [Job::OpenProfile(a)] if a == "did:plc:carol"));
        assert_eq!(app.tab, Tab::Profile);
    }

    #[test]
    fn typing_in_the_search_box_does_not_trigger_commands() {
        let mut app = logged_in();
        app.handle_key(key('/'));
        type_str(&mut app, "q l f");
        assert!(!app.quit);
        assert_eq!(app.search.input.text(), "q l f");
    }

    #[test]
    fn following_yourself_is_refused() {
        let mut app = logged_in();
        let jobs = app.handle_key(key('4'));
        assert!(matches!(&jobs[..], [Job::OpenProfile(a)] if a == "did:plc:me"));
        let me: Profile =
            serde_json::from_value(json!({"did": "did:plc:me", "handle": "me.test"})).unwrap();
        app.handle_event(Event::Profile(Ok((me, vec![].into()))));
        assert!(app.handle_key(key('f')).is_empty());
        assert!(app.status.as_ref().unwrap().error);
    }

    #[test]
    fn profile_editor_loads_then_saves_the_fields() {
        let mut app = logged_in();
        app.handle_key(key('4'));
        let jobs = app.handle_key(key('e'));
        assert!(matches!(jobs[..], [Job::LoadProfileEditor]));
        app.handle_event(Event::ProfileEditor(Ok(
            crate::tui::worker::ProfileFields {
                display_name: "Me".into(),
                description: "old bio".into(),
            },
        )));
        type_str(&mut app, " Myself");
        app.handle_key(code(KeyCode::Tab));
        app.handle_key(code(KeyCode::Enter));
        type_str(&mut app, "line2");
        let jobs = app.handle_key(ctrl('s'));
        match &jobs[..] {
            [
                Job::SaveProfile {
                    display_name,
                    description,
                    avatar,
                },
            ] => {
                assert_eq!(display_name, "Me Myself");
                assert_eq!(description, "old bio\nline2");
                assert_eq!(avatar, &None);
            }
            other => panic!("{other:?}"),
        }
        let jobs = app.handle_event(Event::ProfileSaved(Ok(())));
        assert!(app.overlay.is_none());
        assert!(matches!(&jobs[..], [Job::OpenProfile(_)]));
    }

    #[test]
    fn editing_someone_elses_profile_is_refused() {
        let mut app = logged_in();
        app.handle_key(code(KeyCode::Enter));
        assert_eq!(app.profile.actor.as_deref(), Some("did:plc:alice"));
        assert!(app.handle_key(key('e')).is_empty());
        assert!(app.status.as_ref().unwrap().error);
    }

    #[test]
    fn esc_goes_back_to_the_search_the_profile_was_opened_from() {
        let mut app = logged_in();
        app.handle_key(key('2'));
        app.handle_key(ctrl('t'));
        type_str(&mut app, "carol");
        app.handle_key(code(KeyCode::Enter));
        let actors: Vec<Profile> = ["carol", "dave"]
            .iter()
            .map(|n| {
                serde_json::from_value(
                    json!({"did": format!("did:plc:{n}"), "handle": format!("{n}.test")}),
                )
                .unwrap()
            })
            .collect();
        app.handle_event(Event::SearchActors {
            query: "carol".into(),
            result: Ok(actors.into()),
        });
        app.handle_key(key('j'));
        let jobs = app.handle_key(code(KeyCode::Enter));
        assert!(matches!(&jobs[..], [Job::OpenProfile(a)] if a == "did:plc:dave"));
        assert_eq!(app.tab, Tab::Profile);
        // R reloads the profile and still remembers where it came from.
        app.handle_key(key('R'));
        assert_eq!(app.profile.came_from, Some(Tab::Search));
        let jobs = app.handle_key(code(KeyCode::Esc));
        assert!(jobs.is_empty(), "going back fetches nothing: {jobs:?}");
        assert_eq!(app.tab, Tab::Search);
        assert_eq!(app.search.input.text(), "carol");
        assert_eq!(
            app.search.actors.selected, 1,
            "the selection is where it was"
        );
        assert!(!app.search.editing, "back to the results, not to typing");
        // The Profile tab shows the user's own profile next time.
        let jobs = app.handle_key(key('4'));
        assert!(matches!(&jobs[..], [Job::OpenProfile(a)] if a == "did:plc:me"));
    }

    #[test]
    fn esc_goes_back_to_the_timeline_too() {
        let mut app = logged_in();
        app.handle_key(key('j'));
        app.handle_key(code(KeyCode::Enter));
        assert!(app.handle_key(code(KeyCode::Esc)).is_empty());
        assert_eq!(app.tab, Tab::Timeline);
        assert_eq!(app.timeline.selected, 1);
    }

    #[test]
    fn choosing_a_tab_forgets_the_way_back() {
        let mut app = logged_in();
        app.handle_key(code(KeyCode::Enter)); // alice, from the timeline
        app.handle_key(key('1'));
        app.handle_key(key('4'));
        assert_eq!(app.profile.came_from, None);
        // Esc on someone else's profile with nowhere to go back to: your own.
        let jobs = app.handle_key(code(KeyCode::Esc));
        assert!(matches!(&jobs[..], [Job::OpenProfile(a)] if a == "did:plc:me"));
    }

    #[test]
    fn pending_counts_jobs_in_flight() {
        let mut app = logged_in();
        // The notifications and the pinned feeds, loading in the
        // background since the start.
        assert_eq!(app.pending, 2);
        app.handle_key(key('l'));
        assert_eq!(app.pending, 3);
        app.handle_event(Event::Liked {
            post_uri: "at://a/p/1".into(),
            result: Err(Error::api("x")),
        });
        assert_eq!(app.pending, 2);
    }

    #[test]
    fn a_second_like_press_waits_for_the_first_answer() {
        let mut app = logged_in();
        assert_eq!(app.handle_key(key('l')).len(), 1);
        // The answer has not arrived: a second press must not create a second record.
        assert!(app.handle_key(key('l')).is_empty());
        app.handle_event(Event::Liked {
            post_uri: "at://a/p/1".into(),
            result: Err(Error::api("boom")),
        });
        // Answered (even with an error): the post can be liked again.
        assert_eq!(app.handle_key(key('l')).len(), 1);
    }

    #[test]
    fn a_second_follow_press_waits_for_the_first_answer() {
        let mut app = logged_in();
        assert_eq!(app.handle_key(key('f')).len(), 1);
        assert!(app.handle_key(key('f')).is_empty());
        app.handle_event(Event::Unfollowed {
            did: "did:plc:alice".into(),
            result: Ok(()),
        });
        // Alice's post left the timeline; f now acts on Bob.
        assert!(
            matches!(&app.handle_key(key('f'))[..], [Job::Unfollow { did, .. }] if did == "did:plc:bob")
        );
    }

    #[test]
    fn a_late_editor_answer_does_not_overwrite_typing() {
        let mut app = logged_in();
        app.handle_key(key('4'));
        app.handle_key(key('e'));
        let fields = || crate::tui::worker::ProfileFields {
            display_name: "Server".into(),
            description: String::new(),
        };
        app.handle_event(Event::ProfileEditor(Ok(fields())));
        app.handle_key(ctrl('u'));
        type_str(&mut app, "Typed");
        // A second answer (from an earlier, abandoned editor) arrives late.
        app.handle_event(Event::ProfileEditor(Ok(fields())));
        let Some(Overlay::EditProfile(e)) = &app.overlay else {
            panic!()
        };
        assert_eq!(e.fields[0].text(), "Typed");
    }

    #[test]
    fn an_expired_refresh_token_brings_back_the_login_form() {
        let mut app = logged_in();
        app.handle_event(Event::Timeline(Err(Error::api(
            "com.atproto.server.refreshSession failed: ExpiredToken: Token has expired",
        ))));
        let form = app.login.as_ref().expect("login form");
        assert_eq!(form.fields[0].text(), "https://pds.test");
        assert_eq!(form.fields[1].text(), "me.test");
        assert_eq!(form.focus, 2);
        assert!(form.error.as_deref().unwrap().contains("expired"));
    }

    /// The theme picker goes back to the settings screen only when the
    /// settings screen opened it. A picker opened from the settings that
    /// was closed another way (here the session expiring) used to leave
    /// that behind, and the next T then Esc opened the settings screen on
    /// whatever tab was shown.
    #[test]
    fn a_picker_opened_with_t_closes_to_the_list_after_one_from_the_settings() {
        let mut app = logged_in();
        app.handle_key(key('4'));
        app.handle_key(key('s'));
        app.handle_key(code(KeyCode::Enter));
        assert!(matches!(app.overlay, Some(Overlay::Themes { .. })));
        app.handle_event(Event::Timeline(Err(Error::api(
            "com.atproto.server.refreshSession failed: ExpiredToken: Token has expired",
        ))));
        assert!(app.overlay.is_none());
        app.handle_event(Event::LoggedIn(Ok(session())));
        app.handle_key(key('1'));
        app.handle_key(key('T'));
        app.handle_key(code(KeyCode::Esc));
        assert!(app.overlay.is_none(), "{:?}", app.overlay);
        app.handle_key(key('T'));
        app.handle_key(code(KeyCode::Enter));
        assert!(app.overlay.is_none(), "{:?}", app.overlay);
    }

    #[test]
    fn a_failed_search_settles_even_after_leaving_the_tab() {
        let mut app = logged_in();
        app.handle_key(key('/'));
        type_str(&mut app, "x");
        app.handle_key(code(KeyCode::Enter));
        assert!(!app.search.posts.loaded);
        app.handle_key(key('1'));
        app.handle_event(Event::SearchPosts {
            query: "x".into(),
            result: Err(Error::api("boom")),
        });
        assert!(app.search.posts.loaded);
    }

    #[test]
    fn a_failed_profile_says_why_and_a_stale_one_is_dropped() {
        let mut app = logged_in();
        app.handle_key(code(KeyCode::Enter)); // alice
        app.handle_event(Event::Profile(Err(Error::api("gone"))));
        assert_eq!(app.profile.error.as_deref(), Some("gone"));
        // Bob's profile, opened earlier, answering late, is not shown as Alice's.
        let bob: Profile =
            serde_json::from_value(json!({"did": "did:plc:bob", "handle": "bob.test"})).unwrap();
        app.handle_event(Event::Profile(Ok((bob, vec![].into()))));
        assert!(app.profile.profile.is_none());
        let alice: Profile =
            serde_json::from_value(json!({"did": "did:plc:alice", "handle": "alice.test"}))
                .unwrap();
        app.handle_event(Event::Profile(Ok((alice, vec![].into()))));
        assert_eq!(app.profile.profile.as_ref().unwrap().did, "did:plc:alice");
        assert!(app.profile.error.is_none());
    }

    #[rstest::rstest]
    #[case::digit(KeyEvent::from(KeyCode::Char('2')))]
    #[case::tab(KeyEvent::from(KeyCode::Tab))]
    #[case::slash(KeyEvent::from(KeyCode::Char('/')))]
    fn arriving_at_an_empty_search_tab_types_into_the_box(#[case] arrive: KeyEvent) {
        let mut app = logged_in();
        app.handle_key(arrive);
        assert_eq!(app.tab, Tab::Search);
        // q, l and f would quit, like and follow on a result list.
        type_str(&mut app, "q l f");
        assert!(!app.quit);
        assert_eq!(app.search.input.text(), "q l f");
        let jobs = app.handle_key(code(KeyCode::Enter));
        assert!(matches!(&jobs[..], [Job::SearchPosts(q)] if q == "q l f"));
        // After Enter the results have the keys: j moves, it is not typed.
        app.handle_event(Event::SearchPosts {
            query: "q l f".into(),
            result: Ok(vec![
                post("at://s/1", "did:plc:x", false),
                post("at://s/2", "did:plc:y", false),
            ]
            .into()),
        });
        app.handle_key(key('j'));
        assert_eq!(app.search.posts.selected, 1);
        assert_eq!(app.search.input.text(), "q l f");
    }

    #[test]
    fn backtab_arrives_at_search_focused_too() {
        let mut app = logged_in();
        app.handle_key(code(KeyCode::BackTab)); // Timeline -> Columns
        app.handle_key(code(KeyCode::BackTab)); // Columns -> Profile
        app.handle_key(code(KeyCode::BackTab)); // Profile -> Notifications
        app.handle_key(code(KeyCode::BackTab)); // Notifications -> Search
        assert_eq!(app.tab, Tab::Search);
        assert!(app.search.editing);
    }

    #[test]
    fn coming_back_to_a_search_with_results_leaves_the_keys_to_the_results() {
        let mut app = logged_in();
        app.handle_key(key('2'));
        type_str(&mut app, "rust");
        app.handle_key(code(KeyCode::Enter));
        app.handle_key(key('1'));
        app.handle_key(key('2'));
        assert!(!app.search.editing, "j/k must move through the results");
        // i (or /) goes back to typing.
        app.handle_key(key('i'));
        assert!(app.search.editing);
        type_str(&mut app, "!");
        assert_eq!(app.search.input.text(), "rust!");
    }

    #[test]
    fn tab_leaves_the_search_box_for_the_next_tab() {
        let mut app = logged_in();
        app.handle_key(key('2'));
        type_str(&mut app, "abc");
        app.handle_key(code(KeyCode::Tab));
        assert_eq!(app.tab, Tab::Notifications);
        assert!(!app.search.editing);
        assert_eq!(app.search.input.text(), "abc");
    }

    #[test]
    fn paste_reaches_the_focused_search_box() {
        let mut app = logged_in();
        app.handle_key(key('2'));
        app.handle_paste("pasted words");
        assert_eq!(app.search.input.text(), "pasted words");
    }

    #[test]
    fn tabs_wrap_in_both_directions() {
        assert_eq!(Tab::Timeline.next(-1), *Tab::ALL.last().unwrap());
        assert_eq!(Tab::ALL.last().unwrap().next(1), Tab::Timeline);
    }

    #[test]
    fn status_messages_expire_errors_later() {
        let mut app = logged_in();
        app.info("liked");
        let at = app.status.as_ref().unwrap().at;
        assert!(!app.expire_status(at + STATUS_TTL - Duration::from_millis(1)));
        assert!(app.expire_status(at + STATUS_TTL));
        assert!(app.status.is_none());
        app.error("boom");
        let at = app.status.as_ref().unwrap().at;
        assert!(!app.expire_status(at + STATUS_TTL));
        assert!(app.expire_status(at + ERROR_TTL));
        // Nothing to expire: the screen does not change.
        assert!(!app.expire_status(at + ERROR_TTL));
    }

    #[test]
    fn help_scrolls_and_only_closes_on_purpose() {
        let mut app = logged_in();
        app.handle_key(key('?'));
        app.handle_key(key('j'));
        app.handle_key(key('j'));
        app.handle_key(key('k'));
        assert!(matches!(app.overlay, Some(Overlay::Help { scroll: 1 })));
        // A stray key is not a request to close.
        app.handle_key(key('x'));
        assert!(app.overlay.is_some());
        app.handle_key(code(KeyCode::Esc));
        assert!(app.overlay.is_none());
        app.handle_key(key('?'));
        app.handle_key(key('?'));
        assert!(app.overlay.is_none());
    }

    #[test]
    fn the_theme_picker_previews_and_esc_goes_back() {
        let mut app = logged_in();
        app.handle_key(key('T'));
        app.handle_key(key('j'));
        app.handle_key(key('j'));
        assert_eq!(app.theme.name, THEMES[2].name, "moving previews the theme");
        app.handle_key(code(KeyCode::Esc));
        assert!(app.overlay.is_none());
        assert_eq!(app.theme_index, 0);
        assert!(
            app.take_settings_save().is_none(),
            "cancelling saves nothing"
        );
    }

    #[test]
    fn enter_in_the_picker_saves_the_theme_and_keeps_other_settings() {
        let mut app = logged_in();
        let mut settings = Settings::default();
        settings.other.insert("future".into(), json!(1));
        app.apply_settings(settings, ColorDepth::TrueColor, None);
        app.handle_key(key('T'));
        app.handle_key(key('k')); // wraps to the last theme
        app.handle_key(code(KeyCode::Enter));
        let saved = app.take_settings_save().expect("settings to save");
        assert_eq!(saved.theme.as_deref(), Some(THEMES[THEMES.len() - 1].name));
        assert_eq!(saved.other.get("future"), Some(&json!(1)));
        app.settings_saved(Ok(()));
        assert!(app.status.as_ref().unwrap().text.contains("monochrome"));
    }

    #[test]
    fn a_saved_theme_is_used_and_an_unknown_one_warns() {
        let mut app = logged_in();
        let settings = Settings {
            theme: Some("Nord".into()),
            ..Settings::default()
        };
        app.apply_settings(settings, ColorDepth::TrueColor, None);
        assert_eq!(app.theme.name, "nord");
        let settings = Settings {
            theme: Some("neon".into()),
            ..Settings::default()
        };
        app.apply_settings(settings, ColorDepth::TrueColor, None);
        assert_eq!(app.theme.name, "bluesky");
        assert!(
            app.status
                .as_ref()
                .unwrap()
                .text
                .contains("unknown theme \"neon\"")
        );
    }

    #[test]
    fn no_color_keeps_monochrome_and_refuses_the_picker() {
        let mut app = logged_in();
        let settings = Settings {
            theme: Some("dracula".into()),
            ..Settings::default()
        };
        app.apply_settings(settings, ColorDepth::None, None);
        assert!(app.theme.mono);
        app.handle_key(key('T'));
        assert!(app.overlay.is_none());
        assert!(app.status.as_ref().unwrap().text.contains("NO_COLOR"));
    }

    /// The settings screen, on the row named `name`.
    fn settings_on(app: &mut App, name: &str) {
        app.handle_key(key('4'));
        app.handle_key(key('s'));
        let at = app
            .settings_rows()
            .iter()
            .position(|r| r.name == name)
            .expect("a row of that name");
        for _ in 0..at {
            app.handle_key(key('j'));
        }
        assert!(
            matches!(app.overlay, Some(Overlay::Settings { selected, .. }) if selected == at),
            "{:?}",
            app.overlay
        );
    }

    #[test]
    fn s_opens_the_settings_only_on_your_own_profile() {
        let mut app = logged_in();
        // Not on another tab: s is nothing there.
        app.handle_key(key('s'));
        assert!(app.overlay.is_none());
        app.handle_key(key('4'));
        app.handle_key(key('s'));
        assert!(matches!(
            app.overlay,
            Some(Overlay::Settings { selected: 0, .. })
        ));
        app.handle_key(code(KeyCode::Esc));
        assert!(app.overlay.is_none());
        // Someone else's profile has no settings.
        app.open_profile(Some("did:plc:alice".into()));
        app.handle_key(key('s'));
        assert!(app.overlay.is_none());
    }

    #[test]
    fn the_settings_screen_names_every_setting() {
        let mut app = logged_in();
        let names: Vec<_> = app.settings_rows().iter().map(|r| r.name).collect();
        assert_eq!(
            names,
            [
                "Theme",
                "Pictures",
                "Download folder",
                "Picture cache",
                "Video service",
                "Browser"
            ]
        );
        app.handle_key(key('4'));
        app.handle_key(key('s'));
        // Moving wraps both ways.
        app.handle_key(key('k'));
        assert!(matches!(
            app.overlay,
            Some(Overlay::Settings { selected: 5, .. })
        ));
        app.handle_key(key('j'));
        assert!(matches!(
            app.overlay,
            Some(Overlay::Settings { selected: 0, .. })
        ));
    }

    #[test]
    fn pictures_off_is_kept_and_asked_of_the_event_loop() {
        let mut app = logged_in();
        let mut settings = Settings::default();
        settings.other.insert("future".into(), json!(1));
        app.apply_settings(settings, ColorDepth::TrueColor, None);
        settings_on(&mut app, "Pictures");
        app.handle_key(code(KeyCode::Enter));
        assert!(!app.pictures, "the lists are text at once");
        assert_eq!(app.take_pictures_change(), Some(false));
        assert_eq!(app.take_pictures_change(), None);
        let saved = app.take_settings_save().expect("settings to save");
        assert_eq!(saved.pictures.as_deref(), Some("off"));
        assert_eq!(saved.other.get("future"), Some(&json!(1)));
        app.settings_saved(Ok(()));
        assert_eq!(app.status.as_ref().unwrap().text, "pictures: off");
        // The screen stays open, and says what it is now.
        assert!(matches!(
            app.overlay,
            Some(Overlay::Settings { selected: 1, .. })
        ));
        assert_eq!(app.settings_rows()[1].value, "off");

        // Back to auto: the event loop finds out whether the terminal
        // draws them, and says.
        app.handle_key(key(' '));
        assert_eq!(app.take_pictures_change(), Some(true));
        let saved = app.take_settings_save().expect("settings to save");
        assert_eq!(saved.pictures.as_deref(), Some("auto"));
        app.pictures_back(true);
        assert!(app.pictures);
        app.pictures_back(false);
        assert!(!app.pictures);
        assert!(
            app.status
                .as_ref()
                .unwrap()
                .text
                .contains("cannot show pictures")
        );
    }

    #[test]
    fn a_setting_the_environment_fixes_is_shown_and_not_changed() {
        let mut app = logged_in();
        app.env = crate::config::Environment {
            graphics: Some("kitty".into()),
            download_dir: Some("/tmp/写真👨\u{200d}👩\u{200d}👧".into()),
            ..Default::default()
        };
        let rows = app.settings_rows();
        assert_eq!(rows[1].note, "set by BSKY_GRAPHICS for this run");
        assert!(!rows[1].editable);
        assert_eq!(rows[2].value, "/tmp/写真👨\u{200d}👩\u{200d}👧");
        assert_eq!(rows[2].note, "set by BSKY_DOWNLOAD_DIR for this run");
        settings_on(&mut app, "Pictures");
        app.handle_key(code(KeyCode::Enter));
        assert!(app.pictures);
        assert_eq!(app.take_pictures_change(), None);
        assert!(app.take_settings_save().is_none());
        assert!(app.status.as_ref().unwrap().text.contains("BSKY_GRAPHICS"));
    }

    #[test]
    fn the_theme_row_opens_the_picker_and_comes_back_to_the_settings() {
        let mut app = logged_in();
        settings_on(&mut app, "Theme");
        app.handle_key(code(KeyCode::Enter));
        assert!(matches!(app.overlay, Some(Overlay::Themes { .. })));
        app.handle_key(key('j'));
        app.handle_key(code(KeyCode::Enter));
        assert!(matches!(
            app.overlay,
            Some(Overlay::Settings { selected: 0, .. })
        ));
        let saved = app.take_settings_save().expect("settings to save");
        assert_eq!(saved.theme.as_deref(), Some(THEMES[1].name));
        assert_eq!(app.settings_rows()[0].value, THEMES[1].name);
        // Esc in the picker comes back too, with the theme as it was.
        app.handle_key(code(KeyCode::Enter));
        app.handle_key(key('j'));
        app.handle_key(code(KeyCode::Esc));
        assert!(matches!(
            app.overlay,
            Some(Overlay::Settings { selected: 0, .. })
        ));
        assert_eq!(app.theme_index, 1);
        // T on its own still closes to the list.
        app.handle_key(code(KeyCode::Esc));
        app.handle_key(key('T'));
        app.handle_key(code(KeyCode::Esc));
        assert!(app.overlay.is_none());
    }

    /// A picture post, open in the viewer.
    fn viewing_a_picture(app: &mut App) {
        app.handle_key(key('1'));
        app.handle_event(Event::Timeline(Ok(vec![with_pictures(
            "at://did:plc:alice/app.bsky.feed.post/p1",
            json!({"$type": "app.bsky.embed.images#view", "images": [
                {"thumb": "https://t/1", "fullsize": "https://f/1", "alt": ""}]}),
        )]
        .into())));
        app.handle_key(key(' '));
        assert!(app.viewer_open());
    }

    #[test]
    fn a_folder_chosen_on_the_screen_is_kept_and_d_saves_there() {
        let dir = tempfile::tempdir().unwrap();
        let chosen = dir.path().join("写真👨\u{200d}👩\u{200d}👧 🇯🇵");
        std::fs::create_dir(&chosen).unwrap();
        let mut app = logged_in();
        let mut settings = Settings::default();
        settings.other.insert("future".into(), json!(1));
        app.apply_settings(settings, ColorDepth::TrueColor, None);
        app.browse_from = Some(dir.path().to_path_buf());
        settings_on(&mut app, "Download folder");
        app.handle_key(code(KeyCode::Enter));
        let Some(Overlay::Settings {
            edit: Some(SettingEdit::Folder(b)),
            ..
        }) = &mut app.overlay
        else {
            panic!("{:?}", app.overlay)
        };
        // Into the folder, then choose it.
        b.dir = dir.path().to_path_buf();
        app.handle_key(code(KeyCode::Char('~')));
        let Some(Overlay::Settings {
            edit: Some(SettingEdit::Folder(b)),
            ..
        }) = &mut app.overlay
        else {
            panic!()
        };
        **b = Browser::folder(&chosen);
        app.handle_key(key(' '));
        assert!(
            matches!(
                app.overlay,
                Some(Overlay::Settings {
                    selected: 2,
                    edit: None
                })
            ),
            "{:?}",
            app.overlay
        );
        let want = std::path::absolute(&chosen).unwrap();
        let saved = app.take_settings_save().expect("settings to save");
        assert_eq!(saved.download_dir, Some(want.display().to_string()));
        assert_eq!(saved.other.get("future"), Some(&json!(1)));
        app.settings_saved(Ok(()));
        assert!(
            app.status
                .as_ref()
                .unwrap()
                .text
                .starts_with("download folder: "),
            "{:?}",
            app.status
        );
        let row = &app.settings_rows()[2];
        assert_eq!(row.value, want.display().to_string());
        assert!(row.resettable);
        assert!(
            row.note.contains("x goes back to the default"),
            "{}",
            row.note
        );
        // The next d saves into it.
        app.handle_key(code(KeyCode::Esc));
        viewing_a_picture(&mut app);
        let jobs = app.handle_key(key('d'));
        assert!(
            matches!(&jobs[..], [Job::Download { dir: Some(d), .. }] if *d == want),
            "{jobs:?}"
        );
        // x puts it back.
        app.handle_key(code(KeyCode::Esc));
        settings_on(&mut app, "Download folder");
        app.handle_key(key('x'));
        let saved = app.take_settings_save().expect("settings to save");
        assert_eq!(saved.download_dir, None);
        assert!(!app.settings_rows()[2].resettable);
    }

    #[test]
    fn a_folder_that_cannot_be_written_is_refused_and_the_browser_stays() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a-file");
        std::fs::write(&file, "x").unwrap();
        let mut app = logged_in();
        settings_on(&mut app, "Picture cache");
        app.handle_key(code(KeyCode::Enter));
        let Some(Overlay::Settings {
            edit: Some(SettingEdit::Folder(b)),
            ..
        }) = &mut app.overlay
        else {
            panic!()
        };
        // A folder under a file cannot be made.
        b.dir = file.join("sub");
        app.handle_key(key(' '));
        assert!(matches!(
            app.overlay,
            Some(Overlay::Settings {
                edit: Some(SettingEdit::Folder(_)),
                ..
            })
        ));
        assert!(app.take_settings_save().is_none());
        assert!(
            app.status
                .as_ref()
                .is_some_and(|s| s.error && s.text.starts_with("cannot write to ")),
            "{:?}",
            app.status
        );
        // Esc leaves the browser for the list, with nothing changed.
        app.handle_key(code(KeyCode::Esc));
        assert!(matches!(
            app.overlay,
            Some(Overlay::Settings {
                selected: 3,
                edit: None
            })
        ));
    }

    #[test]
    fn a_new_picture_cache_rebuilds_the_pictures() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = logged_in();
        settings_on(&mut app, "Picture cache");
        app.handle_key(code(KeyCode::Enter));
        let Some(Overlay::Settings {
            edit: Some(SettingEdit::Folder(b)),
            ..
        }) = &mut app.overlay
        else {
            panic!()
        };
        b.dir = dir.path().to_path_buf();
        app.handle_key(key(' '));
        assert_eq!(app.cache_dir(), Some(dir.path().to_path_buf()));
        assert_eq!(app.take_pictures_change(), Some(true));
        // Without pictures there is nothing to rebuild.
        app.pictures = false;
        app.handle_key(key('x'));
        assert_eq!(app.take_pictures_change(), None);
    }

    #[test]
    fn the_video_service_and_the_browser_are_typed_and_go_with_their_jobs() {
        let mut app = logged_in();
        settings_on(&mut app, "Video service");
        app.handle_key(code(KeyCode::Enter));
        // The field starts with what is set now.
        let Some(Overlay::Settings {
            edit: Some(SettingEdit::Text(input)),
            ..
        }) = &app.overlay
        else {
            panic!()
        };
        assert_eq!(input.text(), crate::api::DEFAULT_VIDEO_SERVICE);
        app.handle_key(ctrl('u'));
        type_str(&mut app, "ftp://nope");
        app.handle_key(code(KeyCode::Enter));
        assert!(app.status.as_ref().is_some_and(|s| s.error), "refused");
        assert!(app.take_settings_save().is_none());
        app.handle_key(ctrl('u'));
        app.handle_paste("https://video.example/");
        app.handle_key(code(KeyCode::Enter));
        let saved = app.take_settings_save().expect("settings to save");
        assert_eq!(
            saved.video_service.as_deref(),
            Some("https://video.example/")
        );
        // Kept without the slash, as the variable is.
        assert_eq!(app.settings_rows()[4].value, "https://video.example");

        app.handle_key(key('j'));
        app.handle_key(code(KeyCode::Enter));
        type_str(&mut app, "my browser 🦊");
        app.handle_key(code(KeyCode::Enter));
        let saved = app.take_settings_save().expect("settings to save");
        assert_eq!(saved.browser.as_deref(), Some("my browser 🦊"));
        app.handle_key(code(KeyCode::Esc));

        // The jobs carry them.
        app.handle_key(key('1'));
        app.handle_event(Event::Timeline(Ok(vec![post(
            "at://did:plc:bob/app.bsky.feed.post/p2",
            "did:plc:bob",
            true,
        )]
        .into())));
        let jobs = app.handle_key(key('o'));
        assert!(
            matches!(&jobs[..], [Job::OpenLink { browser: Some(b), .. }] if b == "my browser 🦊"),
            "{jobs:?}"
        );
        app.handle_key(key('n'));
        type_str(&mut app, "hello");
        let jobs = app.handle_key(ctrl('s'));
        assert!(
            matches!(&jobs[..], [Job::Post { video_service, .. }] if video_service == "https://video.example"),
            "{jobs:?}"
        );

        app.handle_event(Event::Posted {
            reply_to: None,
            result: Ok(()),
        });

        // Esc in the field keeps what was there; an empty field is the
        // default.
        settings_on(&mut app, "Browser");
        app.handle_key(code(KeyCode::Enter));
        app.handle_key(ctrl('u'));
        app.handle_key(code(KeyCode::Esc));
        assert!(app.take_settings_save().is_none());
        assert_eq!(app.settings_rows()[5].value, "my browser 🦊");
        app.handle_key(code(KeyCode::Enter));
        app.handle_key(ctrl('u'));
        app.handle_key(code(KeyCode::Enter));
        let saved = app.take_settings_save().expect("settings to save");
        assert_eq!(saved.browser, None);
        assert_eq!(
            app.settings_rows()[5].value,
            crate::browser::system_opener()
        );
    }

    #[test]
    fn a_setting_a_variable_fixes_cannot_be_changed_or_reset() {
        let mut app = logged_in();
        app.env.browser = Some("firefox".into());
        app.settings.browser = Some("chromium".into());
        let row = &app.settings_rows()[5];
        assert_eq!(row.value, "firefox");
        assert!(!row.editable && !row.resettable);
        settings_on(&mut app, "Browser");
        app.handle_key(code(KeyCode::Enter));
        app.handle_key(key('x'));
        assert!(matches!(
            app.overlay,
            Some(Overlay::Settings { edit: None, .. })
        ));
        assert!(app.take_settings_save().is_none());
        assert_eq!(app.browser().as_deref(), Some("firefox"));
    }

    #[test]
    fn pictures_are_not_saved_over_an_unreadable_settings_file() {
        let mut app = logged_in();
        app.apply_settings(
            Settings::default(),
            ColorDepth::TrueColor,
            Some("settings.json is not valid and was ignored".into()),
        );
        settings_on(&mut app, "Pictures");
        app.handle_key(code(KeyCode::Enter));
        // Off for this run, and the file is left as it is.
        assert!(!app.pictures);
        assert_eq!(app.take_pictures_change(), Some(false));
        assert!(app.take_settings_save().is_none());
        assert!(
            app.status
                .as_ref()
                .unwrap()
                .text
                .contains("for this session only")
        );
    }

    fn page(posts: Vec<Post>, cursor: Option<&str>) -> Page<Post> {
        Page {
            items: posts,
            cursor: cursor.map(str::to_string),
        }
    }

    fn timeline_with(n: usize, cursor: Option<&str>) -> App {
        let (mut app, _) = App::new(Some(session()), "x");
        let posts = (0..n)
            .map(|i| post(&format!("at://p/{i}"), "did:plc:a", true))
            .collect();
        app.handle_event(Event::Timeline(Ok(page(posts, cursor))));
        app
    }

    #[test]
    fn nothing_more_is_fetched_on_load_or_far_from_the_end() {
        let mut app = timeline_with(MORE_AHEAD + 3, Some("c1"));
        assert_eq!(
            app.pending, 2,
            "only the notifications and the pinned feeds, in the background"
        );
        assert!(app.handle_key(key('j')).is_empty());
        assert!(app.handle_key(key('j')).is_empty());
    }

    #[test]
    fn nearing_the_end_fetches_the_next_page_once() {
        let mut app = timeline_with(MORE_AHEAD + 2, Some("c1"));
        assert!(app.handle_key(key('j')).is_empty());
        let jobs = app.handle_key(key('j')); // MORE_AHEAD from the end
        assert!(
            matches!(&jobs[..], [Job::More { feed: Feed::Timeline, cursor }] if cursor == "c1")
        );
        // While it is on its way, moving on asks for nothing more.
        assert!(app.handle_key(key('j')).is_empty());
        app.handle_event(Event::More {
            feed: Feed::Timeline,
            cursor: "c1".into(),
            result: Ok(MorePage::Posts(page(
                vec![
                    post(&format!("at://p/{}", MORE_AHEAD + 1), "did:plc:a", true),
                    post("at://p/new", "did:plc:a", true),
                ],
                Some("c2"),
            ))),
        });
        // The first was already there: only one is new; the selection did not move.
        assert_eq!(app.timeline.items.len(), MORE_AHEAD + 3);
        assert_eq!(app.timeline.selected, 3);
        assert_eq!(app.timeline.cursor.as_deref(), Some("c2"));
    }

    #[rstest::rstest]
    #[case::no_cursor(None)]
    #[case::same_cursor(Some("c1"))]
    fn the_list_ends_when_the_cursor_stops_moving(#[case] next: Option<&str>) {
        let mut app = timeline_with(3, Some("c1"));
        let jobs = app.handle_key(key('j'));
        assert_eq!(jobs.len(), 1);
        app.handle_event(Event::More {
            feed: Feed::Timeline,
            cursor: "c1".into(),
            result: Ok(MorePage::Posts(page(vec![], next))),
        });
        assert_eq!(app.timeline.cursor, None);
        assert!(app.handle_key(key('j')).is_empty());
        assert!(app.handle_key(key('G')).is_empty());
    }

    #[test]
    fn a_page_for_a_refreshed_list_is_dropped() {
        let mut app = timeline_with(3, Some("c1"));
        app.handle_key(key('j'));
        // R replaced the list before the old page came back.
        app.handle_event(Event::Timeline(Ok(page(
            vec![post("at://fresh", "did:plc:a", true)],
            Some("new"),
        ))));
        app.handle_event(Event::More {
            feed: Feed::Timeline,
            cursor: "c1".into(),
            result: Ok(MorePage::Posts(page(
                vec![post("at://old", "did:plc:a", true)],
                None,
            ))),
        });
        let uris: Vec<&str> = app.timeline.items.iter().map(|p| p.uri.as_str()).collect();
        assert_eq!(uris, ["at://fresh"]);
        assert_eq!(app.timeline.cursor.as_deref(), Some("new"));
    }

    #[test]
    fn a_failed_page_can_be_tried_again() {
        let mut app = timeline_with(2, Some("c1"));
        assert_eq!(app.handle_key(key('j')).len(), 1);
        app.handle_event(Event::More {
            feed: Feed::Timeline,
            cursor: "c1".into(),
            result: Err(Error::api("boom")),
        });
        assert!(app.status.as_ref().unwrap().error);
        assert_eq!(app.handle_key(key('k')).len(), 1, "the next move retries");
    }

    #[test]
    fn a_page_for_an_old_query_is_dropped() {
        let mut app = logged_in();
        app.handle_key(key('/'));
        type_str(&mut app, "old");
        app.handle_key(code(KeyCode::Enter));
        app.handle_event(Event::SearchPosts {
            query: "old".into(),
            result: Ok(page(vec![post("at://s/1", "did:plc:x", false)], Some("c1"))),
        });
        assert_eq!(app.handle_key(key('j')).len(), 1);
        // A new search starts before the page arrives.
        app.handle_key(key('/'));
        app.handle_key(ctrl('u'));
        type_str(&mut app, "new");
        app.handle_key(code(KeyCode::Enter));
        app.handle_event(Event::More {
            feed: Feed::SearchPosts("old".into()),
            cursor: "c1".into(),
            result: Ok(MorePage::Posts(page(
                vec![post("at://s/2", "did:plc:x", false)],
                None,
            ))),
        });
        assert_eq!(app.search.posts.items.len(), 1);
    }

    #[test]
    fn repost_toggles_and_waits_for_the_answer() {
        let mut app = logged_in();
        let jobs = app.handle_key(key('b'));
        let [Job::Repost { subject }] = &jobs[..] else {
            panic!("{jobs:?}")
        };
        assert_eq!(subject.uri, "at://a/p/1");
        assert!(app.handle_key(key('b')).is_empty(), "second press waits");
        app.handle_event(Event::Reposted {
            post_uri: "at://a/p/1".into(),
            result: Ok("at://did:plc:me/app.bsky.feed.repost/r1".into()),
        });
        assert_eq!(app.timeline.items[0].repost_count, 1);
        let jobs = app.handle_key(key('b'));
        assert!(
            matches!(&jobs[..], [Job::Unrepost { repost_uri, .. }] if repost_uri.ends_with("/r1"))
        );
        app.handle_event(Event::Unreposted {
            post_uri: "at://a/p/1".into(),
            result: Ok(()),
        });
        assert_eq!(app.timeline.items[0].repost_count, 0);
        assert!(app.timeline.items[0].repost_uri().is_none());
    }

    fn thread_json(focus: &str, replies: &[&str]) -> crate::api::types::ThreadNode {
        let node = |uri: &str| {
            json!({"$type": "app.bsky.feed.defs#threadViewPost",
                   "post": {"uri": uri, "cid": format!("cid-{uri}"), "author": {"did": "did:plc:z", "handle": "z.test"}, "record": {"text": uri}},
                   "replies": []})
        };
        let mut v = node(focus);
        v["parent"] = node("at://parent");
        v["replies"] = json!(replies.iter().map(|r| node(r)).collect::<Vec<_>>());
        serde_json::from_value(v).unwrap()
    }

    fn with_pictures(uri: &str, embed: serde_json::Value) -> Post {
        let mut p = post(uri, "did:plc:alice", true);
        p.embed = serde_json::from_value(embed).ok();
        p
    }

    // o always leads somewhere: the post's link where it has one, and the
    // post itself on bsky.app where it has none, which is where its
    // replies and its author's other posts are.
    #[test]
    fn o_opens_the_link_or_the_post_itself() {
        let mut app = logged_in();
        app.handle_event(Event::Timeline(Ok(vec![
            with_pictures(
                "at://did:plc:alice/app.bsky.feed.post/p1",
                json!({"$type": "app.bsky.embed.external#view",
                       "external": {"uri": "https://example.com/a", "title": "A", "description": ""}}),
            ),
            post("at://did:plc:bob/app.bsky.feed.post/p2", "did:plc:bob", true),
        ]
        .into())));
        let jobs = app.handle_key(key('o'));
        assert!(
            matches!(&jobs[..], [Job::OpenLink { url: u, .. }] if u == "https://example.com/a"),
            "{jobs:?}"
        );
        app.handle_key(key('j'));
        let jobs = app.handle_key(key('o'));
        assert!(
            matches!(&jobs[..], [Job::OpenLink { url: u, .. }] if u == "https://bsky.app/profile/did:plc:bob/post/p2"),
            "{jobs:?}"
        );
        // Space still opens the viewer, and says so when there is nothing
        // to view: it does not send the reader to a browser.
        let jobs = app.handle_key(key(' '));
        assert!(jobs.is_empty(), "{jobs:?}");
        assert!(
            app.status
                .as_ref()
                .is_some_and(|s| s.text.contains("no pictures, video, or link")),
            "{:?}",
            app.status
        );
    }

    // A terminal that cannot show pictures has no viewer: space opens the
    // post on bsky.app, where they can be seen, and a link as before.
    #[test]
    fn without_pictures_space_opens_the_post_in_the_browser() {
        let mut app = logged_in();
        app.without_pictures();
        assert!(
            app.status
                .as_ref()
                .is_some_and(|s| s.text.contains("cannot show pictures")),
            "{:?}",
            app.status
        );
        app.handle_event(Event::Timeline(Ok(vec![
            with_pictures(
                "at://did:plc:alice/app.bsky.feed.post/p1",
                json!({"$type": "app.bsky.embed.images#view", "images": [{"thumb": "https://t/1", "fullsize": "https://f/1", "alt": ""}]}),
            ),
            with_pictures(
                "at://did:plc:alice/app.bsky.feed.post/p2",
                json!({"$type": "app.bsky.embed.external#view", "external": {"uri": "https://example.com/a", "title": "A", "description": ""}}),
            ),
        ]
        .into())));
        let jobs = app.handle_key(key(' '));
        assert!(
            matches!(&jobs[..], [Job::OpenLink { url: u, .. }] if u == "https://bsky.app/profile/did:plc:alice/post/p1"),
            "{jobs:?}"
        );
        assert!(app.overlay.is_none());
        app.handle_key(key('j'));
        let jobs = app.handle_key(key(' '));
        assert!(
            matches!(&jobs[..], [Job::OpenLink { url: u, .. }] if u == "https://example.com/a"),
            "{jobs:?}"
        );
    }

    #[test]
    fn without_pictures_the_keys_say_what_space_does() {
        let mut app = logged_in();
        assert!(crate::tui::keys::hints(&app).contains(&("space", "view")));
        app.without_pictures();
        let hints = crate::tui::keys::hints(&app);
        assert!(hints.contains(&("space", "open in browser")), "{hints:?}");
        assert!(!hints.contains(&("space", "view")));
        let help = crate::tui::keys::help(false);
        assert!(help.iter().all(|(title, _)| *title != "Viewer"));
        let posts = &help.iter().find(|(t, _)| *t == "Posts").unwrap().1;
        assert!(
            posts
                .iter()
                .any(|(k, d)| *k == "space" && d.contains("web browser"))
        );
        assert!(
            crate::tui::keys::help(true)
                .iter()
                .any(|(t, _)| *t == "Viewer")
        );
    }

    #[test]
    fn v_opens_the_thread_and_esc_closes_it() {
        let mut app = logged_in();
        let jobs = app.handle_key(key('v'));
        assert!(matches!(&jobs[..], [Job::Thread(u)] if u == "at://a/p/1"));
        app.handle_event(Event::Thread {
            uri: "at://a/p/1".into(),
            result: Ok(thread_json("at://a/p/1", &["at://r1", "at://r2"])),
        });
        let th = app.threads.last().unwrap();
        assert_eq!(th.list.items.len(), 4);
        assert_eq!(
            th.list.selected, 1,
            "the opened post is selected, below its parent"
        );
        // Keys act on the thread: j moves to the first reply and l likes it.
        app.handle_key(key('j'));
        let jobs = app.handle_key(key('l'));
        assert!(matches!(&jobs[..], [Job::Like { subject }] if subject.uri == "at://r1"));
        app.handle_event(Event::Liked {
            post_uri: "at://r1".into(),
            result: Ok("at://like".into()),
        });
        let liked = app.threads.last().unwrap().list.items[2]
            .post()
            .unwrap()
            .clone();
        assert_eq!(liked.like_count, 1);
        app.handle_key(code(KeyCode::Esc));
        assert!(app.threads.is_empty());
        assert_eq!(app.tab, Tab::Timeline);
        assert_eq!(app.timeline.selected, 0, "the timeline is where it was");
    }

    #[test]
    fn a_thread_inside_a_thread_stacks() {
        let mut app = logged_in();
        app.handle_key(key('v'));
        app.handle_event(Event::Thread {
            uri: "at://a/p/1".into(),
            result: Ok(thread_json("at://a/p/1", &["at://r1"])),
        });
        app.handle_key(key('j'));
        let jobs = app.handle_key(key('v'));
        assert!(matches!(&jobs[..], [Job::Thread(u)] if u == "at://r1"));
        assert_eq!(app.threads.len(), 2);
        app.handle_key(code(KeyCode::Esc));
        assert_eq!(app.threads.len(), 1);
    }

    #[test]
    fn a_late_thread_answer_is_dropped_and_a_tab_switch_closes_threads() {
        let mut app = logged_in();
        app.handle_key(key('v'));
        app.handle_key(code(KeyCode::Esc));
        // The answer for the thread already closed changes nothing.
        app.handle_event(Event::Thread {
            uri: "at://a/p/1".into(),
            result: Ok(thread_json("at://a/p/1", &[])),
        });
        assert!(app.threads.is_empty());
        app.handle_key(key('v'));
        app.handle_key(key('2'));
        assert!(app.threads.is_empty());
    }

    // was: R set the selection back to the focused post, so the next key
    // acted on it instead of the reply that was picked.
    #[test]
    fn reloading_a_thread_keeps_the_selected_reply() {
        let mut app = logged_in();
        app.handle_key(key('v'));
        app.handle_event(Event::Thread {
            uri: "at://a/p/1".into(),
            result: Ok(thread_json("at://a/p/1", &["at://r1", "at://r2"])),
        });
        app.handle_key(key('j'));
        app.handle_key(key('j'));
        assert_eq!(app.threads[0].list.selected, 3, "the second reply");
        app.handle_key(key('R'));
        app.handle_event(Event::Thread {
            uri: "at://a/p/1".into(),
            result: Ok(thread_json("at://a/p/1", &["at://r1", "at://r2"])),
        });
        assert_eq!(app.threads[0].list.selected, 3);
        let jobs = app.handle_key(key('l'));
        assert!(
            matches!(&jobs[..], [Job::Like { subject }] if subject.uri == "at://r2"),
            "{jobs:?}"
        );
    }

    #[test]
    fn reloading_a_thread_whose_selected_reply_is_gone_goes_back_to_the_post() {
        let mut app = logged_in();
        app.handle_key(key('v'));
        app.handle_event(Event::Thread {
            uri: "at://a/p/1".into(),
            result: Ok(thread_json("at://a/p/1", &["at://r1", "at://r2"])),
        });
        app.handle_key(key('j'));
        app.handle_key(key('j'));
        app.handle_key(key('R'));
        app.handle_event(Event::Thread {
            uri: "at://a/p/1".into(),
            result: Ok(thread_json("at://a/p/1", &["at://r1"])),
        });
        assert_eq!(
            app.threads[0].list.selected, 1,
            "the opened post, below its parent"
        );
    }

    #[test]
    fn a_failed_thread_says_why_and_r_retries() {
        let mut app = logged_in();
        app.handle_key(key('v'));
        app.handle_event(Event::Thread {
            uri: "at://a/p/1".into(),
            result: Err(Error::api("NotFound: Post not found")),
        });
        assert_eq!(
            app.threads[0].error.as_deref(),
            Some("NotFound: Post not found")
        );
        let jobs = app.handle_key(key('R'));
        assert!(matches!(&jobs[..], [Job::Thread(u)] if u == "at://a/p/1"));
    }

    fn notif(
        reason: &str,
        uri: &str,
        read: bool,
        post: Option<Post>,
        subject: Option<Post>,
    ) -> NotifItem {
        NotifItem {
            n: serde_json::from_value(json!({
                "uri": uri, "cid": "c", "reason": reason, "isRead": read,
                "author": {"did": format!("did:plc:{reason}"), "handle": format!("{reason}.test")},
                "indexedAt": "2026-09-22T00:00:00.000Z"
            }))
            .unwrap(),
            fresh: !read,
            post,
            subject,
        }
    }

    fn notifications_tab() -> App {
        let mut app = logged_in();
        // Loading since the start: arriving asks for nothing more.
        assert!(app.handle_key(key('3')).is_empty());
        let reply = post("at://reply/1", "did:plc:reply", false);
        let mine = post("at://me/post", "did:plc:me", false);
        let jobs = app.handle_event(Event::Notifications {
            seen_at: "2026-09-22T01:00:00.000Z".into(),
            result: Ok(Page {
                items: vec![
                    notif("reply", "at://reply/1", false, Some(reply), None),
                    notif("like", "at://like/1", false, None, Some(mine)),
                    notif("follow", "at://follow/1", true, None, None),
                ],
                cursor: Some("n1".into()),
            }),
        });
        // Two were unread: they are marked seen, as of the time of the fetch.
        assert!(matches!(&jobs[..], [Job::UpdateSeen(at)] if at == "2026-09-22T01:00:00.000Z"));
        assert_eq!(app.unread, 2);
        app
    }

    #[test]
    fn notifications_load_once_and_are_marked_seen() {
        let mut app = notifications_tab();
        app.handle_event(Event::Seen(Ok(())));
        assert_eq!(app.unread, 0);
        assert!(app.notifications.items.iter().all(|i| i.n.is_read));
        // The markers stay for this visit.
        assert!(app.notifications.items[0].fresh);
        // Coming back does not fetch again; R does.
        app.handle_key(key('1'));
        assert!(app.handle_key(key('3')).is_empty());
        assert!(matches!(
            &app.handle_key(key('R'))[..],
            [Job::Notifications]
        ));
    }

    #[test]
    fn nothing_unread_sends_no_update_seen() {
        let mut app = logged_in();
        app.handle_key(key('3'));
        let jobs = app.handle_event(Event::Notifications {
            seen_at: "t".into(),
            result: Ok(vec![notif("follow", "at://f", true, None, None)].into()),
        });
        assert!(jobs.is_empty());
    }

    #[test]
    fn a_reply_notification_can_be_answered_and_liked() {
        let mut app = notifications_tab();
        let jobs = app.handle_key(key('r'));
        assert!(jobs.is_empty());
        let Some(Overlay::Compose(c)) = &app.overlay else {
            panic!("no composer")
        };
        assert_eq!(c.reply.as_ref().unwrap().0.parent.uri, "at://reply/1");
        app.handle_key(code(KeyCode::Esc));
        let jobs = app.handle_key(key('l'));
        assert!(matches!(&jobs[..], [Job::Like { subject }] if subject.uri == "at://reply/1"));
    }

    #[test]
    fn a_like_notification_opens_the_liked_post_and_its_author() {
        let mut app = notifications_tab();
        app.handle_key(key('j'));
        // Nothing to like or answer on a like: it is not a post.
        assert!(app.handle_key(key('l')).is_empty());
        let jobs = app.handle_key(key('v'));
        assert!(matches!(&jobs[..], [Job::Thread(u)] if u == "at://me/post"));
        app.handle_key(code(KeyCode::Esc));
        let jobs = app.handle_key(code(KeyCode::Enter));
        assert!(matches!(&jobs[..], [Job::OpenProfile(a)] if a == "did:plc:like"));
        assert_eq!(app.profile.came_from, Some(Tab::Notifications));
        app.handle_key(code(KeyCode::Esc));
        assert_eq!(app.tab, Tab::Notifications);
    }

    #[test]
    fn notifications_page_like_every_other_list() {
        let mut app = notifications_tab();
        // Three items: the first move is already near the end.
        let jobs = app.handle_key(key('j'));
        assert!(
            matches!(&jobs[..], [Job::More { feed: Feed::Notifications, cursor }] if cursor == "n1")
        );
        app.handle_event(Event::More {
            feed: Feed::Notifications,
            cursor: "n1".into(),
            result: Ok(MorePage::Notifications(Page {
                items: vec![notif("mention", "at://m/1", true, None, None)],
                cursor: None,
            })),
        });
        assert_eq!(app.notifications.items.len(), 4);
        assert_eq!(app.notifications.cursor, None);
    }

    #[test]
    fn selection_is_clamped_to_the_list() {
        let mut app = logged_in();
        for _ in 0..5 {
            app.handle_key(key('j'));
        }
        assert_eq!(app.timeline.selected, 1);
        app.handle_key(key('g'));
        assert_eq!(app.timeline.selected, 0);
        app.handle_key(key('G'));
        assert_eq!(app.timeline.selected, 1);
    }

    #[test]
    fn each_search_list_pages_with_its_own_query() {
        let mut app = logged_in();
        app.handle_key(key('/'));
        type_str(&mut app, "a");
        app.handle_key(code(KeyCode::Enter));
        app.handle_event(Event::SearchPosts {
            query: "a".into(),
            result: Ok(page(vec![post("at://s/1", "did:plc:x", false)], Some("pa"))),
        });
        // Accounts for "b", then back to the posts for "a" without searching.
        app.handle_key(key('/'));
        app.handle_key(ctrl('t'));
        app.handle_key(ctrl('u'));
        type_str(&mut app, "b");
        app.handle_key(code(KeyCode::Enter));
        app.handle_event(Event::SearchActors {
            query: "b".into(),
            result: Ok(Vec::new().into()),
        });
        app.handle_key(key('/'));
        app.handle_key(ctrl('t'));
        app.handle_key(code(KeyCode::Esc));
        let jobs = app.handle_key(key('j'));
        assert!(
            matches!(&jobs[..], [Job::More { feed: Feed::SearchPosts(q), cursor }] if q == "a" && cursor == "pa"),
            "{jobs:?}"
        );
        app.handle_event(Event::More {
            feed: Feed::SearchPosts("a".into()),
            cursor: "pa".into(),
            result: Ok(MorePage::Posts(page(
                vec![post("at://s/2", "did:plc:x", false)],
                None,
            ))),
        });
        assert_eq!(app.search.posts.items.len(), 2);
    }

    #[test]
    fn logging_in_as_someone_else_forgets_the_last_account() {
        let mut app = notifications_tab();
        app.handle_key(key('v'));
        app.fail(&Error::api(
            "com.atproto.server.refreshSession failed: ExpiredToken",
        ));
        let other = Session {
            did: "did:plc:other".into(),
            handle: "other.test".into(),
            ..session()
        };
        let jobs = app.handle_event(Event::LoggedIn(Ok(other)));
        assert!(matches!(
            &jobs[..],
            [Job::Timeline, Job::Notifications, Job::PinnedFeeds]
        ));
        assert!(app.notifications.items.is_empty());
        assert!(!app.notifications.loaded);
        assert_eq!(app.unread, 0);
        assert!(app.threads.is_empty());
        assert!(app.timeline.items.is_empty());
        assert_eq!(app.tab, Tab::Timeline);
        // The same account again keeps what was loaded.
        let mut app = notifications_tab();
        app.handle_event(Event::LoggedIn(Ok(session())));
        assert_eq!(app.notifications.items.len(), 3);
    }

    #[test]
    fn a_thread_reloading_under_another_still_takes_its_answer() {
        let mut app = logged_in();
        app.handle_key(key('v'));
        app.handle_event(Event::Thread {
            uri: "at://a/p/1".into(),
            result: Ok(thread_json("at://a/p/1", &["at://r1"])),
        });
        app.handle_key(key('R'));
        app.handle_key(key('j'));
        app.handle_key(key('v'));
        assert_eq!(app.threads.len(), 2);
        app.handle_event(Event::Thread {
            uri: "at://a/p/1".into(),
            result: Ok(thread_json("at://a/p/1", &["at://r1"])),
        });
        assert!(app.threads[0].list.loaded);
        app.handle_key(code(KeyCode::Esc));
        assert_eq!(app.threads[0].list.items.len(), 3);
    }

    #[test]
    fn an_unreadable_settings_file_is_never_overwritten() {
        let mut app = logged_in();
        app.apply_settings(
            Settings::default(),
            ColorDepth::TrueColor,
            Some("settings.json is not valid and was ignored".into()),
        );
        app.handle_key(key('T'));
        app.handle_key(key('j'));
        app.handle_key(code(KeyCode::Enter));
        assert!(app.take_settings_save().is_none());
        assert_eq!(app.theme_index, 1, "used for this session");
        let status = app.status.as_ref().unwrap();
        assert!(
            status.error && status.text.contains("not overwritten"),
            "{status:?}"
        );
    }

    #[test]
    fn a_failed_first_page_is_kept_as_the_reason() {
        let mut app = logged_in();
        app.handle_key(key('3'));
        app.handle_event(Event::Notifications {
            seen_at: "t".into(),
            result: Err(Error::api("listNotifications failed: boom")),
        });
        assert_eq!(
            app.notifications.error.as_deref(),
            Some("listNotifications failed: boom")
        );
        app.handle_key(key('R'));
        assert!(app.notifications.error.is_none());
    }

    #[test]
    fn a_tab_still_loading_is_not_asked_for_again() {
        let mut app = logged_in();
        // The notifications are on their way since the start.
        assert!(app.handle_key(key('3')).is_empty());
        app.handle_key(key('1'));
        assert!(app.handle_key(key('3')).is_empty());
        assert_eq!(app.handle_key(key('4')).len(), 1);
        app.handle_key(key('1'));
        assert!(app.handle_key(key('4')).is_empty());
    }

    #[test]
    fn search_keys_do_not_reach_behind_a_thread() {
        let mut app = logged_in();
        app.handle_key(key('/'));
        type_str(&mut app, "q");
        app.handle_key(code(KeyCode::Enter));
        app.handle_event(Event::SearchPosts {
            query: "q".into(),
            result: Ok(page(vec![post("at://s/1", "did:plc:x", false)], None)),
        });
        app.handle_key(key('v'));
        app.handle_key(key('i'));
        assert!(!app.search.editing);
        assert!(app.handle_key(key('t')).is_empty());
        assert_eq!(app.search.mode, SearchMode::Posts);
    }

    /// A folder with two pictures and a subfolder, for the picture browser.
    fn pictures() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        for name in ["a.png", "b.png"] {
            image::RgbImage::from_pixel(4, 3, image::Rgb([1, 2, 3]))
                .save(dir.path().join(name))
                .unwrap();
        }
        dir
    }

    fn composer(app: &App) -> &Compose {
        match &app.overlay {
            Some(Overlay::Compose(c)) => c,
            other => panic!("no composer: {other:?}"),
        }
    }

    /// A composer with both pictures of `dir` attached.
    fn composer_with_pictures(dir: &tempfile::TempDir) -> App {
        let mut app = logged_in();
        app.browse_from = Some(dir.path().to_path_buf());
        app.handle_key(key('n'));
        app.handle_key(ctrl('o'));
        let b = composer(&app).browser.as_ref().expect("browser");
        assert_eq!(b.room, MAX_POST_IMAGES);
        assert_eq!(b.current().unwrap().name, "sub");
        app.handle_key(key('j'));
        app.handle_key(key(' '));
        app.handle_key(key(' '));
        app.handle_key(code(KeyCode::Enter));
        assert!(composer(&app).browser.is_none());
        assert_eq!(composer(&app).media.len(), 2);
        app
    }

    #[test]
    fn pictures_are_chosen_described_and_sent_without_text() {
        let dir = pictures();
        let mut app = composer_with_pictures(&dir);
        assert_eq!(app.browse_from.as_deref(), Some(dir.path()), "remembered");
        app.handle_key(code(KeyCode::Tab));
        type_str(&mut app, "first");
        app.handle_key(code(KeyCode::Tab));
        app.handle_paste("second");
        // Tab comes back round to the text, which stays empty.
        app.handle_key(code(KeyCode::Tab));
        assert_eq!(composer(&app).focus, 0);
        let jobs = app.handle_key(ctrl('s'));
        let [Job::Post { text, media, .. }] = &jobs[..] else {
            panic!("{jobs:?}")
        };
        assert_eq!(text, "");
        assert_eq!(
            media,
            &[
                Attachment {
                    path: dir.path().join("a.png"),
                    alt: "first".into()
                },
                Attachment {
                    path: dir.path().join("b.png"),
                    alt: "second".into()
                },
            ]
        );
    }

    #[test]
    fn ctrl_x_removes_the_picture_being_described_or_the_last() {
        let dir = pictures();
        let mut app = composer_with_pictures(&dir);
        app.handle_key(code(KeyCode::Tab));
        app.handle_key(ctrl('x'));
        let c = composer(&app);
        assert_eq!(c.media.len(), 1);
        assert!(c.media[0].path.ends_with("b.png"));
        assert_eq!(c.focus, 1, "on the picture that moved up");
        app.handle_key(code(KeyCode::BackTab));
        app.handle_key(ctrl('x'));
        assert!(composer(&app).media.is_empty());
        assert!(
            app.handle_key(ctrl('x')).is_empty(),
            "nothing left to remove"
        );
        // Without pictures or text there is nothing to post.
        assert!(app.handle_key(ctrl('s')).is_empty());
        assert_eq!(app.status.as_ref().unwrap().text, "the post is empty");
    }

    #[test]
    fn a_fifth_picture_is_refused_and_the_browser_holds_the_keys() {
        let dir = pictures();
        let mut app = composer_with_pictures(&dir);
        app.handle_key(ctrl('o'));
        assert_eq!(composer(&app).browser.as_ref().unwrap().room, 2);
        // Typing and pasting do not reach the post behind the browser.
        app.handle_paste("hidden");
        app.handle_key(key('z'));
        app.handle_key(code(KeyCode::Esc));
        assert!(composer(&app).browser.is_none());
        assert!(composer(&app).input.is_empty());
        let Some(Overlay::Compose(c)) = &mut app.overlay else {
            unreachable!()
        };
        let one = c.media[0].clone();
        c.media.extend([one.clone(), one]);
        app.handle_key(ctrl('o'));
        assert!(composer(&app).browser.is_none());
        assert!(
            app.status
                .as_ref()
                .unwrap()
                .text
                .contains("at most 4 pictures")
        );
    }

    #[test]
    fn the_avatar_is_chosen_in_the_browser() {
        let dir = pictures();
        let mut app = logged_in();
        app.browse_from = Some(dir.path().to_path_buf());
        app.overlay = Some(Overlay::EditProfile(EditProfile {
            fields: [
                TextInput::single("Me"),
                TextInput::multi(""),
                TextInput::single(""),
            ],
            focus: 0,
            loading: false,
            saving: false,
            browser: None,
            avatar_chosen: None,
        }));
        app.handle_key(ctrl('o'));
        app.handle_key(key('G'));
        app.handle_key(code(KeyCode::Enter));
        let Some(Overlay::EditProfile(e)) = &app.overlay else {
            panic!()
        };
        assert!(e.browser.is_none());
        assert_eq!(e.focus, 2);
        assert_eq!(
            e.fields[2].text(),
            dir.path().join("b.png").display().to_string()
        );
    }

    fn testdata(name: &str) -> PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("e2e/atago/testdata")
            .join(name)
    }

    #[test]
    fn one_video_goes_alone_and_nothing_joins_it() {
        let mut app = logged_in();
        app.handle_key(key('n'));
        let Some(Overlay::Compose(c)) = &mut app.overlay else {
            unreachable!()
        };
        c.media.push(Attached::new(testdata("clip.mp4")));
        assert!(c.media[0].is_video());
        assert_eq!(c.media[0].info.dims, Some((64, 36)));
        app.handle_key(ctrl('o'));
        assert!(composer(&app).browser.is_none());
        assert!(
            app.status
                .as_ref()
                .unwrap()
                .text
                .contains("a video can have nothing else")
        );
        let jobs = app.handle_key(ctrl('s'));
        let [Job::Post { media, .. }] = &jobs[..] else {
            panic!("{jobs:?}")
        };
        assert_eq!(media[0].path, testdata("clip.mp4"));
    }

    #[test]
    fn pictures_and_a_video_cannot_share_a_post() {
        let photo = Attached::new(testdata("photo.png"));
        let clip = Attached::new(testdata("clip.mp4"));
        let gif = Attached::new(testdata("moving.gif"));
        assert!(gif.is_video() && gif.info.animated_gif);
        assert_eq!(media_problem(std::slice::from_ref(&clip)), None);
        assert_eq!(media_problem(&[photo.clone(), photo.clone()]), None);
        let mixed = media_problem(&[photo.clone(), clip.clone()]).unwrap();
        assert!(mixed.contains("up to 4 pictures or one video"), "{mixed}");
        assert!(media_problem(&[clip.clone(), gif]).is_some());
        let five = vec![photo; 5];
        assert!(media_problem(&five).unwrap().contains("at most 4 pictures"));
    }

    #[test]
    fn notifications_loaded_at_start_are_seen_only_when_their_tab_is() {
        let mut app = logged_in();
        let jobs = app.handle_event(Event::Notifications {
            seen_at: "2026-09-22T01:00:00.000Z".into(),
            result: Ok(vec![notif("reply", "at://r", false, None, None)].into()),
        });
        assert!(jobs.is_empty(), "not seen from the timeline: {jobs:?}");
        assert_eq!(app.unread, 1, "but counted on the tab");
        let jobs = app.handle_key(key('3'));
        assert!(matches!(&jobs[..], [Job::UpdateSeen(at)] if at == "2026-09-22T01:00:00.000Z"));
        app.handle_key(key('1'));
        assert!(app.handle_key(key('3')).is_empty(), "marked once");
    }

    #[test]
    fn a_background_failure_waits_on_its_tab() {
        let mut app = logged_in();
        app.handle_event(Event::Notifications {
            seen_at: "t".into(),
            result: Err(Error::api("listNotifications failed: boom")),
        });
        assert!(app.status.is_none(), "the timeline is not interrupted");
        assert!(app.notifications.error.is_some());
        // An expired session is still reported at once.
        app.handle_event(Event::Notifications {
            seen_at: "t".into(),
            result: Err(Error::api(
                "com.atproto.server.refreshSession failed: ExpiredToken",
            )),
        });
        assert!(app.login.is_some());
    }

    #[test]
    fn a_sent_post_reloads_the_timeline_so_it_shows() {
        let mut app = logged_in();
        app.handle_key(key('n'));
        type_str(&mut app, "hello");
        app.handle_key(ctrl('s'));
        let jobs = app.handle_event(Event::Posted {
            reply_to: None,
            result: Ok(()),
        });
        assert!(matches!(&jobs[..], [Job::Timeline]), "{jobs:?}");
        assert!(app.overlay.is_none());
    }
}
