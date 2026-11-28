//! The lists the client shows, read a page at a time, and the rows of a
//! thread: what decides where the next page starts, which answer is the
//! one waited for, and how far in a reply is drawn. Nothing here knows the
//! keys or the screen.

use std::collections::{HashMap, HashSet};

use crate::api::types::{Notification, Status};
use crate::error::{Kind, Result};
use crate::tui::worker::Thread;

/// The next page is asked for when the selection is this near the end.
const NEAR_END: usize = 3;
/// The deepest a reply is drawn in from its thread's left edge.
pub(crate) const MAX_DEPTH: usize = 4;

/// Something that has an ID a list can tell it apart by.
pub(crate) trait Keyed {
    fn key(&self) -> &str;
}

impl Keyed for crate::api::types::Account {
    fn key(&self) -> &str {
        &self.id
    }
}

impl Keyed for Status {
    fn key(&self) -> &str {
        &self.id
    }
}

impl Keyed for Notification {
    fn key(&self) -> &str {
        &self.id
    }
}

/// A list read a page at a time, newest first.
#[derive(Debug, Clone)]
pub(crate) struct List<T> {
    pub items: Vec<T>,
    pub selected: usize,
    /// The first item drawn, which the view keeps.
    pub offset: usize,
    /// Whether the first page has come.
    pub loaded: bool,
    /// Where the next page starts; none at the end.
    pub next: Option<String>,
    /// Why the last page could not be read.
    pub error: Option<String>,
    /// The number of the page asked for and not come yet.
    pub(crate) waiting: Option<u64>,
    /// A down movement that reached beyond a page still being fetched.
    pub(crate) pending_selection: Option<usize>,
}

impl<T> Default for List<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selected: 0,
            offset: 0,
            loaded: false,
            next: None,
            error: None,
            waiting: None,
            pending_selection: None,
        }
    }
}

impl<T: Keyed> List<T> {
    pub(crate) fn current(&self) -> Option<&T> {
        self.items.get(self.selected)
    }

    pub(crate) fn select(&mut self, i: usize) {
        self.pending_selection = None;
        self.selected = i.min(self.items.len().saturating_sub(1));
    }

    pub(crate) fn step(&mut self, by: isize) {
        let target = self.selected.saturating_add_signed(by);
        self.select(target);
        if by > 0 && self.loaded && self.next.is_some() && target >= self.items.len() {
            self.pending_selection = Some(target);
        }
    }

    /// Whether the page after the last one should be asked for now.
    pub(crate) fn wants_more(&self) -> bool {
        self.loaded
            && self.waiting.is_none()
            && self.next.is_some()
            && self.selected + NEAR_END >= self.items.len()
    }

    /// Take the page asked for as `seq`: the first one in place of what is
    /// here, a later one after it. A page that brings nothing new ends the
    /// list. Returns whether it was the page waited for.
    pub(crate) fn take(
        &mut self,
        seq: u64,
        first: bool,
        page: Result<(Vec<T>, Option<String>)>,
    ) -> bool {
        if self.waiting != Some(seq) {
            return false;
        }
        self.waiting = None;
        let pending_selection = self.pending_selection.take();
        match page {
            Ok((items, next)) => {
                self.error = None;
                if first {
                    let kept = self.current().map(|c| c.key().to_string());
                    self.items = items;
                    self.selected = kept
                        .and_then(|k| self.items.iter().position(|i| i.key() == k))
                        .unwrap_or(0);
                    self.offset = self.offset.min(self.selected);
                    self.next = next;
                } else {
                    let seen: HashSet<String> =
                        self.items.iter().map(|i| i.key().to_string()).collect();
                    let before = self.items.len();
                    self.items
                        .extend(items.into_iter().filter(|i| !seen.contains(i.key())));
                    self.next = if self.items.len() == before {
                        None
                    } else {
                        next
                    };
                    if let Some(target) = pending_selection {
                        self.select(target);
                    }
                }
                self.loaded = true;
            }
            Err(e) => {
                // An endpoint that cannot supply another page must not be
                // asked again on every navigation key near the list's end.
                if e.kind() == Kind::Unsupported {
                    self.next = None;
                }
                self.error = Some(e.message().to_string());
                self.loaded = true;
            }
        }
        true
    }
}

/// A status in a thread, and how far in it is drawn.
#[derive(Debug, Clone)]
pub(crate) struct Row {
    pub status: Status,
    pub depth: usize,
    /// The status the thread was opened on.
    pub focus: bool,
}

impl Keyed for Row {
    fn key(&self) -> &str {
        &self.status.id
    }
}

/// A thread opened with `v`: what the status answers, it, and its replies.
#[derive(Debug, Clone)]
pub(crate) struct ThreadView {
    /// The status it was opened on.
    pub id: String,
    pub rows: List<Row>,
}

impl ThreadView {
    /// `th` as the rows it is drawn in: what it answers, the status, then
    /// each reply after the one it answers, a level further in.
    pub(crate) fn rows(th: Thread) -> Vec<Row> {
        let mut rows: Vec<Row> = th
            .ancestors
            .into_iter()
            .map(|status| Row {
                status,
                depth: 0,
                focus: false,
            })
            .collect();
        let focus = th.status.id.clone();
        rows.push(Row {
            status: th.status,
            depth: 0,
            focus: true,
        });
        let mut depth_of = HashMap::new();
        depth_of.insert(focus, 0usize);
        for status in th.descendants {
            let depth = status
                .in_reply_to_id
                .as_ref()
                .and_then(|p| depth_of.get(p))
                .map_or(1, |d| d + 1);
            depth_of.insert(status.id.clone(), depth);
            rows.push(Row {
                status,
                depth: depth.min(MAX_DEPTH),
                focus: false,
            });
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;

    fn status(id: &str, parent: Option<&str>) -> Status {
        Status {
            id: id.into(),
            in_reply_to_id: parent.map(Into::into),
            ..Status::default()
        }
    }

    fn ids(l: &List<Status>) -> Vec<&str> {
        l.items.iter().map(|s| s.id.as_str()).collect()
    }

    // A reply is a level further in than the one it answers, never more
    // than MAX_DEPTH; a reply to one not in the thread is the first level.
    #[test]
    fn a_reply_is_a_level_in_from_what_it_answers_up_to_the_deepest() {
        let mut descendants = vec![status("r0", Some("s"))];
        for i in 1..8 {
            descendants.push(status(&format!("r{i}"), Some(&format!("r{}", i - 1))));
        }
        descendants.push(status("lost", Some("gone")));
        let rows = ThreadView::rows(Thread {
            ancestors: vec![status("a", None)],
            status: status("s", Some("a")),
            descendants,
        });
        let depths: Vec<usize> = rows.iter().map(|r| r.depth).collect();
        assert_eq!(depths, [0, 0, 1, 2, 3, 4, 4, 4, 4, 4, 1]);
        let focus: Vec<bool> = rows.iter().map(|r| r.focus).collect();
        assert_eq!(focus.iter().filter(|f| **f).count(), 1);
        assert!(focus[1]);
    }

    // The first page keeps the selection on the status it was on; a later
    // one adds only what is new; an answer not waited for changes nothing.
    #[test]
    fn a_page_keeps_the_selection_and_adds_only_what_is_new() {
        let mut l = List::<Status> {
            waiting: Some(1),
            ..List::default()
        };
        assert!(l.take(
            1,
            true,
            Ok((vec![status("3", None), status("2", None)], Some("2".into())))
        ));
        l.selected = 1;
        l.waiting = Some(2);
        assert!(l.take(
            2,
            true,
            Ok((
                vec![status("4", None), status("3", None), status("2", None)],
                Some("2".into())
            ))
        ));
        assert_eq!(l.current().unwrap().id, "2");
        assert!(
            !l.take(2, false, Ok((vec![status("x", None)], None))),
            "not waited for"
        );
        assert_eq!(ids(&l), ["4", "3", "2"]);
        l.waiting = Some(3);
        assert!(l.take(
            3,
            false,
            Ok((vec![status("2", None), status("1", None)], Some("1".into())))
        ));
        assert_eq!(ids(&l), ["4", "3", "2", "1"]);
        assert_eq!(l.next.as_deref(), Some("1"));
        l.waiting = Some(4);
        l.take(4, false, Err(Error::api("busy")));
        assert_eq!(l.error.as_deref(), Some("busy"));
        assert_eq!(ids(&l), ["4", "3", "2", "1"]);
        assert_eq!(
            l.next.as_deref(),
            Some("1"),
            "the same page can be asked again"
        );
    }

    // The next page is wanted only near the end, once, and while there is one.
    #[test]
    fn the_next_page_is_wanted_near_the_end_only() {
        let mut l = List::<Status> {
            waiting: Some(1),
            ..List::default()
        };
        let items = (0..10).map(|i| status(&i.to_string(), None)).collect();
        l.take(1, true, Ok((items, Some("9".into()))));
        assert!(!l.wants_more());
        l.step(6);
        assert!(!l.wants_more());
        l.step(1);
        assert!(l.wants_more());
        l.waiting = Some(2);
        assert!(!l.wants_more());
        l.waiting = None;
        l.next = None;
        assert!(!l.wants_more());
        l.step(100);
        assert_eq!(l.selected, 9);
        l.step(-100);
        assert_eq!(l.selected, 0);
    }

    #[test]
    fn moving_past_the_loaded_page_is_applied_when_the_next_page_arrives() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../testdata/compat/pleroma-live.json")).unwrap();
        let page: Vec<Status> = serde_json::from_value(fixture["page"]["body"].clone()).unwrap();
        for cancel in [false, true] {
            let mut list = List {
                items: vec![page[0].clone()],
                loaded: true,
                next: Some(page[0].id.clone()),
                waiting: Some(2),
                ..List::default()
            };
            list.step(1);
            assert_eq!(list.selected, 0);
            if cancel {
                list.step(-1);
            }
            assert!(!list.take(1, false, Ok((vec![page[1].clone()], None))));
            assert!(list.take(2, false, Ok((vec![page[1].clone()], None))));
            assert_eq!(list.selected, usize::from(!cancel));
            assert!(!list.take(2, false, Ok((vec![page[1].clone()], None))));
        }
    }

    #[test]
    fn an_unsupported_page_stops_automatic_paging_but_keeps_the_rows() {
        let mut list = List {
            items: vec![status("1", None)],
            loaded: true,
            next: Some("1".into()),
            waiting: Some(7),
            ..List::default()
        };
        let error = Error::unsupported("public timeline", "127.0.0.1:8080");
        assert!(list.take(7, false, Err(error.clone())));
        assert_eq!(ids(&list), ["1"]);
        assert_eq!(list.error.as_deref(), Some(error.message()));
        assert!(list.waiting.is_none());
        assert!(!list.wants_more());
        assert!(!list.take(7, false, Err(error)));
        list.waiting = Some(8);
        assert!(list.take(8, true, Ok((vec![status("2", None)], None))));
        assert!(list.error.is_none());
        assert_eq!(ids(&list), ["2"]);
    }
}
