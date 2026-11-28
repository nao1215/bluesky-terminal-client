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
            ("1 2 3 4 5 6", n!("the tabs, in the order shown")),
            ("tab shift+tab", n!("next / previous tab")),
            ("/", n!("search")),
            ("A", n!("switch accounts")),
            ("s", n!("settings")),
            ("T", n!("theme: each previewed as you move, enter keeps it")),
            ("?", n!("this help")),
            ("q ctrl+c", n!("quit")),
        ],
    },
    Section {
        title: n!("Chat"),
        keys: &[
            ("6", n!("direct conversations")),
            ("m", n!("message the profile account")),
            ("enter", n!("open and mark read / send message")),
            ("i", n!("write a message")),
            ("esc", n!("stop writing / close conversation")),
        ],
    },
    Section {
        title: n!("Columns"),
        keys: &[
            ("+", n!("add a column on Home")),
            ("H L ← →", n!("previous / next column")),
            ("< >", n!("move the column")),
            ("x then y", n!("close the focused column")),
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
            ("enter", n!("open the author profile")),
            ("e", n!("edit your own profile")),
            ("space", n!("open pictures and videos")),
            ("d", n!("save the post files")),
            ("n", n!("new post")),
            ("Q", n!("quote the selected post (Mastodon 4.5+)")),
            ("r", n!("reply to the selected post")),
            ("l", n!("favourite / take back your favourite")),
            ("b", n!("boost / take back your boost")),
            ("f", n!("follow / unfollow the account")),
            ("M B", n!("mute / block the account; y confirms")),
            ("D", n!("delete your own post; y confirms")),
            ("!", n!("report the post or account; y confirms")),
            ("v", n!("open the thread with its replies")),
            (
                "",
                n!("threads read ahead after the selection rests for 400 ms"),
            ),
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
            ("v", n!("the thread of the post it is about")),
            ("r l b", n!("reply, favourite, boost the post it is about")),
            ("R", n!("load new notifications")),
            (
                "",
                n!("unread count and read position sync with the server"),
            ),
        ],
    },
    Section {
        title: n!("Composer"),
        keys: &[
            ("ctrl+s", n!("send the post")),
            (
                "ctrl+q",
                n!("quote approval: default, anyone, followers, only me"),
            ),
            ("ctrl+o", n!("attach pictures or a video")),
            ("ctrl+x", n!("remove the focused attachment, or the last")),
            ("ctrl+w", n!("add / take off a content warning")),
            ("tab", n!("between text, warning and attachment alt text")),
            (
                "ctrl+p",
                n!("who may see it: public, quiet public, followers, mentioned"),
            ),
            ("ctrl+u", n!("clear to the start of the line")),
            ("esc", n!("close without sending")),
        ],
    },
    Section {
        title: n!("Edit profile"),
        keys: &[
            ("tab shift+tab", n!("next / previous field")),
            ("space enter", n!("toggle the focused flag")),
            ("ctrl+o", n!("choose avatar or header")),
            ("ctrl+x", n!("remove chosen picture")),
            ("ctrl+s", n!("save changed fields")),
            ("esc", n!("close without saving")),
        ],
    },
    Section {
        title: n!("Report"),
        keys: &[
            ("tab", n!("optional comment")),
            ("enter ctrl+enter", n!("ask to send the report")),
            ("y", n!("confirm")),
            ("esc", n!("leave comment / close report")),
        ],
    },
    Section {
        title: n!("Search"),
        keys: &[
            ("/", n!("open search box")),
            ("enter", n!("search / open profile")),
            ("Ctrl+T", n!("posts/accounts")),
            ("Esc", n!("cancel / back to timeline")),
            ("enter v", n!("open profile")),
        ],
    },
    Section {
        title: n!("Viewer"),
        keys: &[
            ("h l ← →", n!("previous / next file")),
            ("r", n!("replay video")),
            ("d o", n!("save files / open post")),
            ("esc q", n!("close")),
        ],
    },
    Section {
        title: n!("Settings"),
        keys: &[
            ("j k", n!("move")),
            ("enter", n!("choose")),
            ("x", n!("reset to default")),
            ("esc", n!("close")),
        ],
    },
    Section {
        title: n!("Help"),
        keys: &[("j k pgdn pgup", n!("scroll")), ("esc q ?", n!("close"))],
    },
    Section {
        title: n!("Accounts"),
        keys: &[
            ("A", n!("switch accounts")),
            ("a", n!("add account")),
            ("x then y", n!("log out")),
            ("enter", n!("choose")),
            ("esc", n!("cancel")),
        ],
    },
    Section {
        title: n!("Log in"),
        keys: &[
            ("enter", n!("log in")),
            ("o", n!("open browser")),
            ("esc", n!("cancel")),
        ],
    },
    Section {
        title: n!("Link cards"),
        keys: &[("O", n!("open link card"))],
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
    if app.overlay.is_none() && app.search.editing {
        return terminal_kit::view::search_edit_hints(true, crate::i18n::current());
    }
    if app.overlay.is_none() && app.threads.is_empty() && app.search_results() {
        return terminal_kit::view::search_result_hints(
            app.search.result_accounts,
            crate::i18n::current(),
        );
    }

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
        Some(Overlay::Login { waiting: true, .. }) => {
            return vec![("o", n!("open browser")), ("esc", n!("cancel"))];
        }
        Some(Overlay::Login { waiting: false, .. }) => {
            return vec![("enter", n!("log in")), ("esc", n!("cancel"))];
        }
        Some(Overlay::Logout { .. }) => return vec![("y", n!("log out")), ("esc", n!("cancel"))],
        Some(Overlay::SettingText { .. }) => {
            return vec![("enter", n!("save")), ("esc", n!("cancel"))];
        }
        Some(Overlay::Accounts { .. }) => {
            return vec![
                ("j k", n!("move")),
                ("enter", n!("choose")),
                ("a", n!("add account")),
                ("x", n!("log out")),
                ("esc", n!("close")),
            ];
        }
        Some(Overlay::CloseColumn(_)) => {
            return vec![("y", n!("confirm")), (t("any other key"), n!("cancel"))];
        }
        Some(Overlay::AddColumn { .. }) => {
            return vec![
                ("j k", n!("move")),
                ("enter", n!("choose")),
                ("esc", n!("cancel")),
            ];
        }
        Some(Overlay::EditProfile(_)) => {
            return vec![
                ("ctrl+s", n!("save changed fields")),
                ("tab", n!("next field")),
                ("ctrl+o", n!("choose avatar or header")),
                ("space", n!("toggle the focused flag")),
                ("esc", n!("cancel")),
            ];
        }
        Some(Overlay::Confirm(_)) => {
            return vec![("y", n!("confirm")), (t("any other key"), n!("cancel"))];
        }
        Some(Overlay::Report(r)) => {
            return if r.confirming {
                vec![("y", n!("confirm")), (t("any other key"), n!("cancel"))]
            } else {
                vec![
                    ("tab", n!("optional comment")),
                    (
                        if r.writing { "ctrl+enter" } else { "enter" },
                        n!("confirm"),
                    ),
                    ("esc", n!("close")),
                ]
            };
        }
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
            if c.browser.is_some() {
                return vec![
                    ("enter", n!("choose")),
                    ("space", n!("mark")),
                    ("h", n!("parent folder")),
                    (".", n!("hidden files")),
                    ("esc", n!("cancel")),
                ];
            }
            let mut v = vec![
                ("ctrl+s", n!("send")),
                ("ctrl+o", n!("attach")),
                ("ctrl+x", n!("remove attachment")),
                ("ctrl+w", n!("warning")),
                ("ctrl+p", n!("visibility")),
            ];
            v.push(("ctrl+q", n!("quote approval")));
            if c.warning.is_some() || !c.media.is_empty() {
                v.push(("tab", n!("next field")));
            }
            v.push(("esc", n!("cancel")));
            return v;
        }
        Some(Overlay::Settings { .. }) => {
            return vec![
                ("j k", n!("move")),
                ("enter", n!("choose")),
                ("x", n!("reset to default")),
                ("esc", n!("close")),
            ];
        }
        Some(Overlay::Languages { .. }) => {
            return vec![
                ("j k", n!("move")),
                ("enter", n!("choose")),
                ("esc", n!("close")),
            ];
        }
        Some(Overlay::Viewer { .. }) => {
            return vec![
                ("h l", n!("previous / next file")),
                ("r", n!("replay video")),
                ("d", n!("save")),
                ("o", n!("open")),
                ("esc", n!("close")),
            ];
        }
        None => {}
    }
    if app.tab == Tab::Chat && app.threads.is_empty() {
        return vec![
            ("enter", n!("open and mark read / send message")),
            ("i", n!("write a message")),
            ("esc", n!("stop writing / close conversation")),
            ("?", n!("help")),
        ];
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
    v.insert(1, ("/", n!("search")));
    if app
        .selected()
        .is_some_and(|s| s.card.is_some() && app.text_shown(s))
    {
        v.insert(1, ("O", n!("open link card")));
    }
    if app.tab == Tab::Profile && app.profile.account.as_ref().is_some_and(|a| app.own(a)) {
        v.push(("e", n!("edit your own profile")));
    }
    if app.tab == Tab::Profile {
        v.push(("m", n!("message the profile account")));
    }
    v.push(("enter", n!("open profile")));
    v.push(("space", n!("open pictures and videos")));
    v.push(("f", n!("follow")));
    v.push(("M B", n!("mute / block")));
    v.push(("!", n!("report")));
    v.push(("D", n!("delete")));
    if app.tab == Tab::Home {
        v.push(("+", n!("add column")));
    }
    if app.in_deck() {
        v.push(("H L", n!("previous / next column")));
        v.push(("x", n!("close column")));
    }
    v.push(("Q", n!("quote")));
    v.push(("q", n!("quit")));
    v
}

/// What `.` offers on the selected status: the keys that act on it, each
/// saying what it would do now.
pub(crate) fn actions(app: &App) -> Vec<Hint> {
    let mut v = Vec::new();
    if let Some(a) = app.account_target()
        && !app.own(a)
    {
        let r = app.relationships.get(&a.id);
        v.push((
            "f",
            if r.is_some_and(|r| r.following || r.requested) {
                n!("unfollow / cancel follow request")
            } else {
                n!("follow")
            },
        ));
        v.push((
            "M",
            if r.is_some_and(|r| r.muting) {
                n!("unmute")
            } else {
                n!("mute")
            },
        ));
        v.push((
            "B",
            if r.is_some_and(|r| r.blocking) {
                n!("unblock")
            } else {
                n!("block")
            },
        ));
    }
    v.push(("!", n!("report")));
    let Some(s) = app.selected() else {
        return translated(v);
    };
    // Keep the established status action order for the actions menu.
    let mut status = status_actions(app, s);
    status.push(("Q", n!("quote")));
    status.append(&mut v);
    v = status;
    if app.own(&s.account) {
        v.push(("D", n!("delete")));
    }
    if app.threads.is_empty() {
        v.push(("v", n!("open the thread")));
    } else {
        v.push(("v", n!("open the thread of this post")));
    }
    v.push(("enter", n!("open the author profile")));
    if !s.media_attachments.is_empty() {
        v.push(("space", n!("open pictures and videos")));
        v.push(("d", n!("save the post files")));
    }
    if s.card.is_some() && app.text_shown(s) {
        v.push(("O", n!("open link card")));
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
        "space" => KeyCode::Char(' '),
        _ => KeyCode::Char(name.chars().next().unwrap_or('?')),
    };
    KeyEvent::from(code)
}
