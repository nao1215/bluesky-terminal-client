//! Columns on the Home tab, as bsky's on its Timeline tab: lists side by
//! side. Each column is one source (the home timeline, an antenna, a list,
//! a channel, the notifications, a search, your own notes) with a list of
//! its own, loaded and paged on its own. Which columns there are is kept
//! in `settings.json`, per account.

use crate::api::types::{Note, Notification};
use crate::tui::app::List;

pub(crate) use crate::config::ColumnSource as Source;

/// Run `body` on a column's list, whichever kind it is: `|l| ...` names it.
macro_rules! each_list {
    ($rows:expr, |$l:ident| $body:expr) => {
        match $rows {
            $crate::tui::columns::Rows::Notes($l) => $body,
            $crate::tui::columns::Rows::Notifications($l) => $body,
        }
    };
}
pub(crate) use each_list;

/// A column's list: notes, or notifications.
#[derive(Debug, Clone)]
pub(crate) enum Rows {
    Notes(List<Note>),
    Notifications(List<Notification>),
}

/// One column.
#[derive(Debug, Clone)]
pub(crate) struct Column {
    /// Stays the same while the column exists; answers name it.
    pub id: u64,
    pub source: Source,
    pub rows: Rows,
}

impl Column {
    fn new(id: u64, source: Source) -> Self {
        let rows = match source {
            Source::Notifications => Rows::Notifications(List::default()),
            _ => Rows::Notes(List::default()),
        };
        Self { id, source, rows }
    }

    /// Whether its first page has been asked for.
    pub(crate) fn asked(&self) -> bool {
        each_list!(&self.rows, |l| l.loaded || l.loading)
    }
}

/// Every column of the account in use, and the one keys act on.
#[derive(Debug, Clone, Default)]
pub(crate) struct Columns {
    pub items: Vec<Column>,
    pub focus: usize,
    next_id: u64,
}

impl Columns {
    /// Columns for `sources`, none of them loaded yet.
    pub(crate) fn from_sources(sources: &[Source]) -> Self {
        let mut c = Self::default();
        for s in sources {
            c.add(s.clone());
        }
        c.focus = 0;
        c
    }

    /// The sources, in order, as `settings.json` keeps them.
    pub(crate) fn sources(&self) -> Vec<Source> {
        self.items.iter().map(|c| c.source.clone()).collect()
    }

    /// Add a column after the others and focus it; returns its id.
    pub(crate) fn add(&mut self, source: Source) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.items.push(Column::new(id, source));
        self.focus = self.items.len() - 1;
        id
    }

    pub(crate) fn focused(&self) -> Option<&Column> {
        self.items.get(self.focus)
    }

    pub(crate) fn focused_mut(&mut self) -> Option<&mut Column> {
        self.items.get_mut(self.focus)
    }

    pub(crate) fn by_id(&mut self, id: u64) -> Option<&mut Column> {
        self.items.iter_mut().find(|c| c.id == id)
    }

    /// Move the focus `delta` columns, stopping at the ends.
    pub(crate) fn move_focus(&mut self, delta: isize) {
        if self.items.is_empty() {
            return;
        }
        let last = self.items.len() as isize - 1;
        self.focus = (self.focus as isize + delta).clamp(0, last) as usize;
    }

    /// Move the focused column `delta` places, keeping the focus on it.
    pub(crate) fn move_focused(&mut self, delta: isize) {
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
    pub(crate) fn remove_focused(&mut self) -> Option<Column> {
        if self.items.is_empty() {
            return None;
        }
        let c = self.items.remove(self.focus);
        self.focus = self.focus.min(self.items.len().saturating_sub(1));
        Some(c)
    }

    /// Every list of notes in the columns, for a write to show on.
    pub(crate) fn note_lists(&self) -> impl Iterator<Item = &List<Note>> {
        self.items.iter().filter_map(|c| match &c.rows {
            Rows::Notes(l) => Some(l),
            Rows::Notifications(_) => None,
        })
    }

    pub(crate) fn note_lists_mut(&mut self) -> impl Iterator<Item = &mut List<Note>> {
        self.items.iter_mut().filter_map(|c| match &mut c.rows {
            Rows::Notes(l) => Some(l),
            Rows::Notifications(_) => None,
        })
    }

    /// Every list of notifications in the columns.
    pub(crate) fn notification_lists(&self) -> impl Iterator<Item = &List<Notification>> {
        self.items.iter().filter_map(|c| match &c.rows {
            Rows::Notifications(l) => Some(l),
            Rows::Notes(_) => None,
        })
    }

    pub(crate) fn notification_lists_mut(
        &mut self,
    ) -> impl Iterator<Item = &mut List<Notification>> {
        self.items.iter_mut().filter_map(|c| match &mut c.rows {
            Rows::Notifications(l) => Some(l),
            Rows::Notes(_) => None,
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
        c.add(Source::Home);
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
            Source::Home,
            Source::Antenna {
                id: "a1".into(),
                name: "ニュース 📰".into(),
            },
            Source::List {
                id: "l1".into(),
                name: "家族\n👨\u{200d}👩\u{200d}👧".into(),
            },
            Source::Channel {
                id: "c1".into(),
                name: "お絵描き 🎨".into(),
            },
            Source::Notifications,
            Source::Search {
                query: "🇯🇵 1️⃣ ❤️".into(),
            },
            Source::Author {
                id: "u-me".into(),
                username: "me".into(),
            },
        ];
        let json = serde_json::to_value(&sources).unwrap();
        assert_eq!(json[0], serde_json::json!({"kind": "home"}));
        assert_eq!(
            json[1],
            serde_json::json!({"kind": "antenna", "id": "a1", "name": "ニュース 📰"})
        );
        assert_eq!(json[5]["kind"], "search");
        let back: Vec<Source> = serde_json::from_value(json).unwrap();
        assert_eq!(back, sources);
        let titles: Vec<String> = sources.iter().map(Source::title).collect();
        assert_eq!(
            titles,
            [
                "Home",
                "📡 ニュース 📰",
                "☰ 家族 👨\u{200d}👩\u{200d}👧",
                "# お絵描き 🎨",
                "Notifications",
                "Search: 🇯🇵 1️⃣ ❤️",
                "@me"
            ]
        );
        let c = Columns::from_sources(&sources);
        assert_eq!(c.focus, 0);
        assert!(matches!(c.items[4].rows, Rows::Notifications(_)));
        assert_eq!(c.sources(), sources);
    }
}
