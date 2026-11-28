//! Direct conversations with a dedicated input that cannot change visibility.
use super::*;
use crate::api::types::Conversation;
use crate::api::{Next, Page};
use crate::tui::list::Keyed;
use crate::tui::worker::Thread;

impl Keyed for Conversation {
    fn key(&self) -> &str {
        &self.id
    }
}
#[derive(Default)]
pub(crate) struct Chat {
    pub conversations: List<Conversation>,
    pub open: Option<OpenChat>,
    polled: Option<Instant>,
    reading: HashMap<u64, String>,
}
pub(crate) struct OpenChat {
    pub id: Option<String>,
    pub accounts: Vec<Account>,
    pub posts: List<Status>,
    pub input: TextInput,
    pub typing: bool,
    pub sending: Option<u64>,
    mark_after_load: bool,
}
impl App {
    pub(super) fn load_chat(&mut self, more: bool) -> Jobs {
        if self.session.is_none() {
            return Vec::new();
        }
        self.chat.polled = Some(Instant::now());
        if let Some(open) = &self.chat.open {
            if open.posts.waiting.is_some() || open.sending.is_some() {
                return Vec::new();
            }
            let Some(last) = open.posts.items.last() else {
                return Vec::new();
            };
            let job = self.job(Job::ChatThread {
                id: last.id.clone(),
            });
            if let Some(open) = &mut self.chat.open {
                open.posts.waiting = Some(job.0);
            }
            return vec![job];
        }
        let next = if more {
            self.chat.conversations.next.clone()
        } else {
            None
        };
        let job = self.job(Job::Conversations { next });
        self.chat.conversations.waiting = Some(job.0);
        vec![job]
    }
    pub(crate) fn poll_chat(&mut self, now: Instant) -> Jobs {
        if self.tab == Tab::Chat
            && self.overlay.is_none()
            && self.threads.is_empty()
            && self.chat.conversations.waiting.is_none()
            && self
                .chat
                .polled
                .is_none_or(|at| now.saturating_duration_since(at) >= Duration::from_secs(15))
        {
            return self.load_chat(false);
        }
        Vec::new()
    }
    pub(super) fn message_profile(&mut self) {
        if self.tab != Tab::Profile || !self.threads.is_empty() {
            return;
        }
        if self.session.is_none() {
            self.needs_account();
            return;
        }
        let Some(account) = self.profile.account.clone().filter(|a| !self.own(a)) else {
            return;
        };
        self.tab = Tab::Chat;
        self.chat.open = Some(OpenChat {
            id: None,
            accounts: vec![account],
            posts: List {
                loaded: true,
                ..List::default()
            },
            input: TextInput::single(""),
            typing: true,
            sending: None,
            mark_after_load: false,
        });
    }
    pub(super) fn chat_key(&mut self, key: KeyEvent) -> Option<Jobs> {
        if self.tab != Tab::Chat || !self.threads.is_empty() {
            return None;
        }
        if self.chat.open.as_ref().is_some_and(|o| o.typing) {
            if key.code == KeyCode::Esc {
                if let Some(o) = &mut self.chat.open {
                    o.typing = false;
                }
            } else if key.code == KeyCode::Enter {
                return Some(self.send_chat());
            } else if let Some(o) = &mut self.chat.open
                && o.sending.is_none()
            {
                o.input.handle_key(key);
            }
            return Some(Vec::new());
        }
        match key.code {
            KeyCode::Esc if self.chat.open.is_some() => {
                self.chat.open = None;
                Some(self.load_chat(false))
            }
            KeyCode::Char('i' | 'r') if self.chat.open.is_some() => {
                if let Some(open) = &mut self.chat.open {
                    open.typing = true;
                }
                Some(Vec::new())
            }
            KeyCode::Enter if self.chat.open.is_none() => {
                let Some(c) = self.chat.conversations.current().cloned() else {
                    return Some(Vec::new());
                };
                let Some(last) = c.last_status.filter(|s| s.visibility == "direct") else {
                    return Some(Vec::new());
                };
                let job = self.job(Job::ChatThread {
                    id: last.id.clone(),
                });
                self.chat.open = Some(OpenChat {
                    id: Some(c.id),
                    accounts: c.accounts,
                    posts: List {
                        items: vec![last],
                        waiting: Some(job.0),
                        ..List::default()
                    },
                    input: TextInput::single(""),
                    typing: false,
                    sending: None,
                    mark_after_load: c.unread,
                });
                Some(vec![job])
            }
            _ => None,
        }
    }
    pub(super) fn step_chat(&mut self, by: isize) -> Jobs {
        if let Some(open) = &mut self.chat.open {
            open.posts.step(by);
            return Vec::new();
        }
        self.chat.conversations.step(by);
        if self.chat.conversations.wants_more() {
            self.load_chat(true)
        } else {
            Vec::new()
        }
    }
    fn send_chat(&mut self) -> Jobs {
        let Some(open) = &self.chat.open else {
            return Vec::new();
        };
        if open.sending.is_some() || open.posts.waiting.is_some() {
            return Vec::new();
        }
        let text = open.input.text();
        if text.trim().is_empty() {
            return Vec::new();
        }
        let recipients: Vec<_> = open
            .accounts
            .iter()
            .filter(|a| !self.own(a) && !a.acct.is_empty())
            .map(|a| format!("@{}", a.acct))
            .collect();
        if recipients.is_empty() {
            self.fail(t("no recipients in this conversation"));
            return Vec::new();
        }
        let text = format!("{} {}", recipients.join(" "), text);
        if crate::cli::counted(&text, self.limits.characters_reserved_per_url)
            > self.limits.max_characters
        {
            self.fail(t("message is too long"));
            return Vec::new();
        }
        let draft = Draft {
            files: Vec::new(),
            text,
            cw: String::new(),
            visibility: Visibility::Direct,
            reply: open.posts.items.last().map(|s| s.id.clone()),
            quote: None,
            quote_policy: None,
        };
        let job = self.job(Job::ChatPost(Box::new(draft)));
        if let Some(open) = &mut self.chat.open {
            open.sending = Some(job.0);
        }
        vec![job]
    }
    pub(super) fn conversations_answer(
        &mut self,
        seq: u64,
        first: bool,
        result: Result<Page<Conversation>>,
    ) {
        let page = result.map(|p| {
            let next = match p.next {
                Next::MaxId(id) => Some(id),
                _ => None,
            };
            (p.items, next)
        });
        self.chat.conversations.take(seq, first, page);
    }
    pub(super) fn chat_thread_answer(&mut self, seq: u64, result: Result<Box<Thread>>) -> Jobs {
        let Some(open) = &mut self.chat.open else {
            return Vec::new();
        };
        if open.posts.waiting != Some(seq) {
            return Vec::new();
        }
        let success = result.is_ok();
        let me = self.session.as_ref();
        let page = result.map(|t| {
            let posts = t
                .ancestors
                .into_iter()
                .chain([t.status])
                .chain(t.descendants)
                .filter(|s| {
                    crate::api::same_conversation(
                        s,
                        &open.accounts,
                        me.map(|s| (s.user_id.as_str(), s.username.as_str())),
                    )
                })
                .collect();
            (posts, None)
        });
        let first = !open.posts.loaded;
        open.posts.take(seq, true, page);
        if first {
            open.posts.select(open.posts.items.len().saturating_sub(1));
        }
        let mark = success && std::mem::take(&mut open.mark_after_load);
        let id = open.id.clone();
        if mark && let Some(id) = id {
            if self.chat.reading.values().any(|v| v == &id) {
                return Vec::new();
            }
            let job = self.job(Job::ReadConversation { id: id.clone() });
            for c in &mut self.chat.conversations.items {
                if c.id == id {
                    c.unread = false;
                }
            }
            self.chat.reading.insert(job.0, id);
            return vec![job];
        }
        Vec::new()
    }
    pub(super) fn chat_read_answer(&mut self, seq: u64, result: Result<()>) {
        let Some(id) = self.chat.reading.remove(&seq) else {
            return;
        };
        if let Err(e) = result {
            for c in &mut self.chat.conversations.items {
                if c.id == id {
                    c.unread = true;
                }
            }
            self.fail(e.message().to_owned());
        }
    }
    pub(super) fn chat_post_answer(&mut self, seq: u64, result: Result<Box<Status>>) {
        let Some(open) = &mut self.chat.open else {
            return;
        };
        if open.sending != Some(seq) {
            return;
        }
        open.sending = None;
        match result {
            Ok(s) => {
                open.posts.items.push(*s);
                open.posts.select(open.posts.items.len() - 1);
                open.input = TextInput::single("");
                self.say(t("message sent"));
            }
            Err(e) => self.fail(e.message().to_owned()),
        }
    }
}
