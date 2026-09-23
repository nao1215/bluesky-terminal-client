//! The Chat tab's state: the conversations, and the one open with its
//! messages and the message being typed.

use std::collections::HashSet;
use std::time::Instant;

use crate::api::types::{ChatMessage, Convo};
use crate::tui::app::List;
use crate::tui::input::TextInput;

/// How often the open conversation, or the list, is read again while the
/// Chat tab is shown.
pub const POLL_EVERY: std::time::Duration = std::time::Duration::from_secs(15);

/// The Chat tab.
#[derive(Debug, Clone, Default)]
pub struct ChatPane {
    pub convos: List<Convo>,
    pub open: Option<OpenConvo>,
    /// When the tab last read the server by itself.
    pub polled: Option<Instant>,
    /// Why the conversations cannot be read at all (an app password without
    /// access to direct messages), shown instead of the list.
    pub refused: Option<String>,
}

impl ChatPane {
    /// Unread messages over every conversation, for the tab's title.
    pub fn unread(&self) -> u64 {
        self.convos
            .items
            .iter()
            .filter(|c| !c.muted)
            .map(|c| c.unread_count)
            .sum()
    }
}

/// An open conversation.
#[derive(Debug, Clone)]
pub struct OpenConvo {
    pub convo: Convo,
    /// Oldest first, as they are read.
    pub messages: Vec<ChatMessage>,
    /// Where older messages begin; `None` once there are none.
    pub older: Option<String>,
    /// Older messages have been asked for and not answered yet.
    pub loading_older: bool,
    /// Whether the latest messages have arrived.
    pub loaded: bool,
    pub error: Option<String>,
    /// Lines scrolled up from the newest message.
    pub scroll: usize,
    pub input: TextInput,
    /// Keys go to the message box.
    pub typing: bool,
    /// A message is on its way.
    pub sending: bool,
}

impl OpenConvo {
    pub fn new(convo: Convo) -> Self {
        Self {
            convo,
            messages: Vec::new(),
            older: None,
            loading_older: false,
            loaded: false,
            error: None,
            scroll: 0,
            input: TextInput::single(""),
            typing: false,
            sending: false,
        }
    }

    /// Take the latest messages (newest first, as the server gives them):
    /// the ones not here yet go after the others. Older ones read earlier
    /// stay, so reading again while scrolled up loses nothing.
    pub fn take_latest(&mut self, newest_first: Vec<ChatMessage>, cursor: Option<String>) {
        let first = !self.loaded;
        let known: HashSet<String> = self.messages.iter().map(|m| m.id.clone()).collect();
        let mut new: Vec<ChatMessage> = newest_first
            .into_iter()
            .filter(|m| !known.contains(&m.id))
            .collect();
        new.reverse();
        self.messages.extend(new);
        self.loaded = true;
        self.error = None;
        if first {
            self.older = cursor;
        }
    }

    /// Take a page of older messages (newest first): they go before the
    /// others.
    pub fn take_older(
        &mut self,
        requested: &str,
        newest_first: Vec<ChatMessage>,
        cursor: Option<String>,
    ) {
        if !self.loading_older || self.older.as_deref() != Some(requested) {
            return;
        }
        self.loading_older = false;
        let known: HashSet<String> = self.messages.iter().map(|m| m.id.clone()).collect();
        let mut older: Vec<ChatMessage> = newest_first
            .into_iter()
            .filter(|m| !known.contains(&m.id))
            .collect();
        older.reverse();
        older.append(&mut self.messages);
        self.messages = older;
        self.older = cursor.filter(|c| c != requested);
    }

    /// A message this account just sent, shown at once.
    pub fn sent(&mut self, m: ChatMessage) {
        if !self.messages.iter().any(|x| x.id == m.id) {
            self.messages.push(m);
        }
        self.scroll = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &str) -> ChatMessage {
        ChatMessage {
            id: id.into(),
            text: format!("text {id} 👨\u{200d}👩\u{200d}👧"),
            sender: "did:plc:a".into(),
            sent_at: "2026-09-20T10:00:00Z".into(),
            ..ChatMessage::default()
        }
    }

    fn ids(o: &OpenConvo) -> Vec<&str> {
        o.messages.iter().map(|m| m.id.as_str()).collect()
    }

    #[test]
    fn messages_are_kept_oldest_first_and_reading_again_adds_only_the_new() {
        let mut o = OpenConvo::new(Convo::default());
        o.take_latest(vec![m("3"), m("2")], Some("c1".into()));
        assert_eq!(ids(&o), ["2", "3"]);
        assert_eq!(o.older.as_deref(), Some("c1"));
        // Older ones before, asked for at the cursor.
        o.loading_older = true;
        o.take_older("c1", vec![m("1"), m("0")], Some("c2".into()));
        assert_eq!(ids(&o), ["0", "1", "2", "3"]);
        assert_eq!(o.older.as_deref(), Some("c2"));
        // An answer for another cursor, or not asked for, is dropped.
        o.take_older("c2", vec![m("-1")], None);
        assert_eq!(ids(&o), ["0", "1", "2", "3"]);
        // Reading the latest again adds what is new, and keeps the cursor.
        o.take_latest(vec![m("5"), m("4"), m("3")], Some("other".into()));
        assert_eq!(ids(&o), ["0", "1", "2", "3", "4", "5"]);
        assert_eq!(o.older.as_deref(), Some("c2"));
        // A sent message shows once, whichever comes first.
        o.sent(m("6"));
        o.take_latest(vec![m("6"), m("5")], None);
        assert_eq!(ids(&o).last(), Some(&"6"));
        assert_eq!(o.messages.len(), 7);
    }
}
