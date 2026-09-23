//! Answers from the worker: which are taken, what they change, and writes confirmed shown again on what a late read brought.

use super::*;

impl App {
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
    pub(super) fn answer(&mut self, seq: Option<u64>, event: Event) -> Vec<Job> {
        self.pending = self.pending.saturating_sub(1);
        let read = seq.filter(|s| self.reads_out.remove(s));
        if let Some(seq) = read
            && self.superseded(seq, &event)
        {
            self.forget_writes();
            return Vec::new();
        }
        // A write the account before made (it is sent as that account) is
        // not the one in use's: a like shown on its lists would be undone
        // with a record it does not own. A download or a link opened
        // belongs to no account.
        if let Some(seq) = seq
            && seq < self.account_since
            && !matches!(
                event,
                Event::Downloaded(_) | Event::Opened { .. } | Event::LoggedIn(_)
            )
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
    pub(super) fn superseded(&mut self, seq: u64, event: &Event) -> bool {
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
    pub(super) fn forget_writes(&mut self) {
        match self.reads_out.first().copied() {
            Some(oldest) => self.written.retain(|(at, _)| *at > oldest),
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
    pub(super) fn remove_post(&mut self, uri: &str) {
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
    pub(super) fn set_following(&mut self, did: &str, uri: Option<String>) {
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
    pub(super) fn fail(&mut self, e: &Error) {
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
    pub(super) fn forget_account(&mut self) {
        // Whatever was being typed belonged to the account left behind.
        self.overlay = None;
        self.confirm_delete = None;
        self.confirm_logout = None;
        self.confirm_column_remove = None;
        self.columns = Columns::default();
        self.chat = ChatPane::default();
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
            Event::Convos { cursor, result } => self.convos_page(cursor, result),
            Event::Messages {
                convo_id,
                cursor,
                result,
            } => self.messages_page(&convo_id, cursor, result),
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
                    if convo
                        .others(self.session.as_ref().map_or("", |s| s.did.as_str()))
                        .iter()
                        .any(|m| m.did == did)
                        || convo.members.iter().any(|m| m.did == did)
                    {
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
                            self.chat.convos.items.insert(0, convo);
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

    /// Handle a finished job.
    #[cfg(test)]
    pub fn handle_event(&mut self, event: Event) -> Vec<Job> {
        // As if asked for just now: nothing sent since can be newer.
        self.answer(None, event)
    }
}
