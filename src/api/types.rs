//! The parts of Misskey's objects msky reads. Every field has a default: a
//! server of another version, or a fork, leaves some out, and a note that
//! lacks one is still worth showing.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

/// `User` (the lite form notes carry; `UserDetailed` from `users/show` has
/// these and more).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct User {
    pub id: String,
    pub username: String,
    /// The server the account lives on; `None` for one of the server asked.
    pub host: Option<String>,
    /// The display name, which may be unset.
    pub name: Option<String>,
}

/// `DriveFile`: a file attached to a note.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DriveFile {
    pub id: String,
    /// The name it was uploaded with.
    pub name: String,
    /// Its MIME type, as the server detected it.
    #[serde(rename = "type")]
    pub mime: String,
    /// Its size in bytes.
    pub size: u64,
    pub url: Option<String>,
    pub is_sensitive: bool,
}

/// `Note`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Note {
    pub id: String,
    pub created_at: String,
    /// The ID of the account that wrote it, as `user.id`.
    pub user_id: String,
    pub user: User,
    pub text: Option<String>,
    /// The content warning shown in place of the text until it is opened.
    pub cw: Option<String>,
    pub renote_count: u64,
    pub replies_count: u64,
    /// Each reaction (a Unicode emoji, or `:name@.:` for a custom one of
    /// the server, `:name@host:` for one of another) and how many gave it.
    pub reactions: BTreeMap<String, u64>,
    pub files: Vec<DriveFile>,
    pub reply_id: Option<String>,
    /// The note this one replies to, when the server sent it along.
    pub reply: Option<Box<Note>>,
    pub renote_id: Option<String>,
    /// The note this one renotes or quotes.
    pub renote: Option<Box<Note>>,
    pub poll: Option<Value>,
    /// Where a note from another server was first posted.
    pub uri: Option<String>,
}

impl Note {
    /// Whether this is a renote and nothing else: no text, no warning, no
    /// files, no poll, as Misskey decides it. Such a note shows the one it
    /// renotes; a renote with any of them is a quote.
    pub fn is_pure_renote(&self) -> bool {
        self.renote_id.is_some()
            && self.text.is_none()
            && self.cw.is_none()
            && self.files.is_empty()
            && self.poll.is_none()
    }

    /// The note to show and to take files from: the renoted one for a pure
    /// renote (when the server sent it along), else this one.
    pub fn shown(&self) -> &Note {
        match &self.renote {
            Some(r) if self.is_pure_renote() => r,
            _ => self,
        }
    }

    /// The files it carries: its own, then those of the note it renotes or
    /// quotes, each with the note it is attached to and its place among
    /// that note's files, from 1.
    pub fn attached(&self) -> Vec<(&Note, usize, &DriveFile)> {
        let mut all: Vec<(&Note, usize, &DriveFile)> = self
            .files
            .iter()
            .enumerate()
            .map(|(i, f)| (self, i + 1, f))
            .collect();
        if let Some(r) = &self.renote {
            all.extend(r.files.iter().enumerate().map(|(i, f)| (&**r, i + 1, f)));
        }
        all
    }

    /// The ID of the account that wrote it.
    pub fn author_id(&self) -> &str {
        if self.user_id.is_empty() {
            &self.user.id
        } else {
            &self.user_id
        }
    }

    /// How many reactions it has, of every kind.
    pub fn reaction_total(&self) -> u64 {
        self.reactions.values().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // A note from a server that leaves fields out, or writes null for
    // them, is still read.
    #[test]
    fn a_note_with_missing_and_null_fields_is_read() {
        let n: Note = serde_json::from_value(json!({
            "id": "a1", "text": null, "files": [], "user": {"id": "u", "username": "alice", "host": null}
        }))
        .unwrap();
        assert_eq!(n.id, "a1");
        assert_eq!(n.text, None);
        assert_eq!(n.reaction_total(), 0);
        assert!(!n.is_pure_renote());
    }

    #[test]
    fn a_renote_without_text_shows_the_note_it_renotes_and_a_quote_shows_itself() {
        let inner = json!({"id": "in", "text": "元のノート 🐈", "user": {"username": "bob"}});
        let pure: Note =
            serde_json::from_value(json!({"id": "rn", "renoteId": "in", "renote": inner})).unwrap();
        assert!(pure.is_pure_renote());
        assert_eq!(pure.shown().id, "in");
        let quote: Note = serde_json::from_value(
            json!({"id": "q", "text": "これ", "renoteId": "in", "renote": inner}),
        )
        .unwrap();
        assert!(!quote.is_pure_renote());
        assert_eq!(quote.shown().id, "q");
        let warned: Note =
            serde_json::from_value(json!({"id": "w", "cw": "", "renoteId": "in", "renote": inner}))
                .unwrap();
        assert!(!warned.is_pure_renote());
    }

    // A quote's files come after its own, each named after the note it is
    // attached to.
    #[test]
    fn the_files_of_a_note_include_those_of_the_note_it_quotes() {
        let n: Note = serde_json::from_value(json!({
            "id": "q", "text": "見て", "files": [{"id": "a"}],
            "renoteId": "o", "renote": {"id": "o", "files": [{"id": "b"}, {"id": "c"}]}
        }))
        .unwrap();
        let got: Vec<(&str, usize, &str)> = n
            .attached()
            .into_iter()
            .map(|(owner, i, f)| (owner.id.as_str(), i, f.id.as_str()))
            .collect();
        assert_eq!(got, [("q", 1, "a"), ("o", 1, "b"), ("o", 2, "c")]);
    }

    #[test]
    fn the_reaction_total_adds_every_kind() {
        let n: Note = serde_json::from_value(json!({
            "id": "a", "reactions": {"👍": 3, ":blobcat@.:": 2, ":x@other.example:": 1}
        }))
        .unwrap();
        assert_eq!(n.reaction_total(), 6);
    }
}
