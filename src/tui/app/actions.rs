//! The keys of a post and an account: view, open, reply, like, repost, quote, copy, delete, follow, and the actions list.

use super::*;

impl App {
    /// Open the selected post's pictures (or video) full screen; a post
    /// with none but a link opens the link. A terminal that cannot show
    /// them opens the post on bsky.app, where they can be seen.
    pub(super) fn open_viewer(&mut self) -> Vec<Job> {
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
    pub(super) fn open_link(&mut self, or_post: bool) -> Vec<Job> {
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
                self.info(n!("this post has no pictures, video, or link"));
                Vec::new()
            }
        }
    }

    pub(super) fn open_thread(&mut self) -> Vec<Job> {
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

    pub(super) fn refresh(&mut self) -> Vec<Job> {
        if let Some(th) = self.threads.last_mut() {
            th.list.loaded = false;
            th.error = None;
            return vec![Job::Thread(th.uri.clone())];
        }
        match self.tab {
            Tab::Timeline => {
                self.info(n!("refreshing…"));
                vec![Job::Timeline]
            }
            Tab::Search => self.run_search(),
            // Read again in place, as the other lists are: the profile and
            // its posts stay while it loads, and the answer keeps the
            // selection on the post it was on.
            Tab::Profile => match &self.profile.profile {
                Some(p) => {
                    let target = p.did.clone();
                    self.profile.loading = true;
                    self.profile.error = None;
                    vec![Job::OpenProfile(target)]
                }
                None => self.open_profile(self.profile.actor.clone()),
            },
            Tab::Notifications => self.load_notifications(),
            Tab::Columns => {
                let id = self.columns.focused().map(|c| c.id);
                id.map(|id| self.load_column(id))
                    .into_iter()
                    .flatten()
                    .collect()
            }
            Tab::Chat => self.read_chat(),
        }
    }

    pub(super) fn reply(&mut self) -> Vec<Job> {
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
    /// A write key of the account in use (`like:<uri>`...), as `in_flight`
    /// keeps it: per account.
    pub(super) fn flight(&self, key: &str) -> String {
        let me = self.session.as_ref().map_or("", |s| s.did.as_str());
        format!("{me} {key}")
    }

    pub(super) fn claim(&mut self, key: String) -> bool {
        if self.in_flight.insert(self.flight(&key)) {
            true
        } else {
            self.info(n!("still waiting for the server…"));
            false
        }
    }

    pub(super) fn toggle_like(&mut self) -> Vec<Job> {
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
    pub(super) fn actions_key(
        &mut self,
        key: KeyEvent,
        selected: usize,
        about: Option<String>,
    ) -> Vec<Job> {
        // The selection moved off the post the list was opened on (a reload
        // without it, say): what it offers is not about that post any more.
        if self.actions_subject() != about {
            self.overlay = None;
            self.info(n!(
                "the post the list was about is no longer selected; press . again"
            ));
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
    pub(super) fn open_actions(&mut self) {
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
    pub(super) fn actions_subject(&self) -> Option<String> {
        self.shown_post()
            .map(|p| p.uri.clone())
            .or_else(|| self.shown_account().map(|a| a.did.clone()))
    }

    /// `Q`: a new post that quotes the selected one.
    pub(super) fn quote(&mut self) {
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
    pub(super) fn copy_link(&mut self) {
        let Some(post) = self.post_to_view() else {
            return;
        };
        match post.web_url() {
            Some(url) => {
                self.info(tf("copied {}", &[&url]));
                self.to_copy = Some(url);
            }
            None => self.info(n!("this post has no address to copy")),
        }
    }

    /// Text waiting to go to the terminal's clipboard, taken by the event
    /// loop that owns the terminal.
    pub fn take_copy(&mut self) -> Option<String> {
        self.to_copy.take()
    }

    /// `D`: ask before deleting the selected post. Only your own, and only
    /// with a second key, since a deleted post cannot be brought back.
    pub(super) fn ask_delete(&mut self) {
        let Some(post) = self.selected_post() else {
            return;
        };
        let mine = self
            .session
            .as_ref()
            .is_some_and(|s| s.did == post.author.did);
        if !mine {
            self.info(n!("you can only delete your own posts"));
            return;
        }
        if self
            .in_flight
            .contains(&self.flight(&format!("delete:{}", post.uri)))
        {
            self.info(n!("still waiting for the server…"));
            return;
        }
        self.info(n!("press y to delete this post, any other key to keep it"));
        self.confirm = Some(Confirm::Delete(post.uri));
        self.asked();
    }

    pub(super) fn toggle_repost(&mut self) -> Vec<Job> {
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

    pub(super) fn toggle_follow(&mut self) -> Vec<Job> {
        let Some(account) = self.selected_account() else {
            return Vec::new();
        };
        if self.session.as_ref().is_some_and(|s| s.did == account.did) {
            self.error(n!("you cannot follow yourself"));
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

    /// `M`: mute the selected account, or unmute it. Muting is private and
    /// undone as easily, so it asks nothing.
    pub(super) fn toggle_mute(&mut self) -> Vec<Job> {
        let Some(account) = self.selected_account() else {
            return Vec::new();
        };
        if self.session.as_ref().is_some_and(|s| s.did == account.did) {
            self.error(n!("you cannot mute yourself"));
            return Vec::new();
        }
        if !self.claim(format!("mute:{}", account.did)) {
            return Vec::new();
        }
        vec![Job::Mute {
            on: !account.muted(),
            did: account.did,
        }]
    }

    /// `B`: block the selected account after a `y`, since a block is public
    /// and cuts both ways; unblocking asks nothing.
    pub(super) fn toggle_block(&mut self) -> Vec<Job> {
        let Some(account) = self.selected_account() else {
            return Vec::new();
        };
        if self.session.as_ref().is_some_and(|s| s.did == account.did) {
            self.error(n!("you cannot block yourself"));
            return Vec::new();
        }
        if let Some(uri) = account.blocking_uri() {
            if !self.claim(format!("block:{}", account.did)) {
                return Vec::new();
            }
            return vec![Job::Unblock {
                block_uri: uri.to_string(),
                did: account.did,
            }];
        }
        if self
            .in_flight
            .contains(&self.flight(&format!("block:{}", account.did)))
        {
            self.info(n!("still waiting for the server…"));
            return Vec::new();
        }
        self.info(tf(
            "press y to block @{}, any other key to leave them be",
            &[&account.handle],
        ));
        self.confirm = Some(Confirm::Block(account.did));
        self.asked();
        Vec::new()
    }

    /// `e`: the profile editor, which waits for the record it starts from.
    pub(super) fn edit_profile(&mut self) -> Vec<Job> {
        if self.profile.actor.is_some() {
            self.error(n!("you can only edit your own profile (Esc returns to it)"));
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
            loaded: Default::default(),
        }));
        vec![Job::LoadProfileEditor]
    }

    /// Take the pinned feeds, which `+` offers as columns.
    pub(super) fn set_pinned_feeds(&mut self, infos: Vec<crate::api::types::FeedInfo>) {
        // The + list offers the feeds: its selection stays on its choice.
        let choosing = match &self.overlay {
            Some(Overlay::AddColumn { selected, .. }) => {
                self.column_choices().get(*selected).cloned()
            }
            _ => None,
        };
        self.feeds = infos;
        if let Some(chosen) = choosing {
            let at = self
                .column_choices()
                .iter()
                .position(|c| *c == chosen)
                .unwrap_or(0);
            if let Some(Overlay::AddColumn { selected, .. }) = &mut self.overlay {
                *selected = at;
            }
        }
    }
}
