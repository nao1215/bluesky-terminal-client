//! The key bindings as the user is told about them: the hint row under the
//! current view, the sections of the `?` help, and what `.` lists. All read
//! from here, as in bsky and msky, so a binding cannot be documented in one
//! place and forgotten in the other. The keys are theirs where they have
//! the action.

use crossterm::event::{KeyCode, KeyEvent};

use crate::api::types::Status;
use crate::i18n::{n, t};
use crate::tui::app::{App, Overlay, Tab};

/// A key and what it does.
pub(crate) type Hint = (&'static str, &'static str);

/// A titled group of bindings in the help.
pub(crate) struct Section {
    pub title: &'static str,
    pub keys: &'static [Hint],
}

/// Everything `?` shows, in the order it shows it.
pub(crate) const HELP: &[Section] = &[
    Section {
        title: n!("General"),
        keys: &[
            ("1 2 3 4", n!("the tabs, in the order shown")),
            ("tab shift+tab", n!("next / previous tab")),
            ("T", n!("theme: each previewed as you move, enter keeps it")),
            ("?", n!("this help")),
            ("q ctrl+c", n!("quit")),
        ],
    },
    Section {
        title: n!("Lists"),
        keys: &[
            ("j k ↓ ↑", n!("move the selection")),
            ("g G home end", n!("first / last item")),
            ("pgdn pgup", n!("move five items")),
            ("", n!("more loads by itself near the end")),
            ("R F5", n!("refresh the current view")),
        ],
    },
    Section {
        title: n!("Posts"),
        keys: &[
            ("n", n!("new post")),
            ("r", n!("reply to the selected post")),
            ("l", n!("favourite / take back your favourite")),
            ("b", n!("boost / take back your boost")),
            ("v enter", n!("open the thread with its replies")),
            ("w", n!("show / hide the text behind a content warning")),
            ("o", n!("open the post in the browser")),
            ("c", n!("copy the post's address to the clipboard")),
            (".", n!("list what the keys do to the post")),
        ],
    },
    Section {
        title: n!("Thread"),
        keys: &[
            ("j k", n!("move through the posts and replies")),
            ("v", n!("open the thread of the selected reply")),
            ("esc", n!("close the thread")),
        ],
    },
    Section {
        title: n!("Notifications"),
        keys: &[
            ("v enter", n!("the thread of the post it is about")),
            ("r l b", n!("reply, favourite, boost the post it is about")),
            ("R", n!("load new notifications")),
        ],
    },
    Section {
        title: n!("Composer"),
        keys: &[
            ("ctrl+s", n!("send the post")),
            ("ctrl+w", n!("add / take off a content warning")),
            ("tab", n!("between the text and the warning")),
            (
                "ctrl+p",
                n!("who may see it: public, quiet public, followers, mentioned"),
            ),
            ("ctrl+u", n!("clear to the start of the line")),
            ("esc", n!("close without sending")),
        ],
    },
    Section {
        title: n!("Help"),
        keys: &[("j k pgdn pgup", n!("scroll")), ("esc q ?", n!("close"))],
    },
];

/// The help in the language in use.
pub(crate) fn help() -> Vec<(&'static str, Vec<Hint>)> {
    HELP.iter()
        .map(|s| (t(s.title), s.keys.iter().map(|&(k, d)| (k, t(d))).collect()))
        .collect()
}

/// The bindings worth showing under the current view, most useful first.
pub(crate) fn hints(app: &App) -> Vec<Hint> {
    translated(view_hints(app))
}

/// The hints with what each does in the language in use.
fn translated(mut v: Vec<Hint>) -> Vec<Hint> {
    for h in &mut v {
        h.1 = t(h.1);
    }
    v
}

fn view_hints(app: &App) -> Vec<Hint> {
    match &app.overlay {
        Some(Overlay::Help { .. }) => return vec![("j k", n!("scroll")), ("esc", n!("close"))],
        Some(Overlay::Themes { .. }) => {
            return vec![
                ("j k", n!("preview")),
                ("enter", n!("apply")),
                ("esc", n!("cancel")),
            ];
        }
        Some(Overlay::Actions { .. }) => {
            return vec![
                ("j k", n!("move")),
                ("enter", n!("do it")),
                ("esc", n!("close")),
            ];
        }
        Some(Overlay::Compose(c)) => {
            let mut v = vec![
                ("ctrl+s", n!("send")),
                ("ctrl+w", n!("warning")),
                ("ctrl+p", n!("visibility")),
            ];
            if c.warning.is_some() {
                v.push(("tab", n!("next field")));
            }
            v.push(("esc", n!("cancel")));
            return v;
        }
        None => {}
    }
    if !app.threads.is_empty() {
        return vec![
            ("?", n!("help")),
            ("j k", n!("move")),
            (".", n!("actions")),
            ("esc", n!("back")),
        ];
    }
    let mut v: Vec<Hint> = match app.tab {
        Tab::Notifications => vec![
            ("j k", n!("move")),
            ("tab", n!("next tab")),
            (".", n!("actions")),
            ("R", n!("refresh")),
        ],
        _ => vec![
            ("j k", n!("move")),
            ("tab", n!("next tab")),
            (".", n!("actions")),
            ("n", n!("post")),
            ("R", n!("refresh")),
        ],
    };
    // Help comes first: a narrow terminal cuts the row from the right, and
    // `?` is the key that leads to every other one.
    v.insert(0, ("?", n!("help")));
    v.push(("q", n!("quit")));
    v
}

/// What `.` offers on the selected status: the keys that act on it, each
/// saying what it would do now.
pub(crate) fn actions(app: &App) -> Vec<Hint> {
    let Some(s) = app.selected() else {
        return Vec::new();
    };
    let mut v = status_actions(app, s);
    if app.threads.is_empty() {
        v.push(("v", n!("open the thread")));
    } else {
        v.push(("v", n!("open the thread of this post")));
    }
    v.push(("o", n!("open it in the web browser")));
    v.push(("c", n!("copy its address")));
    translated(v)
}

fn status_actions(app: &App, s: &Status) -> Vec<Hint> {
    let mut v = vec![
        ("r", n!("reply to it")),
        (
            "l",
            if s.favourited == Some(true) {
                n!("take back your favourite")
            } else {
                n!("favourite it")
            },
        ),
    ];
    if !matches!(s.visibility.as_str(), "private" | "direct") {
        v.push((
            "b",
            if s.reblogged == Some(true) {
                n!("take back your boost")
            } else {
                n!("boost it")
            },
        ));
    }
    if !s.spoiler_text.is_empty() {
        v.push((
            "w",
            if app.text_shown(s) {
                n!("hide the text behind its warning")
            } else {
                n!("show the text behind its warning")
            },
        ));
    }
    v
}

/// The key an entry of the actions list stands for.
pub(crate) fn action_key(name: &str) -> KeyEvent {
    let code = match name {
        "enter" => KeyCode::Enter,
        _ => KeyCode::Char(name.chars().next().unwrap_or('?')),
    };
    KeyEvent::from(code)
}
