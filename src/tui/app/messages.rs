//! The Chat tab: the conversations, the one open, writing, and reading again while it is shown.

use super::*;

impl App {
    /// A key on the Chat tab, when it is the tab's to take: in an open
    /// conversation (typing, scrolling, leaving it) and Enter on the list.
    /// Everything else goes on to the keys every tab has.
    pub(super) fn chat_key(&mut self, key: KeyEvent) -> Option<Vec<Job>> {
        let Some(open) = &mut self.chat.open else {
            if key.code == KeyCode::Enter {
                let convo = self.chat.convos.current().cloned()?;
                return Some(self.open_convo(convo));
            }
            return None;
        };
        if open.typing {
            match key.code {
                KeyCode::Esc => open.typing = false,
                KeyCode::Enter if !open.sending => {
                    let text = open.input.text();
                    if text.trim().is_empty() {
                        return Some(Vec::new());
                    }
                    if let Some(why) = crate::api::message_length_problem(text.trim_end()) {
                        self.error(why);
                        return Some(Vec::new());
                    }
                    open.sending = true;
                    return Some(vec![Job::SendMessage {
                        convo_id: open.convo.id.clone(),
                        text,
                    }]);
                }
                KeyCode::Enter => {}
                _ => {
                    open.input.handle_key(key);
                }
            }
            return Some(Vec::new());
        }
        match key.code {
            // Closed while its message is on its way, the conversation opened
            // again would take that message's answer as its own.
            KeyCode::Esc if open.sending => {
                self.info(n!("the message is still on its way"));
                Some(Vec::new())
            }
            KeyCode::Esc => {
                self.chat.open = None;
                Some(Vec::new())
            }
            KeyCode::Enter | KeyCode::Char('i') => {
                open.typing = true;
                Some(Vec::new())
            }
            KeyCode::Char('k') | KeyCode::Up => {
                open.scroll += 1;
                Some(self.older_messages())
            }
            KeyCode::Char('j') | KeyCode::Down => {
                open.scroll = open.scroll.saturating_sub(1);
                Some(Vec::new())
            }
            KeyCode::Char('g') | KeyCode::Home => {
                open.scroll = usize::MAX / 2;
                Some(self.older_messages())
            }
            KeyCode::Char('G') | KeyCode::End => {
                open.scroll = 0;
                Some(Vec::new())
            }
            _ => None,
        }
    }

    /// Scrolled up to the oldest message: ask for the ones before it.
    pub(super) fn older_messages(&mut self) -> Vec<Job> {
        let Some(open) = &mut self.chat.open else {
            return Vec::new();
        };
        // At the top of what is drawn, or scrolled past every message.
        let top = open.at_top || open.scroll >= open.messages.len();
        if open.loading_older || !top {
            return Vec::new();
        }
        let Some(cursor) = open.older.clone() else {
            return Vec::new();
        };
        open.loading_older = true;
        vec![Job::Messages {
            convo_id: open.convo.id.clone(),
            cursor: Some(cursor),
        }]
    }

    /// Open `convo`: its latest messages are asked for, and it is marked
    /// read now that it is looked at, not when the list was loaded.
    pub(super) fn open_convo(&mut self, convo: Convo) -> Vec<Job> {
        self.tab = Tab::Chat;
        self.threads.clear();
        // The conversation open already stays as it is, its draft and a
        // message on its way with it; only its latest messages are asked for.
        if self
            .chat
            .open
            .as_ref()
            .is_some_and(|o| o.convo.id == convo.id)
        {
            let mut jobs = vec![Job::Messages {
                convo_id: convo.id.clone(),
                cursor: None,
            }];
            if convo.unread_count > 0 {
                jobs.push(Job::ReadConvo { convo_id: convo.id });
            }
            self.chat.polled = Some(Instant::now());
            return jobs;
        }
        let mut jobs = vec![Job::Messages {
            convo_id: convo.id.clone(),
            cursor: None,
        }];
        if convo.unread_count > 0 {
            jobs.push(Job::ReadConvo {
                convo_id: convo.id.clone(),
            });
        }
        // Opened from a profile before the tab was ever shown: the list is
        // asked for too, for Esc to go back to.
        if !self.chat.convos.loaded && !self.chat.convos.loading {
            self.chat.convos.begin();
            jobs.push(Job::Convos { cursor: None });
        }
        if !self.chat.convos.items.iter().any(|c| c.id == convo.id) {
            self.chat.convos.push_front(convo.clone());
        }
        self.chat.open = Some(OpenConvo::new(convo));
        self.chat.polled = Some(Instant::now());
        jobs
    }

    /// `m` on someone's profile: the conversation with them.
    pub(super) fn message_profile(&mut self) -> Vec<Job> {
        let Some(p) = self.profile.profile.clone() else {
            return Vec::new();
        };
        if self.profile.actor.is_none() || self.session.as_ref().is_some_and(|s| s.did == p.did) {
            self.info(n!("m opens a conversation with someone else's profile"));
            return Vec::new();
        }
        self.info(tf("opening the conversation with @{}…", &[&p.handle]));
        vec![Job::ConvoFor { did: p.did }]
    }

    /// Read the Chat tab again: the open conversation's latest messages, or
    /// the list.
    pub(super) fn read_chat(&mut self) -> Vec<Job> {
        self.chat.polled = Some(Instant::now());
        match &self.chat.open {
            Some(o) => vec![Job::Messages {
                convo_id: o.convo.id.clone(),
                cursor: None,
            }],
            None => {
                self.chat.convos.loading = true;
                vec![Job::Convos { cursor: None }]
            }
        }
    }

    /// While the Chat tab is shown, read it again every
    /// [`chat::POLL_EVERY`]; nothing is read while another tab is.
    pub fn poll_chat(&mut self, now: Instant) -> Vec<Job> {
        if self.tab != Tab::Chat || self.login.is_some() || self.chat.refused.is_some() {
            return Vec::new();
        }
        if self
            .chat
            .polled
            .is_some_and(|at| now.saturating_duration_since(at) < chat::POLL_EVERY)
        {
            return Vec::new();
        }
        let jobs = self.read_chat();
        self.chat.polled = Some(now);
        self.pending += jobs.len();
        jobs
    }

    pub(super) fn convos_page(
        &mut self,
        cursor: Option<String>,
        result: crate::error::Result<Page<Convo>>,
    ) {
        match (cursor, result) {
            (None, Ok(page)) => {
                self.chat.refused = None;
                self.chat.convos.renew(page);
            }
            (Some(at), Ok(page)) => self.chat.convos.append(&at, page),
            (cursor, Err(e)) => {
                if e.message().contains("scope") || e.message().contains("Scope") {
                    self.chat.convos.loaded = true;
                    self.chat.convos.loading = false;
                    self.chat.refused = Some(
                        n!("this app password cannot read direct messages. Make one with \"Allow access to your direct messages\" (Settings, Privacy and security, App passwords) and log in with it (A, then a).")
                            .into(),
                    );
                    return;
                }
                match cursor {
                    None => self.chat.convos.failed(&e),
                    Some(at) => self.chat.convos.more_failed(&at),
                }
                self.fail(&e);
            }
        }
    }

    pub(super) fn messages_page(
        &mut self,
        convo_id: &str,
        cursor: Option<String>,
        result: crate::error::Result<Page<ChatMessage>>,
    ) -> Vec<Job> {
        let me = self
            .session
            .as_ref()
            .map(|s| s.did.clone())
            .unwrap_or_default();
        let shown = self.tab == Tab::Chat;
        // A conversation left or changed since: its page is dropped.
        let Some(open) = self.chat.open.as_mut().filter(|o| o.convo.id == convo_id) else {
            return Vec::new();
        };
        match (cursor, result) {
            (None, Ok(page)) => {
                let read_before = open.loaded;
                let had = open.messages.len();
                open.take_latest(page.items, page.cursor);
                // Messages from the others that came while it is read are
                // read: marked so, they do not come back as unread. The first
                // page was marked when the conversation was opened.
                let theirs = open.messages[had..].iter().any(|m| m.sender != me);
                if read_before && theirs && shown {
                    return vec![Job::ReadConvo {
                        convo_id: convo_id.to_string(),
                    }];
                }
            }
            (Some(at), Ok(page)) => open.take_older(&at, page.items, page.cursor),
            (cursor, Err(e)) => {
                match cursor {
                    None => {
                        open.loaded = true;
                        open.error = Some(e.message().to_string());
                    }
                    Some(_) => open.loading_older = false,
                }
                self.fail(&e);
            }
        }
        Vec::new()
    }
}
