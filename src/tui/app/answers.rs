//! Answers from the worker: which are taken, what they change, and writes confirmed shown again on what a late read brought.

use super::*;

impl App {
    /// Number a job as it goes to the worker. The answer comes back with
    /// the same number, which tells what was sent after it.
    pub fn stamp(&mut self, job: &Job) -> u64 {
        self.sent += 1;
        if matches!(job, Job::OpenProfile(_)) {
            self.profile_asked = self.sent;
        }
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
    pub(super) fn answer(&mut self, seq: Option<u64>, event: Event) -> Vec<Job> {
        self.pending = self.pending.saturating_sub(1);
        let read = seq.filter(|s| self.reads_out.remove(s));
        // A download or a link opened belongs to no account: it is said
        // whoever is logged in when it is done.
        let anyone = matches!(
            event,
            Event::Downloaded(_) | Event::Opened { .. } | Event::LoggedIn(_)
        );
        if let Some(seq) = read
            && !anyone
            && self.superseded(seq, &event)
        {
            self.forget_writes();
            return Vec::new();
        }
        // A write the account before made (it is sent as that account) is
        // not the one in use's: a like shown on its lists would be undone
        // with a record it does not own.
        if let Some(seq) = seq
            && seq < self.account_since
            && !anyone
        {
            self.forget_writes();
            return Vec::new();
        }
        let written = Written::of(&event);
        self.answering = seq;
        let jobs = self.event(event);
        self.answering = None;
        self.timeline.drop_shown_above();
        for l in self.columns.post_lists() {
            l.drop_shown_above();
        }
        if let Some(w) = written
            && !self.reads_out.is_empty()
        {
            self.written.push((self.sent, w));
        }
        // A write is kept with the last job sent when its answer came: a
        // read sent up to then, that one included, may have been read
        // before the write was made.
        if let Some(seq) = read {
            let since: Vec<Written> = self
                .written
                .iter()
                .filter(|(at, _)| *at >= seq)
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
    pub(super) fn superseded(&mut self, seq: u64, event: &Event) -> bool {
        if seq < self.account_since {
            return true;
        }
        let list = match event {
            Event::Timeline(_) => "timeline".to_string(),
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
    pub(super) fn forget_writes(&mut self) {
        match self.reads_out.first().copied() {
            Some(oldest) => self.written.retain(|(at, _)| *at >= oldest),
            None => self.written.clear(),
        }
    }

    /// Show a confirmed write again on whatever a late read brought.
    pub(super) fn apply(&mut self, w: &Written) {
        match w {
            Written::Like { post, uri } => self.set_like(post, uri.clone()),
            Written::Repost { post, uri } => self.set_repost(post, uri.clone()),
            Written::Follow { did, uri } => {
                self.set_following(did, uri.clone());
                if uri.is_none() {
                    self.drop_unfollowed(did);
                }
            }
            Written::Mute { did, on } => {
                self.set_account(did, |v| v.muted = *on);
                if *on {
                    self.remove_posts_by(did);
                }
            }
            Written::Block { did, uri } => {
                let blocking = uri.clone();
                self.set_account(did, move |v| v.blocking = blocking.clone());
                if uri.is_some() {
                    self.remove_posts_by(did);
                }
            }
            // A page asked for before the delete still carries the post.
            Written::Deleted { post } => self.remove_post(post),
        }
    }

    /// Every list of posts: the timeline, the search, the profile, and the
    /// columns'.
    fn post_lists(&mut self) -> impl Iterator<Item = &mut List<Post>> {
        [
            &mut self.timeline,
            &mut self.search.posts,
            &mut self.profile.posts,
        ]
        .into_iter()
        .chain(self.columns.post_lists())
    }

    /// Every list of notifications: the tab's and the columns'.
    fn notification_lists(&mut self) -> impl Iterator<Item = &mut List<NotifItem>> {
        std::iter::once(&mut self.notifications).chain(self.columns.notification_lists())
    }

    /// Take a post out of every list it is in. A thread keeps its row, as
    /// the placeholder for a post that is not there any more, so the replies
    /// under it keep their place.
    pub(super) fn remove_post(&mut self, uri: &str) {
        for list in self.post_lists() {
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
    pub(super) fn set_like(&mut self, post: &str, like: Option<String>) {
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

    pub(super) fn set_repost(&mut self, post: &str, repost: Option<String>) {
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

    /// Apply `f` to every copy of the post `uri` on screen.
    pub(super) fn each_post(&mut self, uri: &str, mut f: impl FnMut(&mut Post)) {
        for list in self.post_lists() {
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
        for list in self.notification_lists() {
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
    pub(super) fn set_following(&mut self, did: &str, uri: Option<String>) {
        // The profile shown counts you among its followers, or not, when
        // this changes whether you follow it.
        if let Some(p) = &mut self.profile.profile
            && p.did == did
        {
            let was = p.viewer.as_ref().is_some_and(|v| v.following.is_some());
            if was != uri.is_some() {
                p.followers_count = p.followers_count.map(|n| {
                    if uri.is_some() {
                        n + 1
                    } else {
                        n.saturating_sub(1)
                    }
                });
            }
        }
        self.set_account(did, move |v| v.following = uri.clone());
    }

    /// Load the timeline and the Following columns again, and with `own` a
    /// column of your own posts: what shows posts a write just changed.
    fn reload_following(&mut self, own: bool) -> Vec<Job> {
        let me = self.session.as_ref().map(|s| s.did.clone());
        let ids: Vec<u64> = self
            .columns
            .items
            .iter()
            .filter(|c| match &c.source {
                columns::Source::Following => true,
                columns::Source::Author { did, .. } => own && Some(did) == me.as_ref(),
                _ => false,
            })
            .map(|c| c.id)
            .collect();
        let mut jobs = vec![Job::Timeline];
        for id in ids {
            jobs.extend(self.load_column(id));
        }
        jobs
    }

    /// Take an unfollowed account's posts off the timeline and the Following
    /// columns, which show followed accounts only.
    fn drop_unfollowed(&mut self, did: &str) {
        let following = columns::Source::Following;
        for list in std::iter::once(&mut self.timeline).chain(self.columns.posts_of(&following)) {
            list.retain(|p| p.author.did != did);
        }
    }

    /// The posts and notifications of an account muted or blocked leave
    /// every list, as the server leaves them out of the next pages. Its
    /// profile, where the change was made, and an open thread stay.
    pub(super) fn remove_posts_by(&mut self, did: &str) {
        let feeds = self.columns.post_lists();
        for list in [&mut self.timeline, &mut self.search.posts]
            .into_iter()
            .chain(feeds)
        {
            list.retain(|p| p.author.did != did);
        }
        for list in self.notification_lists() {
            list.retain(|item| item.n.author.did != did);
        }
        // The count on the tab is of the notifications there are.
        let gone_unread = self.unread.saturating_sub(
            self.notifications
                .items
                .iter()
                .filter(|i| !i.n.is_read)
                .count(),
        );
        self.unread -= gone_unread.min(self.unread);
    }

    /// Change what the viewer is to `did` (follow, mute, block) everywhere
    /// the account is shown.
    pub(super) fn set_account(
        &mut self,
        did: &str,
        f: impl Fn(&mut crate::api::types::ActorViewer),
    ) {
        let apply = |p: &mut Profile| {
            if p.did == did {
                f(p.viewer.get_or_insert_with(Default::default));
            }
        };
        for list in self.post_lists() {
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
        for list in self.notification_lists() {
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
    pub(super) fn fail(&mut self, e: &Error) {
        if e.message().starts_with("com.atproto.server.refreshSession") {
            // The other requests out when the session expired come back
            // with it too: the form the first brought up stays, with what
            // has been typed into it; and once logged in again, an answer to
            // a request sent before does not ask for a login again.
            if self.login.is_some() || self.answering.is_some_and(|s| s < self.session_since) {
                return;
            }
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
            self.confirm = None;
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
    pub(super) fn forget_account(&mut self) {
        // Whatever was being typed belonged to the account left behind.
        self.overlay = None;
        self.confirm = None;
        self.columns = Columns::default();
        self.chat = ChatPane::default();
        self.timeline = List::default();
        self.feeds.clear();
        self.feeds_asked = false;
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
    pub(super) fn wanted_profile(&self, p: &Profile) -> bool {
        match (&self.profile.actor, &self.session) {
            (Some(actor), _) => *actor == p.did || *actor == p.handle,
            (None, Some(s)) => s.did == p.did,
            (None, None) => false,
        }
    }

    /// Put a further page where it belongs, or drop it when that list has
    /// moved on (a new query, another profile, a refresh).
    pub(super) fn more(
        &mut self,
        feed: Feed,
        cursor: &str,
        result: crate::error::Result<MorePage>,
    ) {
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
                match feed {
                    Feed::Notifications => self.notifications.more_failed(cursor),
                    Feed::Timeline => self.timeline.more_failed(cursor),
                    // A feed's pages come only to its column.
                    Feed::Custom(_) => {}
                    Feed::SearchPosts(_) => self.search.posts.more_failed(cursor),
                    Feed::SearchActors(_) => self.search.actors.more_failed(cursor),
                    Feed::Author(_) => self.profile.posts.more_failed(cursor),
                }
                self.fail(&e);
            }
            _ => {}
        }
    }

    pub(super) fn event(&mut self, event: Event) -> Vec<Job> {
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
            Event::Muted { did, .. } => {
                self.in_flight.remove(&format!("mute:{did}"));
            }
            Event::Blocked { did, .. } | Event::Unblocked { did, .. } => {
                self.in_flight.remove(&format!("block:{did}"));
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
                self.session_since = self.sent + 1;
                let mut jobs = self.startup_jobs();
                if other {
                    self.load_columns();
                    jobs.extend(self.settle_timeline());
                } else {
                    // The same account again: its columns, which the expired
                    // session left failed or waiting, are loaded again.
                    let ids: Vec<u64> = self.columns.items.iter().map(|c| c.id).collect();
                    for id in ids {
                        jobs.extend(self.load_column(id));
                    }
                }
                return jobs;
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
                self.timeline.renew(posts);
            }
            Event::PinnedFeeds(Ok(infos)) => self.set_pinned_feeds(infos),
            // The timeline still works; + offers no feeds, and asks again
            // the next time.
            Event::PinnedFeeds(Err(_)) => self.feeds_asked = false,
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
            Event::Convos { cursor, result } => self.convos_page(cursor, result),
            Event::Messages {
                convo_id,
                cursor,
                result,
            } => return self.messages_page(&convo_id, cursor, result),
            Event::MessageSent {
                convo_id,
                text,
                result,
            } => {
                let open = self.chat.open.as_mut().filter(|o| o.convo.id == convo_id);
                match (open, result) {
                    (Some(o), Ok(m)) => {
                        o.sending = false;
                        // Only what was sent leaves the box: what was typed
                        // after Enter, or a new draft in the conversation
                        // opened again, stays.
                        let draft = o.input.text();
                        if let Some(rest) = draft.strip_prefix(&text) {
                            o.input = TextInput::single(rest.trim_start());
                        }
                        let last = m.clone();
                        o.sent(m);
                        if let Some(c) =
                            self.chat.convos.items.iter_mut().find(|c| c.id == convo_id)
                        {
                            c.last_message = Some(last);
                        }
                    }
                    (None, Ok(_)) => self.info("message sent"),
                    (Some(o), Err(e)) => {
                        // The text stays in the box, to send again or change.
                        o.sending = false;
                        self.fail(&e);
                    }
                    (None, Err(e)) => self.fail(&e),
                }
            }
            Event::ConvoFor { did, result } => match result {
                Ok(convo) => {
                    if convo.members.iter().any(|m| m.did == did) {
                        // Only while their profile is still what is looked
                        // at: after a move elsewhere the answer would take
                        // the screen, and mark the conversation read.
                        let still_there = self.tab == Tab::Profile
                            && self.threads.is_empty()
                            && self.overlay.is_none()
                            && self.profile.profile.as_ref().is_some_and(|p| p.did == did);
                        if still_there {
                            return self.open_convo(convo);
                        }
                        if !self.chat.convos.items.iter().any(|c| c.id == convo.id) {
                            self.chat.convos.push_front(convo);
                        }
                        self.info("the conversation is ready on the Chat tab");
                    }
                }
                Err(e) => self.fail(&e),
            },
            Event::ConvoRead { convo_id, result } => match result {
                Ok(()) => {
                    if let Some(c) = self.chat.convos.items.iter_mut().find(|c| c.id == convo_id) {
                        c.unread_count = 0;
                    }
                }
                Err(e) => self.fail(&e),
            },
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
                // need not be on top (one can be opened over a reload). Every
                // view of that post still waiting takes it: the answer to the
                // other one's request, older, is dropped as superseded.
                let waiting = |t: &ThreadView| t.uri == uri && !t.list.loaded;
                if !self.threads.iter().any(waiting) {
                    return Vec::new();
                }
                match result {
                    Ok(node) => {
                        let (rows, focus) = thread_rows::flatten(node);
                        for th in self.threads.iter_mut().filter(|t| waiting(t)) {
                            // A reload keeps the row that was selected, as
                            // every other list does; only a first load jumps
                            // to the post the thread was opened on.
                            let was = th
                                .list
                                .items
                                .get(th.list.selected)
                                .map(|r| r.key().to_string());
                            th.list.selected = was
                                .and_then(|key| rows.iter().position(|r| r.key() == key))
                                .unwrap_or(focus);
                            th.list.items = rows.clone();
                            // The view scrolls the selection into sight from here.
                            th.list.offset = th.list.offset.min(th.list.selected);
                            th.list.loaded = true;
                        }
                    }
                    Err(e) => {
                        for th in self.threads.iter_mut().filter(|t| waiting(t)) {
                            th.list.loaded = true;
                            th.error = Some(e.message().to_string());
                        }
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
                self.drop_unfollowed(&did);
                self.info("unfollowed");
            }
            Event::Muted {
                did,
                on,
                result: Ok(()),
            } => {
                self.set_account(&did, |v| v.muted = on);
                if on {
                    self.remove_posts_by(&did);
                    self.info("muted: their posts leave your lists; M on their profile unmutes");
                } else {
                    self.info("unmuted");
                    return self.reload_following(false);
                }
            }
            Event::Blocked {
                did,
                result: Ok(uri),
            } => {
                self.set_account(&did, move |v| v.blocking = Some(uri.clone()));
                self.remove_posts_by(&did);
                self.info("blocked: B on their profile unblocks");
            }
            Event::Unblocked {
                did,
                result: Ok(()),
            } => {
                self.set_account(&did, |v| v.blocking = None);
                self.info("unblocked");
                return self.reload_following(false);
            }
            Event::Posted {
                reply_to,
                result: Ok(()),
            } => {
                self.overlay = None;
                // Your own posts are on the timeline, in a column of them
                // and on your profile: load those again so the new one is
                // there, and a thread it answers in, so it shows under the
                // post.
                let mut jobs = self.reload_following(true);
                let me = self.session.as_ref().map(|s| s.did.clone());
                if let Some(me) = me
                    && self.profile.actor.is_none()
                    && (self.tab == Tab::Profile
                        || self.profile.profile.as_ref().is_some_and(|p| p.did == me))
                {
                    jobs.push(Job::OpenProfile(me));
                }
                match reply_to {
                    Some(uri) => {
                        self.each_post(&uri, |p| p.reply_count += 1);
                        self.info("reply sent");
                        jobs.extend(
                            self.threads
                                .iter()
                                .filter(|th| {
                                    th.list
                                        .items
                                        .iter()
                                        .any(|r| r.post().is_some_and(|p| p.uri == uri))
                                })
                                .map(|th| Job::Thread(th.uri.clone())),
                        );
                    }
                    None => self.info("posted"),
                }
                return jobs;
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
                            e.loaded = [e.fields[0].text(), e.fields[1].text()];
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
            // An error of a profile left for another since is not that one's
            // (an answer that came is checked by who it is about).
            Event::Profile(Err(_)) if self.answering.is_some_and(|s| s < self.profile_asked) => {}
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
            | Event::Unfollowed { result: Err(e), .. }
            | Event::Muted { result: Err(e), .. }
            | Event::Blocked { result: Err(e), .. }
            | Event::Unblocked { result: Err(e), .. } => self.fail(&e),
        }
        Vec::new()
    }

    /// Handle a finished job.
    #[cfg(test)]
    pub fn handle_event(&mut self, event: Event) -> Vec<Job> {
        // As if asked for just now: nothing sent since can be newer.
        self.answer(None, event)
    }
}
