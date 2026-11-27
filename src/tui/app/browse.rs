//! The Search tab (3) and the Profile tab (5), as bsky's: notes or
//! accounts that match a query, `/` or `i` to type it and `t` to switch
//! what is searched; an account's header and its notes, `f` to follow or
//! unfollow it, enter on a note's author or an account to open its
//! profile, and esc back to where it was opened from.

use super::*;
use crate::api::types::User;
use crate::tui::input::TextInput;
use crate::tui::worker::Who;

impl Keyed for User {
    fn key(&self) -> &str {
        &self.id
    }
}

/// What the Search tab looks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Notes,
    Accounts,
}

/// The Search tab.
#[derive(Debug, Clone)]
pub struct Search {
    pub input: TextInput,
    /// Keys go to the box.
    pub editing: bool,
    pub mode: SearchMode,
    pub notes: List<Note>,
    pub users: List<User>,
    /// The query each list is for: the box may have changed since.
    pub notes_query: String,
    pub users_query: String,
}

impl Default for Search {
    fn default() -> Self {
        Self {
            input: TextInput::single(""),
            editing: false,
            mode: SearchMode::Notes,
            notes: List::default(),
            users: List::default(),
            notes_query: String::new(),
            users_query: String::new(),
        }
    }
}

/// The Profile tab.
#[derive(Debug, Clone, Default)]
pub struct Profile {
    /// Whose: the account in use, or another by ID.
    pub who: Option<Who>,
    pub user: Option<User>,
    pub notes: List<Note>,
    /// Why the account could not be read.
    pub error: Option<String>,
    pub loading: bool,
    /// The tab it was opened from, which esc goes back to.
    pub came_from: Option<Tab>,
    asked: u64,
}

impl Profile {
    /// Whether it shows the account in use.
    pub fn own(&self, me: Option<&str>) -> bool {
        matches!(self.who, Some(Who::Me))
            || self
                .user
                .as_ref()
                .is_some_and(|u| Some(u.id.as_str()) == me)
    }
}

/// The page of accounts after `items`, by offset, as `users/search` pages
/// them; none once a page brought nothing new.
fn users_cursor(list: &mut List<User>, before: usize) {
    list.cursor = (list.items.len() > before).then(|| list.items.len().to_string());
}

impl App {
    /// Open the profile of `who`, remembering the tab it is opened from.
    pub(super) fn open_profile(&mut self, who: Who) -> Jobs {
        if who == Who::Me && self.session.is_none() {
            let host = self.host().to_string();
            self.tab = Tab::Profile;
            self.threads.clear();
            self.profile = Profile {
                who: Some(Who::Me),
                error: Some(format!(
                    "no account is logged in on {host}: press A then a to log in (or run `msky login --host {host}`)"
                )),
                ..Profile::default()
            };
            return Vec::new();
        }
        let came_from = if self.tab == Tab::Profile {
            self.profile.came_from
        } else {
            Some(self.tab)
        };
        self.tab = Tab::Profile;
        self.threads.clear();
        let (seq, job) = self.stamp(Job::Profile { who: who.clone() });
        self.profile = Profile {
            who: Some(who),
            loading: true,
            came_from,
            asked: seq,
            ..Profile::default()
        };
        self.profile.notes.begin();
        vec![(seq, job)]
    }

    /// The account selected: the profile shown, an account found, a
    /// notification's sender, or a note's author.
    pub(super) fn selected_user_id(&self) -> Option<String> {
        if self.threads.is_empty() {
            match self.tab {
                Tab::Search if self.search.mode == SearchMode::Accounts => {
                    return self.search.users.current().map(|u| u.id.clone());
                }
                Tab::Notifications => {
                    if let Some(u) = self.notifications.current().and_then(|n| n.user.as_ref()) {
                        return Some(u.id.clone());
                    }
                }
                _ => {}
            }
        }
        self.shown_note().map(|n| n.author_id().to_string())
    }

    /// enter: the selected account's profile.
    pub(super) fn open_selected_profile(&mut self) -> Jobs {
        let Some(id) = self.selected_user_id().filter(|id| !id.is_empty()) else {
            return Vec::new();
        };
        if self.tab == Tab::Profile
            && self.threads.is_empty()
            && self.profile.user.as_ref().is_some_and(|u| u.id == id)
        {
            return Vec::new();
        }
        self.open_profile(Who::Id(id))
    }

    /// esc on the Profile tab: back to where it was opened from; from
    /// someone else's profile with nowhere to go back to, your own.
    pub(super) fn profile_back(&mut self) -> Jobs {
        if let Some(tab) = self.profile.came_from.take() {
            self.tab = tab;
            self.search.editing = false;
            self.profile = Profile::default();
            return Vec::new();
        }
        if !self.profile.own(self.my_user_id()) {
            return self.open_profile(Who::Me);
        }
        Vec::new()
    }

    pub(super) fn my_user_id(&self) -> Option<&str> {
        self.session
            .as_ref()
            .map(|s| s.user_id.as_str())
            .filter(|id| !id.is_empty())
    }

    /// The account `f` acts on: the profile shown, or an account found.
    fn follow_target(&self) -> Option<&User> {
        if !self.threads.is_empty() {
            return None;
        }
        match self.tab {
            Tab::Profile => self.profile.user.as_ref(),
            Tab::Search if self.search.mode == SearchMode::Accounts => self.search.users.current(),
            _ => None,
        }
    }

    /// f: follow the account, or unfollow it, or take back the request.
    pub(super) fn toggle_follow(&mut self) -> Jobs {
        let Some(u) = self.follow_target() else {
            if self.shown_note().is_some() {
                self.info("enter opens the author's profile, where f follows");
            }
            return Vec::new();
        };
        if Some(u.id.as_str()) == self.my_user_id() {
            self.info("this is your own account");
            return Vec::new();
        }
        let (id, on, requested) = (
            u.id.clone(),
            !(u.is_following || u.has_pending_follow_request_from_you),
            u.has_pending_follow_request_from_you,
        );
        if !self.may("write:following", "follow accounts") || !self.claim(format!("follow:{id}")) {
            return Vec::new();
        }
        self.info(if on { "following…" } else { "unfollowing…" });
        vec![self.stamp(Job::Follow {
            user_id: id,
            on,
            requested,
        })]
    }

    /// The account as the server has it after a follow.
    pub(super) fn followed(&mut self, id: &str, result: crate::error::Result<Box<User>>) {
        self.in_flight.remove(&format!("follow:{id}"));
        let u = match result {
            Ok(u) => *u,
            Err(e) => {
                self.error(said(&e));
                return;
            }
        };
        let name = match u.host.as_deref().filter(|h| !h.is_empty()) {
            Some(h) => format!("@{}@{h}", u.username),
            None => format!("@{}", u.username),
        };
        self.info(if u.is_following {
            format!("following {name}")
        } else if u.has_pending_follow_request_from_you {
            format!("asked to follow {name}; pending until it is accepted")
        } else {
            format!("unfollowed {name}")
        });
        if let Some(p) = self.profile.user.as_mut().filter(|p| p.id == u.id) {
            *p = u.clone();
        }
        for s in self.search.users.items.iter_mut().filter(|s| s.id == u.id) {
            *s = u.clone();
        }
    }

    /// enter in the search box: search for what it holds.
    pub(super) fn run_search(&mut self) -> Jobs {
        let q = self.search.input.text().trim().to_string();
        if q.is_empty() {
            return Vec::new();
        }
        match self.search.mode {
            SearchMode::Notes => {
                if self.search.notes_query != q {
                    self.search.notes = List::default();
                }
                self.search.notes_query = q.clone();
                let (seq, job) = self.stamp(Job::SearchNotes {
                    query: q,
                    until: None,
                });
                self.search.notes.begin();
                self.search.notes.asked = seq;
                vec![(seq, job)]
            }
            SearchMode::Accounts => {
                if self.search.users_query != q {
                    self.search.users = List::default();
                }
                self.search.users_query = q.clone();
                let (seq, job) = self.stamp(Job::SearchUsers {
                    query: q,
                    offset: 0,
                });
                self.search.users.begin();
                self.search.users.asked = seq;
                vec![(seq, job)]
            }
        }
    }

    pub(super) fn toggle_search_mode(&mut self) {
        self.search.mode = match self.search.mode {
            SearchMode::Notes => SearchMode::Accounts,
            SearchMode::Accounts => SearchMode::Notes,
        };
    }

    /// A key while the search box has focus.
    pub(super) fn search_key(&mut self, key: KeyEvent) -> Jobs {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => self.search.editing = false,
            // Tabs switch even while typing, so the box never traps you.
            KeyCode::Tab => {
                self.search.editing = false;
                return self.switch_tab(self.next_tab(1));
            }
            KeyCode::BackTab => {
                self.search.editing = false;
                return self.switch_tab(self.next_tab(-1));
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

    /// The next page of the Search or Profile tab's list, near its end.
    pub(super) fn step_browse(&mut self, delta: isize) -> Jobs {
        match self.tab {
            Tab::Search if self.search.mode == SearchMode::Accounts => {
                self.search.users.step(delta);
                match self.search.users.want_more() {
                    Some(offset) => {
                        let offset = offset.parse().unwrap_or(0);
                        let query = self.search.users_query.clone();
                        vec![self.stamp(Job::SearchUsers { query, offset })]
                    }
                    None => Vec::new(),
                }
            }
            Tab::Search => {
                self.search.notes.step(delta);
                match self.search.notes.want_more() {
                    Some(until) => {
                        let query = self.search.notes_query.clone();
                        vec![self.stamp(Job::SearchNotes {
                            query,
                            until: Some(until),
                        })]
                    }
                    None => Vec::new(),
                }
            }
            Tab::Profile => {
                self.profile.notes.step(delta);
                let Some(id) = self.profile.user.as_ref().map(|u| u.id.clone()) else {
                    return Vec::new();
                };
                match self.profile.notes.want_more() {
                    Some(until) => vec![self.stamp(Job::UserNotes { user_id: id, until })],
                    None => Vec::new(),
                }
            }
            _ => Vec::new(),
        }
    }

    /// R on the Search or Profile tab.
    pub(super) fn refresh_browse(&mut self) -> Jobs {
        match self.tab {
            Tab::Search => {
                let q = match self.search.mode {
                    SearchMode::Notes => self.search.notes_query.clone(),
                    SearchMode::Accounts => self.search.users_query.clone(),
                };
                if q.is_empty() {
                    return Vec::new();
                }
                self.search.input = TextInput::single(&q);
                self.run_search()
            }
            Tab::Profile => match self.profile.who.clone() {
                Some(who) => {
                    let came_from = self.profile.came_from;
                    let jobs = self.open_profile(who);
                    self.profile.came_from = came_from;
                    jobs
                }
                None => self.open_profile(Who::Me),
            },
            _ => Vec::new(),
        }
    }

    /// Answers for the two tabs.
    pub(super) fn browse_answer(&mut self, seq: u64, event: Event) {
        match event {
            Event::Profile(result) => {
                if self.profile.asked != seq {
                    return;
                }
                self.profile.loading = false;
                match result {
                    Ok(b) => {
                        let (user, notes) = *b;
                        self.profile.user = Some(user);
                        self.profile.notes.set(notes);
                    }
                    Err(e) => {
                        self.profile.error = Some(said(&e));
                        self.profile.notes.failed(said(&e));
                    }
                }
            }
            Event::UserNotes {
                user_id,
                until,
                result,
            } => {
                if self.profile.user.as_ref().map(|u| u.id.as_str()) != Some(user_id.as_str()) {
                    return;
                }
                match result {
                    Ok(notes) => self.profile.notes.append(&until, notes),
                    Err(e) => {
                        self.profile.notes.more_failed(&until);
                        self.error(said(&e));
                    }
                }
            }
            Event::SearchNotes {
                query,
                until,
                result,
            } => {
                if query != self.search.notes_query {
                    return;
                }
                let list = &mut self.search.notes;
                match (until, result) {
                    (None, Ok(notes)) if list.asked == seq => list.set(notes),
                    (None, Err(e)) if list.asked == seq => list.failed(said(&e)),
                    (Some(u), Ok(notes)) => list.append(&u, notes),
                    (Some(u), Err(e)) => {
                        list.more_failed(&u);
                        self.error(said(&e));
                    }
                    _ => {}
                }
            }
            Event::SearchUsers {
                query,
                offset,
                result,
            } => {
                if query != self.search.users_query {
                    return;
                }
                let list = &mut self.search.users;
                match result {
                    Ok(users) if offset == 0 && list.asked == seq => {
                        list.set(users);
                        users_cursor(list, 0);
                    }
                    Ok(users) if offset > 0 => {
                        let asked = offset.to_string();
                        let continues = list.more_pending && list.cursor.as_deref() == Some(&asked);
                        let before = list.items.len();
                        list.append(&asked, users);
                        if continues {
                            users_cursor(list, before);
                        }
                    }
                    Err(e) if offset == 0 && list.asked == seq => list.failed(said(&e)),
                    Err(e) => {
                        list.more_failed(&offset.to_string());
                        self.error(said(&e));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
