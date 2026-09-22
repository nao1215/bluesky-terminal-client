//! UI state and what every key does. Nothing here touches the terminal or
//! the network: keys and finished jobs go in, [`Job`]s come out.

use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::api::types::{Post, Profile, ReplyRef};
use crate::api::{MAX_POST_GRAPHEMES, grapheme_len};
use crate::config::Session;
use crate::error::Error;
use crate::tui::input::TextInput;
use crate::tui::worker::{Event, Job};

/// The three top-level views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Timeline,
    Search,
    Profile,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Timeline, Tab::Search, Tab::Profile];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Timeline => "Timeline",
            Tab::Search => "Search",
            Tab::Profile => "Profile",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap()
    }
}

/// What the search box looks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Posts,
    Accounts,
}

/// A scrollable list with a selection.
#[derive(Debug, Clone, Default)]
pub struct List<T> {
    pub items: Vec<T>,
    pub selected: usize,
    /// First item drawn; kept by the view so the selection stays visible.
    pub offset: usize,
    /// Whether a result (possibly empty) has arrived.
    pub loaded: bool,
}

impl<T> List<T> {
    fn set(&mut self, items: Vec<T>) {
        self.items = items;
        self.selected = 0;
        self.offset = 0;
        self.loaded = true;
    }

    pub fn current(&self) -> Option<&T> {
        self.items.get(self.selected)
    }

    fn step(&mut self, delta: isize) {
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
}

/// The post composer.
#[derive(Debug, Clone)]
pub struct Compose {
    pub input: TextInput,
    /// Reply target, the handle being answered, and an excerpt of the post.
    pub reply: Option<(ReplyRef, String, String)>,
    pub sending: bool,
}

/// The profile editor.
#[derive(Debug, Clone)]
pub struct EditProfile {
    /// Display name, description, avatar path.
    pub fields: [TextInput; 3],
    pub focus: usize,
    pub loading: bool,
    pub saving: bool,
}

impl EditProfile {
    pub const LABELS: [&'static str; 3] = [
        "Display name",
        "Description",
        "New avatar (PNG/JPEG path, optional)",
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
    Help,
}

/// A one-line message in the status bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub text: String,
    pub error: bool,
}

/// All UI state.
#[derive(Debug, Clone)]
pub struct App {
    pub session: Option<Session>,
    pub login: Option<LoginForm>,
    pub tab: Tab,
    pub timeline: List<Post>,
    pub search: Search,
    pub profile: ProfilePane,
    pub overlay: Option<Overlay>,
    pub status: Option<Status>,
    /// Jobs sent and not yet answered.
    pub pending: usize,
    /// Likes and follows waiting for the server, keyed `like:<post uri>` and
    /// `follow:<did>`. A second press on the same target is refused until the
    /// first is answered, or two presses would create two records.
    pub in_flight: HashSet<String>,
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
            },
            profile: ProfilePane::default(),
            overlay: None,
            status: None,
            pending: 0,
            in_flight: HashSet::new(),
            quit: false,
        };
        let jobs = if app.session.is_some() {
            vec![Job::Timeline]
        } else {
            Vec::new()
        };
        app.pending += jobs.len();
        (app, jobs)
    }

    fn info(&mut self, text: impl Into<String>) {
        self.status = Some(Status {
            text: text.into(),
            error: false,
        });
    }

    fn error(&mut self, text: impl Into<String>) {
        self.status = Some(Status {
            text: text.into(),
            error: true,
        });
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
            Some(Overlay::Compose(c)) => c.input.insert_str(text),
            Some(Overlay::EditProfile(e)) => e.fields[e.focus].insert_str(text),
            Some(Overlay::Help) => {}
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
            Overlay::Help => {
                self.overlay = None;
            }
            Overlay::Compose(c) => {
                if c.sending {
                    return Vec::new();
                }
                match key.code {
                    KeyCode::Esc => self.overlay = None,
                    KeyCode::Char('s') if ctrl => {
                        let text = c.input.text();
                        let len = grapheme_len(text.trim_end());
                        if text.trim().is_empty() {
                            self.error("the post is empty");
                        } else if len > MAX_POST_GRAPHEMES {
                            self.error(format!(
                                "the post is {len} characters; the limit is {MAX_POST_GRAPHEMES}"
                            ));
                        } else {
                            c.sending = true;
                            let reply = c.reply.as_ref().map(|(r, _, _)| r.clone());
                            return vec![Job::Post { text, reply }];
                        }
                    }
                    _ => {
                        c.input.handle_key(key);
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
                match key.code {
                    KeyCode::Esc => self.overlay = None,
                    KeyCode::Tab => e.focus = (e.focus + 1) % 3,
                    KeyCode::BackTab => e.focus = (e.focus + 2) % 3,
                    KeyCode::Char('s') if ctrl => {
                        e.saving = true;
                        let [display_name, description, avatar_path] =
                            e.fields.clone().map(|f| f.text());
                        return vec![Job::SaveProfile {
                            display_name,
                            description,
                            avatar_path,
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
                self.search.posts.loaded = false;
                vec![Job::SearchPosts(q)]
            }
            SearchMode::Accounts => {
                self.search.actors.loaded = false;
                vec![Job::SearchActors(q)]
            }
        }
    }

    fn switch_tab(&mut self, tab: Tab) -> Vec<Job> {
        self.tab = tab;
        if tab == Tab::Profile && self.profile.profile.is_none() && self.profile.actor.is_none() {
            return self.open_profile(None);
        }
        Vec::new()
    }

    fn open_profile(&mut self, actor: Option<String>) -> Vec<Job> {
        let Some(session) = &self.session else {
            return Vec::new();
        };
        let target = actor.clone().unwrap_or_else(|| session.did.clone());
        let own = target == session.did || target == session.handle;
        self.tab = Tab::Profile;
        self.profile = ProfilePane {
            actor: if own { None } else { actor },
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
        }
    }

    fn selected_post(&mut self) -> Option<Post> {
        self.current_posts()?.current().cloned()
    }

    /// The account `f` and Enter act on in the current view.
    fn selected_account(&mut self) -> Option<Profile> {
        match self.tab {
            Tab::Search if self.search.mode == SearchMode::Accounts => {
                self.search.actors.current().cloned()
            }
            Tab::Profile => self.profile.profile.clone(),
            _ => self.selected_post().map(|p| p.author),
        }
    }

    fn main_key(&mut self, key: KeyEvent) -> Vec<Job> {
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.overlay = Some(Overlay::Help),
            KeyCode::Char('1') => return self.switch_tab(Tab::Timeline),
            KeyCode::Char('2') => return self.switch_tab(Tab::Search),
            KeyCode::Char('3') => return self.switch_tab(Tab::Profile),
            KeyCode::Tab => return self.switch_tab(Tab::ALL[(self.tab.index() + 1) % 3]),
            KeyCode::BackTab => return self.switch_tab(Tab::ALL[(self.tab.index() + 2) % 3]),
            KeyCode::Char('j') | KeyCode::Down => self.step(1),
            KeyCode::Char('k') | KeyCode::Up => self.step(-1),
            KeyCode::PageDown => self.step(5),
            KeyCode::PageUp => self.step(-5),
            KeyCode::Char('g') | KeyCode::Home => self.step(isize::MIN / 2),
            KeyCode::Char('G') | KeyCode::End => self.step(isize::MAX / 2),
            KeyCode::Char('/') => {
                self.tab = Tab::Search;
                self.search.editing = true;
            }
            KeyCode::Char('t') if self.tab == Tab::Search => {
                self.toggle_search_mode();
                return self.run_search();
            }
            KeyCode::Char('n') => {
                self.overlay = Some(Overlay::Compose(Compose {
                    input: TextInput::multi(""),
                    reply: None,
                    sending: false,
                }));
            }
            KeyCode::Char('r') => return self.reply(),
            KeyCode::Char('l') => return self.toggle_like(),
            KeyCode::Char('f') => return self.toggle_follow(),
            KeyCode::Char('e') if self.tab == Tab::Profile => return self.edit_profile(),
            KeyCode::Enter => {
                if self.tab != Tab::Profile
                    && let Some(account) = self.selected_account()
                {
                    return self.open_profile(Some(account.did));
                }
            }
            KeyCode::Esc if self.tab == Tab::Profile && self.profile.actor.is_some() => {
                return self.open_profile(None);
            }
            KeyCode::Char('R') | KeyCode::F(5) => return self.refresh(),
            _ => {}
        }
        Vec::new()
    }

    fn step(&mut self, delta: isize) {
        if self.tab == Tab::Search && self.search.mode == SearchMode::Accounts {
            self.search.actors.step(delta);
        } else if let Some(list) = self.current_posts() {
            list.step(delta);
        }
    }

    fn refresh(&mut self) -> Vec<Job> {
        match self.tab {
            Tab::Timeline => {
                self.info("refreshing…");
                vec![Job::Timeline]
            }
            Tab::Search => self.run_search(),
            Tab::Profile => self.open_profile(self.profile.actor.clone()),
        }
    }

    fn reply(&mut self) -> Vec<Job> {
        if let Some(post) = self.selected_post() {
            let excerpt = post.record().text.lines().next().unwrap_or("").to_string();
            self.overlay = Some(Overlay::Compose(Compose {
                input: TextInput::multi(""),
                reply: Some((post.reply_ref(), post.author.handle.clone(), excerpt)),
                sending: false,
            }));
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

    /// Whether a loaded profile is the one the Profile tab is waiting for;
    /// an answer for a profile opened earlier is dropped.
    fn wanted_profile(&self, p: &Profile) -> bool {
        match (&self.profile.actor, &self.session) {
            (Some(actor), _) => *actor == p.did || *actor == p.handle,
            (None, Some(s)) => s.did == p.did,
            (None, None) => false,
        }
    }

    fn event(&mut self, event: Event) -> Vec<Job> {
        match &event {
            Event::Liked { post_uri, .. } | Event::Unliked { post_uri, .. } => {
                self.in_flight.remove(&format!("like:{post_uri}"));
            }
            Event::Followed { did, .. } | Event::Unfollowed { did, .. } => {
                self.in_flight.remove(&format!("follow:{did}"));
            }
            _ => {}
        }
        match event {
            Event::LoggedIn(Ok(session)) => {
                self.info(format!("logged in as @{}", session.handle));
                self.session = Some(session);
                self.login = None;
                return vec![Job::Timeline];
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
                self.timeline.loaded = true;
                self.fail(&e);
            }
            Event::SearchPosts(Err(e)) => {
                self.search.posts.loaded = true;
                self.fail(&e);
            }
            Event::SearchActors(Err(e)) => {
                self.search.actors.loaded = true;
                self.fail(&e);
            }
            Event::Profile(Err(e)) => {
                self.profile.error = Some(e.message().to_string());
                self.fail(&e);
            }
            Event::Liked { result: Err(e), .. }
            | Event::Unliked { result: Err(e), .. }
            | Event::Followed { result: Err(e), .. }
            | Event::Unfollowed { result: Err(e), .. } => self.fail(&e),
        }
        Vec::new()
    }
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
        assert!(matches!(jobs[..], [Job::Timeline]));
        app.handle_event(Event::Timeline(Ok(vec![
            post("at://a/p/1", "did:plc:alice", true),
            post("at://b/p/2", "did:plc:bob", true),
        ])));
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
        assert!(matches!(jobs[..], [Job::Timeline]));
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
                },
            ] => {
                assert_eq!(text, "hello");
                assert_eq!(r.parent.uri, "at://b/p/2");
                assert_eq!(r.root.uri, "at://b/p/2");
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
        app.handle_event(Event::SearchActors(Ok(vec![carol])));
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
        let jobs = app.handle_key(key('3'));
        assert!(matches!(&jobs[..], [Job::OpenProfile(a)] if a == "did:plc:me"));
        let me: Profile =
            serde_json::from_value(json!({"did": "did:plc:me", "handle": "me.test"})).unwrap();
        app.handle_event(Event::Profile(Ok((me, vec![]))));
        assert!(app.handle_key(key('f')).is_empty());
        assert!(app.status.as_ref().unwrap().error);
    }

    #[test]
    fn profile_editor_loads_then_saves_the_fields() {
        let mut app = logged_in();
        app.handle_key(key('3'));
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
                    avatar_path,
                },
            ] => {
                assert_eq!(display_name, "Me Myself");
                assert_eq!(description, "old bio\nline2");
                assert_eq!(avatar_path, "");
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
        let jobs = app.handle_key(code(KeyCode::Esc));
        assert!(matches!(&jobs[..], [Job::OpenProfile(a)] if a == "did:plc:me"));
        assert!(app.profile.actor.is_none());
    }

    #[test]
    fn pending_counts_jobs_in_flight() {
        let mut app = logged_in();
        assert_eq!(app.pending, 0);
        app.handle_key(key('l'));
        assert_eq!(app.pending, 1);
        app.handle_event(Event::Liked {
            post_uri: "at://a/p/1".into(),
            result: Err(Error::api("x")),
        });
        assert_eq!(app.pending, 0);
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
        app.handle_key(key('3'));
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
        app.handle_event(Event::Profile(Ok((bob, vec![]))));
        assert!(app.profile.profile.is_none());
        let alice: Profile =
            serde_json::from_value(json!({"did": "did:plc:alice", "handle": "alice.test"}))
                .unwrap();
        app.handle_event(Event::Profile(Ok((alice, vec![]))));
        assert_eq!(app.profile.profile.as_ref().unwrap().did, "did:plc:alice");
        assert!(app.profile.error.is_none());
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
}
