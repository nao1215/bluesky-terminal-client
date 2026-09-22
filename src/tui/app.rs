//! UI state and what every key does. Nothing here touches the terminal or
//! the network: keys and finished jobs go in, [`Job`]s come out.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::api::types::{Post, Profile, ReplyRef};
use crate::api::{MAX_POST_GRAPHEMES, grapheme_len};
use crate::config::{Session, Settings};
use crate::error::Error;
use crate::media::{self, MAX_POST_IMAGES};
use crate::tui::files::{Action, Browser};
use crate::tui::input::TextInput;
use crate::tui::theme::{self, ColorDepth, THEMES, Theme};
use crate::tui::thread::{self as thread_rows, ThreadRow};
use crate::tui::worker::{Attachment, Event, Feed, Job, MorePage, NotifItem, Page};

/// The three top-level views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Timeline,
    Search,
    Profile,
    Notifications,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Timeline, Tab::Search, Tab::Notifications, Tab::Profile];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Timeline => "Timeline",
            Tab::Search => "Search",
            Tab::Profile => "Profile",
            Tab::Notifications => "Notifications",
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
    /// Replace the list with a first page.
    fn set(&mut self, page: Page<T>) {
        self.items = page.items;
        self.cursor = page.cursor;
        self.more_pending = false;
        self.selected = 0;
        self.offset = 0;
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
            sending: false,
            media: Vec::new(),
            focus: 0,
            browser: None,
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
    /// Service URL, identifier, app password.
    pub fields: [TextInput; 3],
    pub focus: usize,
    pub pending: bool,
    pub error: Option<String>,
}

impl LoginForm {
    pub const LABELS: [&'static str; 3] = ["Service", "Handle or email", "App password"];

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
    pub timeline: List<Post>,
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
    /// The colors everything is drawn with: the chosen theme, adapted to what
    /// the terminal can show.
    pub theme: Theme,
    /// Index into [`THEMES`] of the chosen theme.
    pub theme_index: usize,
    pub color_depth: ColorDepth,
    /// The settings as loaded, so saving keeps what bs did not change.
    pub settings: Settings,
    /// The folder the picture browser last showed, where it opens next.
    pub browse_from: Option<PathBuf>,
    /// Settings waiting to be written by the event loop.
    pub settings_to_save: Option<Settings>,
    /// Whether the settings file may be written: not when it was there but
    /// could not be read, since writing would lose what it holds.
    pub settings_writable: bool,
    pub quit: bool,
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
            theme: THEMES[0],
            theme_index: 0,
            color_depth: ColorDepth::TrueColor,
            settings: Settings::default(),
            browse_from: None,
            settings_to_save: None,
            settings_writable: true,
            quit: false,
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
        vec![Job::Timeline, Job::Notifications]
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
                warning = Some(format!("unknown theme {name:?}; using default"));
                0
            }),
            None => 0,
        };
        self.settings = settings;
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
        match result {
            Ok(()) => self.info(format!("theme: {}", THEMES[self.theme_index].name)),
            Err(e) => self.error(e.message().to_string()),
        }
    }

    fn open_theme_picker(&mut self) {
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

    /// Handle a key press.
    pub fn handle_key(&mut self, key: KeyEvent) -> Vec<Job> {
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
            Some(Overlay::Help { .. } | Overlay::Themes { .. }) => {}
            None if self.tab == Tab::Search && self.search.editing => {
                self.search.input.insert_str(text)
            }
            None => {}
        }
    }

    /// Handle a finished job.
    pub fn handle_event(&mut self, event: Event) -> Vec<Job> {
        self.pending = self.pending.saturating_sub(1);
        let jobs = self.event(event);
        self.pending += jobs.len();
        jobs
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
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match self.overlay.as_mut().unwrap() {
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
            Overlay::Themes { selected, previous } => {
                let (selected, previous) = (*selected, *previous);
                let n = THEMES.len();
                let pick = match key.code {
                    KeyCode::Char('j') | KeyCode::Down => Some((selected + 1) % n),
                    KeyCode::Char('k') | KeyCode::Up => Some((selected + n - 1) % n),
                    KeyCode::Enter => {
                        self.overlay = None;
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
                        self.overlay = None;
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
                        let len = grapheme_len(text.trim_end());
                        if text.trim().is_empty() && c.media.is_empty() {
                            self.error("the post is empty");
                        } else if len > MAX_POST_GRAPHEMES {
                            self.error(format!(
                                "the post is {len} characters; the limit is {MAX_POST_GRAPHEMES}"
                            ));
                        } else {
                            c.sending = true;
                            let reply = c.reply.as_ref().map(|(r, _, _)| r.clone());
                            let media = c
                                .media
                                .iter()
                                .map(|a| Attachment {
                                    path: a.path.clone(),
                                    alt: a.alt.text(),
                                })
                                .collect();
                            return vec![Job::Post { text, reply, media }];
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

    /// The post list of the current view, if it shows posts.
    pub fn current_posts(&mut self) -> Option<&mut List<Post>> {
        match self.tab {
            Tab::Timeline => Some(&mut self.timeline),
            Tab::Search if self.search.mode == SearchMode::Posts => Some(&mut self.search.posts),
            Tab::Search => None,
            Tab::Profile => Some(&mut self.profile.posts),
            Tab::Notifications => None,
        }
    }

    fn selected_post(&mut self) -> Option<Post> {
        if let Some(th) = self.threads.last() {
            return th.list.current().and_then(ThreadRow::post).cloned();
        }
        if self.tab == Tab::Notifications {
            // Only a reply, mention, or quote is a post to act on.
            return self.notifications.current()?.post.clone();
        }
        self.current_posts()?.current().cloned()
    }

    /// The account `f` and Enter act on in the current view.
    fn selected_account(&mut self) -> Option<Profile> {
        if !self.threads.is_empty() {
            return self.selected_post().map(|p| p.author);
        }
        match self.tab {
            Tab::Search if self.search.mode == SearchMode::Accounts => {
                self.search.actors.current().cloned()
            }
            Tab::Profile => self.profile.profile.clone(),
            Tab::Notifications => self.notifications.current().map(|i| i.n.author.clone()),
            _ => self.selected_post().map(|p| p.author),
        }
    }

    fn main_key(&mut self, key: KeyEvent) -> Vec<Job> {
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.overlay = Some(Overlay::Help { scroll: 0 }),
            KeyCode::Char('T') => self.open_theme_picker(),
            KeyCode::Char('1') => return self.switch_tab(Tab::Timeline),
            KeyCode::Char('2') => return self.switch_tab(Tab::Search),
            KeyCode::Char('3') => return self.switch_tab(Tab::Notifications),
            KeyCode::Char('4') => return self.switch_tab(Tab::Profile),
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
            KeyCode::Char('v') => return self.open_thread(),
            KeyCode::Esc if self.tab == Tab::Profile => return self.go_back(),
            KeyCode::Char('R') | KeyCode::F(5) => return self.refresh(),
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
                self.timeline.step(delta);
                self.timeline.want_more().map(|c| (Feed::Timeline, c))
            }
            Tab::Notifications => {
                self.notifications.step(delta);
                self.notifications
                    .want_more()
                    .map(|c| (Feed::Notifications, c))
            }
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

    fn open_thread(&mut self) -> Vec<Job> {
        let post = if self.tab == Tab::Notifications && self.threads.is_empty() {
            // A like or repost opens the thread of the post it is about.
            self.notifications
                .current()
                .and_then(|i| i.post.clone().or_else(|| i.subject.clone()))
        } else {
            self.selected_post()
        };
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
                vec![Job::Timeline]
            }
            Tab::Search => self.run_search(),
            Tab::Profile => self.open_profile(self.profile.actor.clone()),
            Tab::Notifications => self.load_notifications(),
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
    fn each_post(&mut self, uri: &str, mut f: impl FnMut(&mut Post)) {
        for list in [
            &mut self.timeline,
            &mut self.search.posts,
            &mut self.profile.posts,
        ] {
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
        for item in &mut self.notifications.items {
            item.post
                .iter_mut()
                .chain(item.subject.iter_mut())
                .filter(|p| p.uri == uri)
                .for_each(&mut f);
        }
    }

    /// Set the follow state of `did` everywhere it is shown.
    fn set_following(&mut self, did: &str, uri: Option<String>) {
        let apply = |p: &mut Profile| {
            if p.did == did {
                p.viewer.get_or_insert_with(Default::default).following = uri.clone();
            }
        };
        for list in [
            &mut self.timeline,
            &mut self.search.posts,
            &mut self.profile.posts,
        ] {
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
        for item in &mut self.notifications.items {
            apply(&mut item.n.author);
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
            self.overlay = None;
            return;
        }
        self.error(e.message().to_string());
    }

    /// Drop everything loaded for the account that was logged in, when
    /// another one logs in: its notifications, its profile, its threads.
    fn forget_account(&mut self) {
        self.timeline = List::default();
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
            _ => {}
        }
        match event {
            Event::LoggedIn(Ok(session)) => {
                self.info(format!("logged in as @{}", session.handle));
                if self.session.as_ref().is_some_and(|s| s.did != session.did) {
                    self.forget_account();
                }
                self.session = Some(session);
                self.login = None;
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
            Event::SearchPosts(Ok(posts)) => self.search.posts.set(posts),
            Event::SearchActors(Ok(actors)) => self.search.actors.set(actors),
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
                self.each_post(&post_uri, |p| {
                    p.viewer.get_or_insert_with(Default::default).like = Some(like.clone());
                    p.like_count += 1;
                });
                self.info("liked");
            }
            Event::Unliked {
                post_uri,
                result: Ok(()),
            } => {
                self.each_post(&post_uri, |p| {
                    if let Some(v) = &mut p.viewer {
                        v.like = None;
                    }
                    p.like_count = p.like_count.saturating_sub(1);
                });
                self.info("like removed");
            }
            Event::Reposted {
                post_uri,
                result: Ok(repost),
            } => {
                self.each_post(&post_uri, |p| {
                    p.viewer.get_or_insert_with(Default::default).repost = Some(repost.clone());
                    p.repost_count += 1;
                });
                self.info("reposted");
            }
            Event::Unreposted {
                post_uri,
                result: Ok(()),
            } => {
                self.each_post(&post_uri, |p| {
                    if let Some(v) = &mut p.viewer {
                        v.repost = None;
                    }
                    p.repost_count = p.repost_count.saturating_sub(1);
                });
                self.info("repost removed");
            }
            Event::More {
                feed,
                cursor,
                result,
            } => self.more(feed, &cursor, result),
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
                        let (rows, focus) = thread_rows::flatten(node);
                        th.list.items = rows;
                        th.list.selected = focus;
                        th.list.offset = 0;
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
            }
            Event::Posted { result: Err(e), .. } => {
                if let Some(Overlay::Compose(c)) = &mut self.overlay {
                    c.sending = false;
                }
                self.fail(&e);
            }
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
            Event::SearchPosts(Err(e)) => {
                self.search.posts.failed(&e);
                self.fail(&e);
            }
            Event::SearchActors(Err(e)) => {
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
        assert!(matches!(jobs[..], [Job::Timeline, Job::Notifications]));
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
        assert!(matches!(jobs[..], [Job::Timeline, Job::Notifications]));
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
        type_str(&mut app, &"あ".repeat(MAX_POST_GRAPHEMES + 1));
        assert!(app.handle_key(ctrl('s')).is_empty());
        assert!(app.status.as_ref().unwrap().text.contains("301"));
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
        app.handle_event(Event::SearchActors(Ok(vec![carol].into())));
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
        app.handle_event(Event::SearchActors(Ok(actors.into())));
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
        // The notifications loading in the background since the start.
        assert_eq!(app.pending, 1);
        app.handle_key(key('l'));
        assert_eq!(app.pending, 2);
        app.handle_event(Event::Liked {
            post_uri: "at://a/p/1".into(),
            result: Err(Error::api("x")),
        });
        assert_eq!(app.pending, 1);
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

    #[test]
    fn a_failed_search_settles_even_after_leaving_the_tab() {
        let mut app = logged_in();
        app.handle_key(key('/'));
        type_str(&mut app, "x");
        app.handle_key(code(KeyCode::Enter));
        assert!(!app.search.posts.loaded);
        app.handle_key(key('1'));
        app.handle_event(Event::SearchPosts(Err(Error::api("boom"))));
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
        app.handle_event(Event::SearchPosts(Ok(vec![
            post("at://s/1", "did:plc:x", false),
            post("at://s/2", "did:plc:y", false),
        ]
        .into())));
        app.handle_key(key('j'));
        assert_eq!(app.search.posts.selected, 1);
        assert_eq!(app.search.input.text(), "q l f");
    }

    #[test]
    fn backtab_arrives_at_search_focused_too() {
        let mut app = logged_in();
        app.handle_key(code(KeyCode::BackTab)); // Timeline -> Notifications
        app.handle_key(code(KeyCode::BackTab)); // Notifications -> Profile
        app.handle_key(code(KeyCode::BackTab)); // Profile -> Search
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
        assert_eq!(app.theme.name, "default");
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
        assert_eq!(app.pending, 1, "only the notifications, in the background");
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
        app.handle_event(Event::SearchPosts(Ok(page(
            vec![post("at://s/1", "did:plc:x", false)],
            Some("c1"),
        ))));
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
        app.handle_event(Event::SearchPosts(Ok(page(
            vec![post("at://s/1", "did:plc:x", false)],
            Some("pa"),
        ))));
        // Accounts for "b", then back to the posts for "a" without searching.
        app.handle_key(key('/'));
        app.handle_key(ctrl('t'));
        app.handle_key(ctrl('u'));
        type_str(&mut app, "b");
        app.handle_key(code(KeyCode::Enter));
        app.handle_event(Event::SearchActors(Ok(Vec::new().into())));
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
        assert!(matches!(&jobs[..], [Job::Timeline, Job::Notifications]));
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
        app.handle_event(Event::SearchPosts(Ok(page(
            vec![post("at://s/1", "did:plc:x", false)],
            None,
        ))));
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
}
