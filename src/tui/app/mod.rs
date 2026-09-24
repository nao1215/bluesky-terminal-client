//! UI state and what every key does. Nothing here touches the terminal or
//! the network: keys and finished jobs go in, [`Job`]s come out.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::api::post_length_problem;
use crate::api::types::{ChatMessage, Convo};
use crate::api::types::{Media, Post, Profile, ReplyRef, StrongRef};
use crate::config::{Environment, Session, Settings};
use crate::error::Error;
use crate::media::{self, MAX_POST_IMAGES};
use crate::tui::chat::{self, ChatPane, OpenConvo};
use crate::tui::columns::{self, Columns, Rows};
use crate::tui::files::{Action, Browser};
use crate::tui::input::TextInput;
use crate::tui::keys;
use crate::tui::theme::{self, ColorDepth, THEMES, Theme};
use crate::tui::thread::{self as thread_rows, RowKind, ThreadRow};
use crate::tui::worker::{Attachment, Event, Feed, Job, MorePage, NotifItem, Page};

mod accounts;
mod actions;
mod answers;
mod deck;
mod key_map;
mod messages;
mod selection;
mod settings;
#[cfg(test)]
mod tests;

/// The top-level views. `Columns` is the Timeline tab showing columns side
/// by side, once one has been added with `+`; it is not a tab of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Timeline,
    Search,
    Profile,
    Notifications,
    Columns,
    Chat,
}

impl Tab {
    /// The tabs, in the order of their keys 1 to 5.
    pub const ALL: [Tab; 5] = [
        Tab::Timeline,
        Tab::Chat,
        Tab::Search,
        Tab::Notifications,
        Tab::Profile,
    ];

    pub fn title(self) -> &'static str {
        match self.shown_as() {
            Tab::Search => "Search",
            Tab::Profile => "Profile",
            Tab::Notifications => "Notifications",
            Tab::Chat => "Chat",
            _ => "Timeline",
        }
    }

    /// The tab this view belongs to: the columns are the Timeline tab's.
    pub fn shown_as(self) -> Tab {
        match self {
            Tab::Columns => Tab::Timeline,
            t => t,
        }
    }

    fn index(self) -> usize {
        let tab = self.shown_as();
        Self::ALL.iter().position(|t| *t == tab).unwrap()
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

impl Keyed for Convo {
    fn key(&self) -> &str {
        &self.id
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

    /// Put a first page read again at the top of the list, and keep what
    /// was loaded after it: the items further down, the cursor that goes on
    /// from them, and the selection wherever it was.
    fn renew(&mut self, page: Page<T>) {
        let fresh: HashSet<String> = page.items.iter().map(|i| i.key().to_string()).collect();
        let selected = self.current().map(|i| i.key().to_string());
        // A first page without a cursor is the whole list. One that shares
        // nothing with the list is not its top either: posts between them
        // would never be loaded, so it starts the list again.
        let whole = page.cursor.is_none() || !self.items.iter().any(|i| fresh.contains(i.key()));
        let rest: Vec<T> = std::mem::take(&mut self.items)
            .into_iter()
            .filter(|i| !whole && !fresh.contains(i.key()))
            .collect();
        let (cursor, pending) = if rest.is_empty() {
            (page.cursor, false)
        } else {
            (self.cursor.take(), self.more_pending)
        };
        self.items = page.items.into_iter().chain(rest).collect();
        self.cursor = cursor;
        self.more_pending = pending;
        self.selected = selected
            .and_then(|k| self.items.iter().position(|i| i.key() == k))
            .unwrap_or(0);
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

impl List<Post> {
    /// Leave out a post a reply above it already shows as the post it
    /// answers or its thread's first: the same author and text twice in a
    /// row, as a self-thread brings. The reply, newer, is what stays.
    fn drop_shown_above(&mut self) {
        let mut shown: HashSet<String> = HashSet::new();
        let keep: Vec<bool> = self
            .items
            .iter()
            .map(|p| {
                let kept = !shown.contains(&p.uri);
                if let Some(c) = &p.context {
                    shown.extend(c.parent.uri().map(str::to_string));
                    shown.extend(c.root.as_ref().and_then(|r| r.uri()).map(str::to_string));
                }
                kept
            })
            .collect();
        if keep.iter().all(|k| *k) {
            return;
        }
        let mut at = 0;
        self.retain(|_| {
            at += 1;
            keep[at - 1]
        });
    }
}

impl<T> List<T> {
    /// A first page has been asked for. A next page still on its way is
    /// of the list before: it is not waited for, and if the first page
    /// fails, the next one can be asked for again.
    fn begin(&mut self) {
        self.loaded = false;
        self.loading = true;
        self.error = None;
        self.more_pending = false;
    }

    /// The next page from `requested` could not be loaded: the next move
    /// asks again. A failure of a page asked for before the list was loaded
    /// again leaves the page awaited now.
    fn more_failed(&mut self, requested: &str) {
        if self.cursor.as_deref() == Some(requested) {
            self.more_pending = false;
        }
    }

    /// The first page could not be loaded; what was there stays.
    fn failed(&mut self, e: &Error) {
        self.loaded = true;
        self.loading = false;
        self.error = Some(e.message().to_string());
    }

    /// Put `item` at the top, the selection and the scroll staying on what
    /// they were on.
    fn push_front(&mut self, item: T) {
        if !self.items.is_empty() {
            self.selected += 1;
            self.offset += 1;
        }
        self.items.insert(0, item);
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

    /// Keep the items `keep` says to. The selection and the scroll stay on
    /// the item they were on, or the next one when it went: counted by
    /// position, one taken out above them would move them onto another.
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
    /// The display name and description as the editor showed them when
    /// they came: a field still equal to its own is not sent, so text the
    /// editor shows differently (a tab, a line break in the name) is not
    /// rewritten when only another field was changed.
    pub loaded: [String; 2],
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
    /// What a new column can show, opened with `+` on the Timeline tab;
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
    /// The following timeline: what the Timeline tab shows without columns.
    pub timeline: List<Post>,
    /// The custom feeds the account pinned, which `+` offers as columns.
    pub feeds: Vec<crate::api::types::FeedInfo>,
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
    /// The account `B` asked to block, waiting for the `y` that confirms it.
    pub confirm_block: Option<String>,
    /// When the question waiting for its y was asked: it lasts as long as
    /// its prompt is on screen.
    question_at: Option<Instant>,
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
    /// The columns of the Timeline tab, of the account in use.
    pub columns: Columns,
    /// The Chat tab of the account in use.
    pub chat: ChatPane,
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
    /// Jobs sent before this number were sent with tokens a later login
    /// replaced: their finding that the session expired is old news.
    session_since: u64,
    /// The number of the job whose answer is being taken, if it has one.
    answering: Option<u64>,
    /// The number of the last profile asked for: an answer to an earlier
    /// one is of a profile left since.
    profile_asked: u64,
}

/// A write the server confirmed, as it shows on a post or an account.
#[derive(Debug, Clone)]
enum Written {
    Like { post: String, uri: Option<String> },
    Repost { post: String, uri: Option<String> },
    Follow { did: String, uri: Option<String> },
    Mute { did: String, on: bool },
    Block { did: String, uri: Option<String> },
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
            Event::Muted {
                did,
                on,
                result: Ok(()),
            } => Written::Mute {
                did: did.clone(),
                on: *on,
            },
            Event::Blocked {
                did,
                result: Ok(uri),
            } => Written::Block {
                did: did.clone(),
                uri: Some(uri.clone()),
            },
            Event::Unblocked {
                did,
                result: Ok(()),
            } => Written::Block {
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
            confirm_block: None,
            question_at: None,
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
            chat: ChatPane::default(),
            confirm_column_remove: None,
            pictures_change: None,
            settings_return: None,
            save_note: None,
            sent: 0,
            reads_out: BTreeSet::new(),
            written: Vec::new(),
            first_pages: HashMap::new(),
            account_since: 0,
            session_since: 0,
            answering: None,
            profile_asked: 0,
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
        let changed = self.expire_message(now);
        // A question lasts while its prompt is on screen: gone (its time up,
        // or another message in its place), a y pressed later answers
        // nothing.
        let asking = self.confirm_delete.is_some()
            || self.confirm_block.is_some()
            || self.confirm_column_remove.is_some()
            || self.confirm_logout.is_some();
        if asking && self.status.as_ref().map(|s| s.at) != self.question_at {
            self.confirm_delete = None;
            self.confirm_block = None;
            self.confirm_column_remove = None;
            self.confirm_logout = None;
            self.question_at = None;
        }
        changed
    }

    /// Note that the prompt just shown asks a question.
    pub(super) fn asked(&mut self) {
        self.question_at = self.status.as_ref().map(|s| s.at);
    }

    fn expire_message(&mut self, now: Instant) -> bool {
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
            // Esc that closes the box over the composer or the profile
            // editor only closes the box: there it would also throw the
            // draft away. Other keys go on, so typing goes on.
            if key.code == KeyCode::Esc
                && matches!(
                    self.overlay,
                    Some(Overlay::Compose(_) | Overlay::EditProfile(_))
                )
                && self.login.is_none()
            {
                return Vec::new();
            }
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
            // Not while the post is on its way or the profile loads or is
            // saved: the answer would drop what was pasted.
            Some(Overlay::Compose(c)) if c.browser.is_none() && !c.sending => {
                c.field().insert_str(text)
            }
            Some(Overlay::EditProfile(e)) if e.browser.is_none() && !e.loading && !e.saving => {
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
            None if self.tab == Tab::Chat && self.chat.open.as_ref().is_some_and(|o| o.typing) => {
                if let Some(o) = &mut self.chat.open {
                    o.input.insert_str(text);
                }
            }
            None => {}
        }
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
impl App {
    /// What still says it waits for the server: once every job has been
    /// answered, a flag left on is a spinner that never stops or a key that
    /// never works again.
    pub fn stuck(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.pending != 0 {
            out.push(format!("pending {}", self.pending));
        }
        if !self.in_flight.is_empty() {
            out.push(format!("in flight {:?}", self.in_flight));
        }
        let mut list = |name: &str, loading: bool, more: bool| {
            if loading {
                out.push(format!("{name} loading"));
            }
            if more {
                out.push(format!("{name} waiting for a next page"));
            }
        };
        list(
            "timeline",
            self.timeline.loading,
            self.timeline.more_pending,
        );
        list(
            "search posts",
            self.search.posts.loading,
            self.search.posts.more_pending,
        );
        list(
            "search accounts",
            self.search.actors.loading,
            self.search.actors.more_pending,
        );
        list(
            "profile posts",
            self.profile.posts.loading,
            self.profile.posts.more_pending,
        );
        list(
            "notifications",
            self.notifications.loading,
            self.notifications.more_pending,
        );
        list(
            "conversations",
            self.chat.convos.loading,
            self.chat.convos.more_pending,
        );
        for c in &self.columns.items {
            match &c.rows {
                Rows::Posts(l) => list("column", l.loading, l.more_pending),
                Rows::Notifications(l) => list("column", l.loading, l.more_pending),
            }
        }
        if self.profile.loading {
            out.push("profile loading".into());
        }
        for th in &self.threads {
            if !th.list.loaded && th.error.is_none() {
                out.push(format!("thread {} loading", th.uri));
            }
        }
        if let Some(o) = &self.chat.open {
            if o.loading_older {
                out.push("conversation loading older messages".into());
            }
            if o.sending {
                out.push("message sending".into());
            }
        }
        match &self.overlay {
            Some(Overlay::Compose(c)) if c.sending => out.push("post sending".into()),
            Some(Overlay::EditProfile(e)) if e.loading || e.saving => {
                out.push("profile editor waiting".into())
            }
            _ => {}
        }
        if self.login.as_ref().is_some_and(|f| f.pending) {
            out.push("login pending".into());
        }
        out
    }
}
