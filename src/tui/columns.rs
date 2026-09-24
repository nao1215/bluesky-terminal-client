//! Columns on the Timeline tab: lists the client already has, side by
//! side. Each column is one source (the following timeline, a pinned feed,
//! the notifications, a search, an account's posts) with a list of its own,
//! loaded and paged as the same list is on its own tab. Which columns there
//! are is kept in `settings.json`, per account.

use crate::api::types::Post;
use crate::tui::app::List;
use crate::tui::worker::{Feed, NotifItem};

pub use crate::config::ColumnSource as Source;

/// The list the worker loads for a column of `source`.
pub fn feed_of(source: &Source) -> Feed {
    match source {
        Source::Following => Feed::Timeline,
        Source::Feed { uri, .. } => Feed::Custom(uri.clone()),
        Source::Notifications => Feed::Notifications,
        Source::Search { query } => Feed::SearchPosts(query.clone()),
        Source::Author { did, .. } => Feed::Author(did.clone()),
    }
}

/// A column's list: posts, or notifications.
#[derive(Debug, Clone)]
pub enum Rows {
    Posts(List<Post>),
    Notifications(List<NotifItem>),
}

/// One column.
#[derive(Debug, Clone)]
pub struct Column {
    /// Stays the same while the column exists; answers name it.
    pub id: u64,
    pub source: Source,
    pub rows: Rows,
    /// Counts the first-page loads asked for: an answer to an earlier one
    /// (the column was refreshed since) is dropped.
    pub generation: u64,
}

impl Column {
    fn new(id: u64, source: Source) -> Self {
        let rows = match source {
            Source::Notifications => Rows::Notifications(List::default()),
            _ => Rows::Posts(List::default()),
        };
        Self {
            id,
            source,
            rows,
            generation: 0,
        }
    }

    /// Whether its first page has been asked for.
    pub fn asked(&self) -> bool {
        match &self.rows {
            Rows::Posts(l) => l.loaded || l.loading,
            Rows::Notifications(l) => l.loaded || l.loading,
        }
    }
}

/// Every column of the account in use, and the one keys act on.
#[derive(Debug, Clone, Default)]
pub struct Columns {
    pub items: Vec<Column>,
    pub focus: usize,
    next_id: u64,
}

impl Columns {
    /// Columns for `sources`, none of them loaded yet.
    pub fn from_sources(sources: &[Source]) -> Self {
        let mut c = Self::default();
        for s in sources {
            c.add(s.clone());
        }
        c.focus = 0;
        c
    }

    /// The sources, in order, as `settings.json` keeps them.
    pub fn sources(&self) -> Vec<Source> {
        self.items.iter().map(|c| c.source.clone()).collect()
    }

    /// Add a column after the others and focus it; returns its id.
    pub fn add(&mut self, source: Source) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.items.push(Column::new(id, source));
        self.focus = self.items.len() - 1;
        id
    }

    pub fn focused(&self) -> Option<&Column> {
        self.items.get(self.focus)
    }

    pub fn focused_mut(&mut self) -> Option<&mut Column> {
        self.items.get_mut(self.focus)
    }

    pub fn by_id(&mut self, id: u64) -> Option<&mut Column> {
        self.items.iter_mut().find(|c| c.id == id)
    }

    /// Move the focus `delta` columns, stopping at the ends.
    pub fn move_focus(&mut self, delta: isize) {
        if self.items.is_empty() {
            return;
        }
        let last = self.items.len() as isize - 1;
        self.focus = (self.focus as isize + delta).clamp(0, last) as usize;
    }

    /// Move the focused column `delta` places, keeping the focus on it.
    pub fn move_focused(&mut self, delta: isize) {
        if self.items.is_empty() {
            return;
        }
        let last = self.items.len() as isize - 1;
        let to = (self.focus as isize + delta).clamp(0, last) as usize;
        let c = self.items.remove(self.focus);
        self.items.insert(to, c);
        self.focus = to;
    }

    /// Remove the focused column.
    pub fn remove_focused(&mut self) -> Option<Column> {
        if self.items.is_empty() {
            return None;
        }
        let c = self.items.remove(self.focus);
        self.focus = self.focus.min(self.items.len().saturating_sub(1));
        Some(c)
    }

    /// Every list of posts in the columns, for a write to show on.
    pub fn post_lists(&mut self) -> impl Iterator<Item = &mut List<Post>> {
        self.items.iter_mut().filter_map(|c| match &mut c.rows {
            Rows::Posts(l) => Some(l),
            Rows::Notifications(_) => None,
        })
    }

    /// The lists of posts of the columns showing `source`.
    pub fn posts_of<'a>(
        &'a mut self,
        source: &'a Source,
    ) -> impl Iterator<Item = &'a mut List<Post>> + 'a {
        self.items
            .iter_mut()
            .filter(move |c| c.source == *source)
            .filter_map(|c| match &mut c.rows {
                Rows::Posts(l) => Some(l),
                Rows::Notifications(_) => None,
            })
    }

    /// Every list of notifications in the columns.
    pub fn notification_lists(&mut self) -> impl Iterator<Item = &mut List<NotifItem>> {
        self.items.iter_mut().filter_map(|c| match &mut c.rows {
            Rows::Notifications(l) => Some(l),
            Rows::Posts(_) => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(c: &Columns) -> Vec<u64> {
        c.items.iter().map(|c| c.id).collect()
    }

    #[test]
    fn columns_are_added_moved_and_removed_with_the_focus_following() {
        let mut c = Columns::default();
        c.add(Source::Following);
        c.add(Source::Notifications);
        c.add(Source::Search {
            query: "猫🐈‍⬛".into(),
        });
        assert_eq!(ids(&c), [1, 2, 3]);
        assert_eq!(c.focus, 2);
        c.move_focused(-1);
        assert_eq!(ids(&c), [1, 3, 2]);
        assert_eq!(c.focus, 1);
        c.move_focus(-5);
        assert_eq!(c.focus, 0);
        c.move_focused(-1);
        assert_eq!(ids(&c), [1, 3, 2]);
        c.move_focus(9);
        assert_eq!(c.remove_focused().unwrap().id, 2);
        assert_eq!(c.focus, 1);
        c.remove_focused();
        c.remove_focused();
        assert!(c.remove_focused().is_none());
        assert_eq!(c.focus, 0);
    }

    #[test]
    fn sources_round_trip_through_json_and_name_their_column() {
        let sources = vec![
            Source::Following,
            Source::Feed {
                uri: "at://did:plc:g/app.bsky.feed.generator/cats".into(),
                name: "Cats 🐈".into(),
            },
            Source::Notifications,
            Source::Search {
                query: "家族👨\u{200d}👩\u{200d}👧".into(),
            },
            Source::Author {
                did: "did:plc:a".into(),
                handle: "alice.test".into(),
            },
        ];
        let json = serde_json::to_value(&sources).unwrap();
        assert_eq!(json[0], serde_json::json!({"kind": "following"}));
        assert_eq!(json[3]["kind"], "search");
        let back: Vec<Source> = serde_json::from_value(json).unwrap();
        assert_eq!(back, sources);
        let titles: Vec<String> = sources.iter().map(Source::title).collect();
        assert_eq!(
            titles,
            [
                "Following",
                "Cats 🐈",
                "Notifications",
                "Search: 家族👨\u{200d}👩\u{200d}👧",
                "@alice.test"
            ]
        );
        let c = Columns::from_sources(&sources);
        assert_eq!(c.focus, 0);
        assert!(matches!(c.items[2].rows, Rows::Notifications(_)));
        assert_eq!(c.sources(), sources);
    }
}
