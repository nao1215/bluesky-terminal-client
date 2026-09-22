//! A thread laid out as rows: the posts above the opened one, the opened
//! post, then every reply, depth-first and fully expanded.

use crate::api::types::{Post, ThreadNode};
use crate::tui::app::Keyed;

/// Replies deeper than this are drawn at this depth, so a long chain of
/// replies does not indent the text away.
pub const MAX_INDENT: u16 = 6;

/// What a row of the thread shows.
#[derive(Debug, Clone, PartialEq)]
pub enum RowKind {
    Post(Box<Post>),
    /// A post the AppView could not find (deleted, or never seen).
    NotFound(String),
    /// A post hidden by a block.
    Blocked(String),
}

/// One row of a thread.
#[derive(Debug, Clone, PartialEq)]
pub struct ThreadRow {
    /// 0 for the opened post and the posts above it, 1 for its replies, and
    /// so on.
    pub depth: u16,
    /// The post the thread was opened on.
    pub focused: bool,
    pub kind: RowKind,
}

impl ThreadRow {
    pub fn post(&self) -> Option<&Post> {
        match &self.kind {
            RowKind::Post(p) => Some(p),
            _ => None,
        }
    }

    pub fn post_mut(&mut self) -> Option<&mut Post> {
        match &mut self.kind {
            RowKind::Post(p) => Some(p),
            _ => None,
        }
    }
}

impl Keyed for ThreadRow {
    fn key(&self) -> &str {
        match &self.kind {
            RowKind::Post(p) => &p.uri,
            RowKind::NotFound(uri) | RowKind::Blocked(uri) => uri,
        }
    }
}

fn row(node: ThreadNode, depth: u16, focused: bool) -> Option<(ThreadRow, Vec<ThreadNode>)> {
    let (kind, replies) = match node {
        ThreadNode::Post { post, replies, .. } => (RowKind::Post(post), replies),
        ThreadNode::NotFound { uri } => (RowKind::NotFound(uri), Vec::new()),
        ThreadNode::Blocked { uri } => (RowKind::Blocked(uri), Vec::new()),
        ThreadNode::Other => return None,
    };
    Some((
        ThreadRow {
            depth,
            focused,
            kind,
        },
        replies,
    ))
}

/// Lay out a thread. Returns the rows and the index of the opened post.
pub fn flatten(node: ThreadNode) -> (Vec<ThreadRow>, usize) {
    // The parents come nearest first; they are shown from the top down.
    let (node, mut parents) = take_parents(node);
    parents.reverse();
    let mut rows: Vec<ThreadRow> = parents
        .into_iter()
        .filter_map(|p| row(p, 0, false).map(|(r, _)| r))
        .collect();
    let focus = rows.len();
    if let Some((r, replies)) = row(node, 0, true) {
        rows.push(r);
        push_replies(&mut rows, replies, 1);
    }
    let focus = focus.min(rows.len().saturating_sub(1));
    (rows, focus)
}

/// Split the parent chain off a node, nearest parent first.
fn take_parents(node: ThreadNode) -> (ThreadNode, Vec<ThreadNode>) {
    let mut chain = Vec::new();
    let (node, mut next) = match node {
        ThreadNode::Post {
            post,
            parent,
            replies,
        } => (
            ThreadNode::Post {
                post,
                parent: None,
                replies,
            },
            parent,
        ),
        other => (other, None),
    };
    while let Some(p) = next {
        match *p {
            ThreadNode::Post {
                post,
                parent,
                replies,
            } => {
                chain.push(ThreadNode::Post {
                    post,
                    parent: None,
                    replies,
                });
                next = parent;
            }
            other => {
                chain.push(other);
                next = None;
            }
        }
    }
    (node, chain)
}

fn push_replies(rows: &mut Vec<ThreadRow>, replies: Vec<ThreadNode>, depth: u16) {
    for reply in replies {
        if let Some((r, children)) = row(reply, depth, false) {
            rows.push(r);
            push_replies(rows, children, depth + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn post(uri: &str) -> Value {
        json!({"uri": uri, "cid": "c", "author": {"did": "d", "handle": "h.test"}, "record": {"text": uri}})
    }

    fn node(uri: &str, parent: Option<Value>, replies: Vec<Value>) -> Value {
        let mut n = json!({"$type": "app.bsky.feed.defs#threadViewPost", "post": post(uri), "replies": replies});
        if let Some(p) = parent {
            n["parent"] = p;
        }
        n
    }

    fn layout(v: Value) -> (Vec<(u16, bool, String)>, usize) {
        let thread: ThreadNode = serde_json::from_value(v).unwrap();
        let (rows, focus) = flatten(thread);
        let rows = rows
            .into_iter()
            .map(|r| (r.depth, r.focused, r.key().to_string()))
            .collect();
        (rows, focus)
    }

    #[test]
    fn parents_come_first_then_the_post_then_every_reply_expanded() {
        let v = node(
            "at://focus",
            Some(node(
                "at://parent",
                Some(node("at://root", None, vec![])),
                vec![],
            )),
            vec![
                node(
                    "at://r1",
                    None,
                    vec![node(
                        "at://r1a",
                        None,
                        vec![node("at://r1a1", None, vec![])],
                    )],
                ),
                node("at://r2", None, vec![]),
            ],
        );
        let (rows, focus) = layout(v);
        assert_eq!(
            rows,
            [
                (0, false, "at://root".into()),
                (0, false, "at://parent".into()),
                (0, true, "at://focus".into()),
                (1, false, "at://r1".into()),
                (2, false, "at://r1a".into()),
                (3, false, "at://r1a1".into()),
                (1, false, "at://r2".into()),
            ]
        );
        assert_eq!(focus, 2);
    }

    #[test]
    fn missing_posts_become_placeholders_and_unknown_ones_are_skipped() {
        let v = node(
            "at://focus",
            Some(
                json!({"$type": "app.bsky.feed.defs#notFoundPost", "uri": "at://gone", "notFound": true}),
            ),
            vec![
                json!({"$type": "app.bsky.feed.defs#blockedPost", "uri": "at://blocked", "blocked": true, "author": {}}),
                json!({"$type": "app.bsky.feed.defs#somethingNew"}),
                json!({"$type": "app.bsky.feed.defs#threadViewPost", "post": 42}),
                node("at://ok", None, vec![]),
            ],
        );
        let thread: ThreadNode = serde_json::from_value(v).unwrap();
        let (rows, focus) = flatten(thread);
        assert!(matches!(&rows[0].kind, RowKind::NotFound(u) if u == "at://gone"));
        assert_eq!(focus, 1);
        assert!(matches!(&rows[2].kind, RowKind::Blocked(u) if u == "at://blocked"));
        assert_eq!(rows.last().unwrap().key(), "at://ok");
        assert_eq!(
            rows.len(),
            4,
            "the unknown and the unreadable reply are skipped"
        );
    }

    #[test]
    fn a_thread_without_replies_key_or_parent_is_just_the_post() {
        let v = json!({"$type": "app.bsky.feed.defs#threadViewPost", "post": post("at://only")});
        let (rows, focus) = layout(v);
        assert_eq!(rows, [(0, true, "at://only".to_string())]);
        assert_eq!(focus, 0);
    }
}
