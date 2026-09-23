//! What each key does: the login form, the overlays, the search box, the tabs and their lists.

use super::*;

impl App {
    pub(super) fn key(&mut self, key: KeyEvent) -> Vec<Job> {
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
        if self.tab == Tab::Chat
            && self.threads.is_empty()
            && let Some(jobs) = self.chat_key(key)
        {
            return jobs;
        }
        self.main_key(key)
    }

    pub(super) fn login_key(&mut self, key: KeyEvent) -> Vec<Job> {
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

    pub(super) fn overlay_key(&mut self, key: KeyEvent) -> Vec<Job> {
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
                        let changed =
                            |text: String, loaded: &String| (text != *loaded).then_some(text);
                        let display_name = changed(display_name, &e.loaded[0]);
                        let description = changed(description, &e.loaded[1]);
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

    pub(super) fn search_key(&mut self, key: KeyEvent) -> Vec<Job> {
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

    pub(super) fn toggle_search_mode(&mut self) {
        self.search.mode = match self.search.mode {
            SearchMode::Posts => SearchMode::Accounts,
            SearchMode::Accounts => SearchMode::Posts,
        };
    }

    pub(super) fn run_search(&mut self) -> Vec<Job> {
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

    pub(super) fn switch_tab(&mut self, tab: Tab) -> Vec<Job> {
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
        if tab == Tab::Chat && !self.chat.convos.loaded && !self.chat.convos.loading {
            self.chat.convos.begin();
            self.chat.polled = Some(Instant::now());
            return vec![Job::Convos { cursor: None }];
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

    pub(super) fn load_notifications(&mut self) -> Vec<Job> {
        self.notifications.begin();
        vec![Job::Notifications]
    }

    pub(super) fn open_profile(&mut self, actor: Option<String>) -> Vec<Job> {
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

    pub(super) fn main_key(&mut self, key: KeyEvent) -> Vec<Job> {
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
        // As D's: the next key answers B's question, whatever it is.
        if let Some(did) = self.confirm_block.take() {
            if key.code != KeyCode::Char('y') {
                self.info("not blocked");
                return Vec::new();
            }
            if !self.claim(format!("block:{did}")) {
                return Vec::new();
            }
            self.info("blocking…");
            return vec![Job::Block { did }];
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
            KeyCode::Char('6') => return self.switch_tab(Tab::Chat),
            KeyCode::Char('m') if self.tab == Tab::Profile && self.threads.is_empty() => {
                return self.message_profile();
            }
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
            KeyCode::Char('M') => return self.toggle_mute(),
            KeyCode::Char('B') => return self.toggle_block(),
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
    pub(super) fn go_back(&mut self) -> Vec<Job> {
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

    pub(super) fn step(&mut self, delta: isize) -> Vec<Job> {
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
            Tab::Chat => {
                self.chat.convos.step(delta);
                return self
                    .chat
                    .convos
                    .want_more()
                    .map(|c| Job::Convos { cursor: Some(c) })
                    .into_iter()
                    .collect();
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
}
