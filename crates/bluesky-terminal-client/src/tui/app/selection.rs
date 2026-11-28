//! What the keys act on: the list, post and account shown on the current tab or thread.

use super::*;

impl App {
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
            Tab::Timeline => Some(&mut self.timeline),
            Tab::Search if self.search.mode == SearchMode::Posts => Some(&mut self.search.posts),
            Tab::Search => None,
            Tab::Profile => Some(&mut self.profile.posts),
            Tab::Notifications => None,
            Tab::Columns => match self.columns.focused_mut().map(|c| &mut c.rows) {
                Some(Rows::Posts(l)) => Some(l),
                _ => None,
            },
            Tab::Chat => None,
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
    pub(super) fn selected_account(&mut self) -> Option<Profile> {
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

    /// The post Space and `o` act on: the selected one, or on the
    /// Notifications tab the post a notification is about.
    pub(super) fn post_to_view(&mut self) -> Option<Post> {
        match self.shown_notifications() {
            Some(l) => l
                .current()
                .and_then(|i| i.post.clone().or_else(|| i.subject.clone())),
            None => self.selected_post(),
        }
    }

    /// The post a notification of a like or a repost is about, when the
    /// selected notification carries no post of its own: what `v`, Space, `o`
    /// and `c` act on there.
    pub fn subject_shown(&self) -> Option<&Post> {
        let item = self.shown_notifications()?.current()?;
        item.post
            .is_none()
            .then_some(item.subject.as_ref())
            .flatten()
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
            Tab::Timeline => Some(&self.timeline),
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
}
