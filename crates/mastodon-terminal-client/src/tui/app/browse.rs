//! Search, profile, media, accounts and settings transitions.
use super::*;
use crate::i18n;
use crate::tui::worker::{ProfilePage, SearchPage};

pub(crate) struct Search {
    pub input: TextInput,
    pub editing: bool,
    pub accounts: bool,
    pub result_accounts: bool,
    pub back: Option<Tab>,
    pub tab: Tab,
    pending: Option<(u64, String, bool)>,
    previous_profile: Option<Profile>,
    pub query: String,
    pub posts: List<Status>,
    pub users: List<Account>,
}
impl Default for Search {
    fn default() -> Self {
        Self {
            input: TextInput::single(""),
            editing: false,
            accounts: false,
            result_accounts: false,
            back: None,
            tab: Tab::Home,
            pending: None,
            previous_profile: None,
            query: String::new(),
            posts: List::default(),
            users: List::default(),
        }
    }
}
#[derive(Default)]
pub(crate) struct Profile {
    pub account: Option<Account>,
    pub id: Option<String>,
    pub posts: List<Status>,
    pub back: Option<Tab>,
}

impl App {
    pub(crate) fn search_results(&self) -> bool {
        self.search.back.is_some() && self.tab == self.search.tab
    }

    pub(super) fn leave_search(&mut self) {
        if let Some(tab) = self.search.back.take() {
            self.tab = tab;
        }
        if let Some(profile) = self.search.previous_profile.take() {
            self.profile = profile;
        }
        self.search.pending = None;
        self.search.posts.waiting = None;
        self.search.users.waiting = None;
    }

    pub(super) fn run_search(&mut self, more: bool) -> Jobs {
        let query = if more {
            self.search.query.clone()
        } else {
            self.search.input.text().trim().to_string()
        };
        if query.is_empty() {
            return Vec::new();
        }
        let accounts = if more {
            self.search.result_accounts
        } else {
            self.search.accounts
        };
        if self.session.is_none() && (accounts || crate::cli::hashtag(&query).is_none()) {
            self.say(t(
                "log in to search accounts or posts; visitors can search #hashtags",
            ));
            return Vec::new();
        }
        let next = if more {
            if accounts {
                self.search.users.next.clone()
            } else {
                self.search.posts.next.clone()
            }
        } else {
            None
        };
        let job = self.job(Job::Search {
            query: query.clone(),
            accounts,
            next,
        });
        self.search.pending = Some((job.0, query, accounts));
        self.search.posts.waiting = Some(job.0);
        self.search.users.waiting = Some(job.0);
        vec![job]
    }

    pub(super) fn search_key(&mut self, key: KeyEvent) -> Jobs {
        match key.code {
            KeyCode::Esc => self.search.editing = false,
            KeyCode::Enter if !self.search.input.text().trim().is_empty() => {
                self.search.editing = false;
                return self.run_search(false);
            }
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.search.accounts = !self.search.accounts
            }
            _ => {
                self.search.input.handle_key(key);
            }
        }
        Vec::new()
    }

    pub(super) fn search_answer(&mut self, seq: u64, first: bool, result: Result<SearchPage>) {
        if self.search.pending.as_ref().map(|p| p.0) != Some(seq) {
            return;
        }
        let Some((_, query, accounts)) = self.search.pending.take() else {
            return;
        };
        match result {
            Ok(p) => {
                if first {
                    if self.search.back.is_none() {
                        self.search.back = Some(self.tab);
                        if self.tab == Tab::Profile {
                            self.search.previous_profile = Some(std::mem::take(&mut self.profile));
                        }
                        self.search.tab = if self.tab.kind().is_some() {
                            self.tab
                        } else {
                            Tab::Home
                        };
                    }
                    self.tab = self.search.tab;
                    self.threads.clear();
                    self.search.posts = List::default();
                    self.search.users = List::default();
                    self.search.posts.waiting = Some(seq);
                    self.search.users.waiting = Some(seq);
                    self.search.query = query;
                    self.search.result_accounts = accounts;
                }
                self.search
                    .posts
                    .take(seq, first, Ok((p.posts, p.next.clone())));
                self.search.users.take(seq, first, Ok((p.accounts, p.next)));
            }
            Err(e) => {
                self.search.posts.waiting = None;
                self.search.users.waiting = None;
                self.say(e.message());
            }
        }
    }

    pub(super) fn open_profile(&mut self) -> Jobs {
        let account =
            if self.search_results() && self.threads.is_empty() && self.search.result_accounts {
                self.search.users.current().cloned()
            } else {
                self.selected().map(|s| s.account.clone())
            };
        let Some(account) = account else {
            return Vec::new();
        };
        let back = if self.tab == Tab::Profile {
            self.profile.back
        } else {
            Some(self.tab)
        };
        self.profile = Profile {
            id: Some(account.id.clone()),
            account: Some(account),
            back,
            ..Default::default()
        };
        self.tab = Tab::Profile;
        self.threads.clear();
        self.load_profile(false)
    }

    pub(super) fn load_profile(&mut self, more: bool) -> Jobs {
        if self.profile.id.is_none() && self.session.is_none() {
            return Vec::new();
        }
        let job = self.job(Job::Profile {
            id: self.profile.id.clone(),
            max_id: if more {
                self.profile.posts.next.clone()
            } else {
                None
            },
        });
        self.profile.posts.waiting = Some(job.0);
        vec![job]
    }
    pub(super) fn profile_answer(&mut self, seq: u64, first: bool, result: Result<ProfilePage>) {
        if self.profile.posts.waiting != Some(seq) {
            return;
        }
        let page = result.map(|p| {
            self.profile.id = Some(p.account.id.clone());
            self.profile.account = Some(p.account);
            let next = next_of(&p.posts, |s: &Status| &s.id);
            (p.posts.items, next)
        });
        self.profile.posts.take(seq, first, page);
    }
    pub(super) fn browse_step(&mut self, by: isize) -> Jobs {
        let wants = match self.tab {
            Tab::Profile => {
                self.profile.posts.step(by);
                self.profile.posts.wants_more()
            }
            _ if self.search.result_accounts => {
                self.search.users.step(by);
                self.search.users.wants_more()
            }
            _ => {
                self.search.posts.step(by);
                self.search.posts.wants_more()
            }
        };
        if wants {
            if self.search_results() {
                return self.run_search(true);
            }
            self.load_tab(self.tab, true)
        } else {
            Vec::new()
        }
    }

    pub(super) fn open_viewer(&mut self) {
        let Some(s) = self.selected() else {
            return;
        };
        if !self.text_shown(s) {
            self.say(t("press w to open the content warning first"));
            return;
        }
        let files: Vec<_> = s
            .media_attachments
            .iter()
            .filter(|m| matches!(m.kind.as_str(), "image" | "video" | "gifv"))
            .cloned()
            .collect();
        if !files.is_empty() {
            self.overlay = Some(Overlay::Viewer {
                files,
                index: 0,
                generation: 0,
            });
        }
    }
    pub(super) fn download(&mut self) -> Jobs {
        let Some(s) = self.selected() else {
            return Vec::new();
        };
        if s.media_attachments.is_empty() {
            return Vec::new();
        }
        let job = Job::Download {
            id: s.id.clone(),
            files: s.media_attachments.clone(),
            directory: self.settings.as_ref().and_then(|s| s.download_dir.clone()),
        };
        self.say(t("saving files…"));
        vec![self.job(job)]
    }

    pub(crate) fn take_switch(&mut self) -> Option<Session> {
        self.switch_to.take()
    }
    pub(crate) fn switched(&mut self, session: Session) -> Jobs {
        self.origin = session.service.clone();
        self.session = Some(session);
        self.timelines = Default::default();
        self.restore_deck();
        self.notifications = List::default();
        self.read_ahead = Default::default();
        self.unread = 0;
        self.chat = Default::default();
        self.search = Search::default();
        self.profile = Profile::default();
        self.threads.clear();
        self.opened.clear();
        self.acting.clear();
        self.relationships.clear();
        self.relation_pending.clear();
        self.relation_before.clear();
        self.deleted.clear();
        self.deleting.clear();
        self.waiting.clear();
        self.overlay = None;
        self.message = None;
        self.copy = None;
        self.limits = Limits::default();
        self.tab = Tab::Home;
        let mut jobs = self.load_tab(self.tab, false);
        jobs.extend(self.load_deck());
        jobs.push(self.job(Job::Limits));
        if self.session.as_ref().is_some_and(|s| s.user_id.is_empty()) {
            jobs.push(self.job(Job::Identity));
        }
        jobs
    }

    pub(super) fn return_settings(&mut self) {
        if std::mem::take(&mut self.settings_return) {
            self.overlay = Some(Overlay::Settings { selected: 0 });
        }
    }
    pub(super) fn extra_overlay_key(&mut self, overlay: Overlay, key: KeyEvent) -> Jobs {
        match overlay {
            Overlay::Login {
                input,
                url,
                waiting,
            } => self.login_key(key, input, url, waiting),
            Overlay::Logout { host } => {
                if key.code == KeyCode::Char('y') {
                    self.logout_host = Some(host);
                }
            }
            Overlay::Viewer {
                files,
                mut index,
                mut generation,
            } => {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => return Vec::new(),
                    KeyCode::Right | KeyCode::Char('l' | 'j') => {
                        index = (index + 1).min(files.len().saturating_sub(1))
                    }
                    KeyCode::Left | KeyCode::Char('h' | 'k') => index = index.saturating_sub(1),
                    KeyCode::Char('r') => generation = generation.wrapping_add(1),
                    KeyCode::Char('d') => {
                        let jobs = self.download();
                        self.overlay = Some(Overlay::Viewer {
                            files,
                            index,
                            generation,
                        });
                        return jobs;
                    }
                    KeyCode::Char('o') => {
                        let jobs = self.list_key(key);
                        self.overlay = Some(Overlay::Viewer {
                            files,
                            index,
                            generation,
                        });
                        return jobs;
                    }
                    _ => {}
                }
                self.overlay = Some(Overlay::Viewer {
                    files,
                    index,
                    generation,
                });
            }
            Overlay::SettingText { row, input } => self.setting_text_key(key, row, input),
            Overlay::Settings { selected } => self.settings_key(key, selected),
            Overlay::Languages { mut selected } => {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        self.return_settings();
                        return Vec::new();
                    }
                    KeyCode::Char('j') | KeyCode::Down => selected = 1,
                    KeyCode::Char('k') | KeyCode::Up => selected = 0,
                    KeyCode::Enter => {
                        let lang = crate::i18n::ALL[selected];
                        i18n::set(lang);
                        if let Some(s) = &mut self.settings {
                            s.language = Some(lang.code().into());
                            self.settings_save = true;
                        } else {
                            self.say(t("settings cannot be saved this run"));
                        }
                        self.return_settings();
                        return Vec::new();
                    }
                    _ => {}
                }
                self.overlay = Some(Overlay::Languages { selected });
            }
            Overlay::Accounts { mut selected } => {
                match key.code {
                    KeyCode::Char('a') => {
                        self.overlay = Some(Overlay::Login {
                            input: TextInput::single(""),
                            url: None,
                            waiting: false,
                        });
                        return Vec::new();
                    }
                    KeyCode::Char('x') => {
                        if let Some(s) = self.accounts.get(selected) {
                            self.overlay = Some(Overlay::Logout {
                                host: s.host().to_string(),
                            });
                        }
                        return Vec::new();
                    }
                    KeyCode::Esc | KeyCode::Char('q' | 'A') => return Vec::new(),
                    KeyCode::Char('j') | KeyCode::Down => {
                        selected = (selected + 1).min(self.accounts.len().saturating_sub(1))
                    }
                    KeyCode::Char('k') | KeyCode::Up => selected = selected.saturating_sub(1),
                    KeyCode::Enter => {
                        self.switch_to = self.accounts.get(selected).cloned();
                        return Vec::new();
                    }
                    _ => {}
                }
                self.overlay = Some(Overlay::Accounts { selected });
            }
            _ => unreachable!("the other overlays have their own handlers"),
        }
        Vec::new()
    }
}
