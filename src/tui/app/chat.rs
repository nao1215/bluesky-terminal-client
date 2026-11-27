//! The Chat tab, as bsky's: the conversations (`enter` opens one, which
//! is read again every 15 s while shown), the messages of one (`i` writes,
//! `enter` sends, `esc` stops writing and goes back), with Misskey's chat
//! (`chat/history`, `chat/messages/*`). A server without chat, or a role
//! that may not chat, takes the tab away and says why.

use super::*;
use crate::api::types::ChatMessage;
use crate::tui::input::TextInput;
use crate::tui::worker::ChatWith;

/// How often the open tab is read again, as bsky's.
pub const CHAT_POLL: Duration = Duration::from_secs(15);

impl Keyed for ChatMessage {
    fn key(&self) -> &str {
        &self.id
    }
}

/// A conversation shown.
#[derive(Debug, Clone)]
pub struct OpenChat {
    pub with: ChatWith,
    /// Who or what it is with, as the list names it.
    pub title: String,
    /// The messages, newest first, as the server pages them.
    pub messages: List<ChatMessage>,
    pub input: TextInput,
    pub typing: bool,
    pub sending: bool,
}

/// The Chat tab.
#[derive(Debug, Clone, Default)]
pub struct ChatPane {
    /// The latest message of each conversation, the newest first.
    pub convos: List<ChatMessage>,
    pub open: Option<OpenChat>,
    /// When it was last read, for the 15 s polling.
    pub polled: Option<Instant>,
}

/// Whom message `m` is a conversation with, seen from `me`, and its title.
pub fn partner(m: &ChatMessage, me: Option<&str>) -> (ChatWith, String) {
    if let Some(room) = &m.to_room {
        return (ChatWith::Room(room.id.clone()), format!("# {}", room.name));
    }
    if let Some(r) = m.to_room_id.as_deref() {
        return (ChatWith::Room(r.to_string()), format!("# {r}"));
    }
    let from_me = Some(m.from_user_id.as_str()) == me;
    let other = if from_me {
        m.to_user.as_ref()
    } else {
        m.from_user.as_ref()
    };
    let id = if from_me {
        m.to_user_id.clone().unwrap_or_default()
    } else {
        m.from_user_id.clone()
    };
    let title = other.map_or_else(
        || id.clone(),
        |u| match u.host.as_deref().filter(|h| !h.is_empty()) {
            Some(h) => format!("@{}@{h}", u.username),
            None => format!("@{}", u.username),
        },
    );
    (ChatWith::User(id), title)
}

impl App {
    /// The tab arrived at: the conversations, read the first time.
    pub(super) fn show_chat(&mut self) -> Jobs {
        if self.session.is_none() {
            let host = self.host().to_string();
            self.chat.convos.needs_account(format!(
                "no account is logged in on {host}: press A then a to log in (or run `msky login --host {host}`)"
            ));
            return Vec::new();
        }
        if !self.chat.convos.loaded && !self.chat.convos.loading {
            return self.load_chat();
        }
        Vec::new()
    }

    fn load_chat(&mut self) -> Jobs {
        let (seq, job) = self.stamp(Job::ChatHistory);
        self.chat.convos.begin();
        self.chat.convos.asked = seq;
        self.chat.polled = Some(Instant::now());
        vec![(seq, job)]
    }

    /// Every 15 s while the tab is shown: the list, or the conversation
    /// open, read again, as bsky's.
    pub fn poll_chat(&mut self, now: Instant) -> Jobs {
        if self.tab != Tab::Chat || self.session.is_none() || self.overlay.is_some() {
            return Vec::new();
        }
        if self
            .chat
            .polled
            .is_some_and(|t| now.duration_since(t) < CHAT_POLL)
        {
            return Vec::new();
        }
        self.chat.polled = Some(now);
        match &self.chat.open {
            Some(o) if !o.sending && !o.messages.loading => {
                let with = o.with.clone();
                self.load_messages(with)
            }
            Some(_) => Vec::new(),
            None if !self.chat.convos.loading => self.load_chat(),
            None => Vec::new(),
        }
    }

    fn load_messages(&mut self, with: ChatWith) -> Jobs {
        let (seq, job) = self.stamp(Job::ChatMessages { with, until: None });
        if let Some(o) = &mut self.chat.open {
            o.messages.begin();
            o.messages.asked = seq;
        }
        vec![(seq, job)]
    }

    /// A key on the Chat tab; `None` when the tab's keys do not take it.
    pub(super) fn chat_key(&mut self, key: KeyEvent) -> Option<Jobs> {
        let me = self.my_user_id().map(str::to_string);
        if let Some(o) = &mut self.chat.open {
            if o.typing {
                match key.code {
                    KeyCode::Esc => o.typing = false,
                    KeyCode::Enter if !o.sending => {
                        let text = o.input.text();
                        if text.trim().is_empty() {
                            return Some(Vec::new());
                        }
                        if !self.may("write:chat", "send messages") {
                            return Some(Vec::new());
                        }
                        let Some(o) = &mut self.chat.open else {
                            return Some(Vec::new());
                        };
                        o.sending = true;
                        let with = o.with.clone();
                        return Some(vec![self.stamp(Job::ChatSend {
                            with,
                            text: text.trim().to_string(),
                        })]);
                    }
                    _ if o.sending => {}
                    _ => {
                        o.input.handle_key(key);
                    }
                }
                return Some(Vec::new());
            }
            return match key.code {
                KeyCode::Esc => {
                    self.chat.open = None;
                    Some(Vec::new())
                }
                KeyCode::Char('i') => {
                    o.typing = true;
                    Some(Vec::new())
                }
                // Older messages, further back.
                KeyCode::Char('k') | KeyCode::Up => {
                    o.messages.step(1);
                    let with = o.with.clone();
                    Some(match o.messages.want_more() {
                        Some(until) => vec![self.stamp(Job::ChatMessages {
                            with,
                            until: Some(until),
                        })],
                        None => Vec::new(),
                    })
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    o.messages.step(-1);
                    Some(Vec::new())
                }
                KeyCode::Char('R') | KeyCode::F(5) => {
                    let with = o.with.clone();
                    Some(self.load_messages(with))
                }
                _ => None,
            };
        }
        match key.code {
            KeyCode::Enter => {
                let m = self.chat.convos.current()?;
                let (with, title) = partner(m, me.as_deref());
                self.chat.open = Some(OpenChat {
                    with: with.clone(),
                    title,
                    messages: List::default(),
                    input: TextInput::single(""),
                    typing: false,
                    sending: false,
                });
                Some(self.load_messages(with))
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.chat.convos.step(1);
                Some(Vec::new())
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.chat.convos.step(-1);
                Some(Vec::new())
            }
            KeyCode::Char('R') | KeyCode::F(5) => Some(self.load_chat()),
            _ => None,
        }
    }

    /// Answers for the Chat tab.
    pub(super) fn chat_answer(&mut self, seq: u64, event: Event) {
        match event {
            Event::ChatHistory(result) => {
                if self.chat.convos.asked != seq {
                    return;
                }
                match result {
                    Ok(list) => {
                        // One entry a conversation: the newest message of each.
                        let me = self.my_user_id().map(str::to_string);
                        let mut seen = HashSet::new();
                        let list: Vec<ChatMessage> = list
                            .into_iter()
                            .filter(|m| seen.insert(partner(m, me.as_deref()).0))
                            .collect();
                        self.chat.convos.set(list);
                        // A list of conversations has no next page.
                        self.chat.convos.cursor = None;
                    }
                    Err(e) if crate::api::chat_unavailable(&e) => {
                        self.tabs.retain(|t| *t != Tab::Chat);
                        if self.tab == Tab::Chat {
                            self.tab = self.tabs[0];
                        }
                        self.error(format!(
                            "{} has no chat for this account ({}); the Chat tab is taken away",
                            self.host(),
                            e.message()
                        ));
                    }
                    Err(e) => self.chat.convos.failed(said(&e)),
                }
            }
            Event::ChatMessages {
                with,
                until,
                result,
            } => {
                let Some(o) = self.chat.open.as_mut().filter(|o| o.with == with) else {
                    return;
                };
                match (until, result) {
                    (None, Ok(m)) if o.messages.asked == seq => {
                        // Read again, the newest stays in sight.
                        o.messages.set(m);
                        o.messages.selected = 0;
                    }
                    (None, Err(e)) if o.messages.asked == seq => o.messages.failed(said(&e)),
                    (Some(u), Ok(m)) => o.messages.append(&u, m),
                    (Some(u), Err(e)) => {
                        o.messages.more_failed(&u);
                        let why = said(&e);
                        self.error(why);
                    }
                    _ => {}
                }
            }
            Event::ChatSent { with, text, result } => {
                let Some(o) = self.chat.open.as_mut().filter(|o| o.with == with) else {
                    return;
                };
                o.sending = false;
                match result {
                    Ok(m) => {
                        o.input = TextInput::single("");
                        if !o.messages.items.iter().any(|x| x.id == m.id) {
                            o.messages.items.insert(0, *m);
                        }
                        o.messages.selected = 0;
                    }
                    Err(e) => {
                        // What was typed stays, to send again.
                        o.input = TextInput::single(&text);
                        let why = said(&e);
                        self.error(why);
                    }
                }
            }
            _ => {}
        }
    }
}
