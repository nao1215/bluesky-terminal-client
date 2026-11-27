//! The key bindings as the user is told about them: the hint row under the
//! current view, the sections of the `?` help, and what `.` lists. All read
//! from here, as in bsky, so a binding cannot be documented in one place and
//! forgotten in the other. The keys are bsky's where bsky has the action.

use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::{App, Overlay, Tab};

/// A key and what it does.
pub type Hint = (&'static str, &'static str);

/// A titled group of bindings in the help.
pub struct Section {
    pub title: &'static str,
    pub keys: &'static [Hint],
}

/// Everything `?` shows, in the order it shows it.
pub const HELP: &[Section] = &[
    Section {
        title: "Global",
        keys: &[
            ("1 2 3 …", "the tabs, in the order shown"),
            ("tab shift+tab", "next / previous tab"),
            ("", "which tabs, and their order: the tabs setting"),
            ("A", "accounts: switch, log out"),
            ("?", "this help"),
            ("q ctrl+c", "quit"),
        ],
    },
    Section {
        title: "Lists",
        keys: &[
            ("j k ↓ ↑", "move the selection"),
            ("g G home end", "first / last item"),
            ("pgdn pgup", "move five items"),
            ("", "more loads by itself near the end"),
            ("R F5", "refresh the current view"),
        ],
    },
    Section {
        title: "Notes",
        keys: &[
            ("n", "new note"),
            ("r", "reply to the selected note"),
            ("Q", "quote the selected note in a new note"),
            ("l", "react ❤ / take back your reaction"),
            (
                "e",
                "choose a reaction: recent, default, or the server's emoji by name",
            ),
            ("b", "renote / take back your renote"),
            ("D", "delete your own note: y confirms"),
            ("enter", "open the profile of its author"),
            ("v", "open the thread with its replies"),
            ("space", "view the pictures and files"),
            ("w", "show / hide the text behind a content warning"),
            ("d", "save the note's files in the download folder"),
            ("o", "open the note in the browser"),
            ("c", "copy the note's address to the clipboard"),
            (".", "list what the keys do to the note"),
        ],
    },
    Section {
        title: "Thread",
        keys: &[
            ("j k", "move through the notes and replies"),
            ("v", "open the thread of the selected reply"),
            ("esc", "close the thread"),
        ],
    },
    Section {
        title: "Search",
        keys: &[
            ("type", "the search box has focus when you arrive"),
            ("enter", "search, then move through the results"),
            ("/ i", "type in the search box again"),
            ("ctrl+t t", "search notes or accounts"),
            ("enter", "open the account's profile"),
            ("f", "follow / unfollow the account"),
            ("esc", "leave the search box"),
        ],
    },
    Section {
        title: "Profile",
        keys: &[
            ("", "its tab is your own profile"),
            ("f", "follow / unfollow the account shown"),
            ("M", "mute / unmute the account shown"),
            ("B", "block / unblock the account: y confirms"),
            (
                "",
                "an account that approves followers, or of another server, is asked",
            ),
            ("esc", "back to the list it was opened from"),
        ],
    },
    Section {
        title: "Notifications",
        keys: &[
            ("v space", "the thread or the files of the note it is about"),
            ("r l b", "reply, react, renote the note it is about"),
            ("", "they are marked read when the tab is shown"),
            ("R", "load new notifications"),
        ],
    },
    Section {
        title: "Composer",
        keys: &[
            ("ctrl+s", "send the note"),
            ("ctrl+o", "attach pictures or a video"),
            ("tab", "next field: the warning, a file's alt text"),
            ("ctrl+w", "add / take off a content warning"),
            ("ctrl+p", "who may see it: public, home, followers"),
            ("ctrl+x", "remove the attachment"),
            ("ctrl+u", "clear to the start of the line"),
            ("esc", "close without sending"),
        ],
    },
    Section {
        title: "File browser",
        keys: &[
            ("j k", "move; pictures are previewed"),
            ("enter l", "open the folder / choose the file"),
            ("space", "mark files to choose together with enter"),
            ("h backspace", "the folder above"),
            (".", "show or hide hidden files"),
            ("~", "your home folder"),
            ("esc", "close without choosing"),
        ],
    },
    Section {
        title: "Viewer",
        keys: &[
            ("← → h l", "previous / next file"),
            ("r", "play the video again"),
            ("d", "save it in the download folder"),
            ("o", "open it in the browser"),
            ("esc q", "back to where you were"),
        ],
    },
    Section {
        title: "Chat",
        keys: &[
            ("enter", "open the conversation"),
            ("i enter", "write a message; enter sends it"),
            ("k j", "read further back / forward"),
            ("esc", "back to the conversations"),
            ("", "read again every 15 s while shown"),
        ],
    },
    Section {
        title: "Accounts",
        keys: &[
            ("A", "the accounts, the one in use marked"),
            ("j k", "move"),
            ("enter", "use the selected account"),
            ("x", "log the selected account out: y confirms"),
            ("a", "log in another account"),
            ("esc", "close"),
        ],
    },
    Section {
        title: "Log in",
        keys: &[
            ("j k", "choose a server from the list"),
            ("/ i", "type another server's host"),
            ("enter", "log in there: allow msky in the browser"),
            ("o", "open the page that allows it again"),
            ("esc", "back, or call the login off"),
        ],
    },
    Section {
        title: "Help",
        keys: &[("j k pgdn pgup", "scroll"), ("esc q ?", "close")],
    },
];

/// The help as it applies to this terminal. Without pictures there is no
/// viewer, and `space` opens a note in the web browser instead.
pub fn help(pictures: bool) -> Vec<(&'static str, Vec<Hint>)> {
    HELP.iter()
        .filter(|s| pictures || s.title != "Viewer")
        .map(|s| {
            let keys = s
                .keys
                .iter()
                .map(|&(k, d)| match (pictures, s.title, k) {
                    (false, "Notes", "space") => (k, "open the note in the browser"),
                    _ => (k, d),
                })
                .collect();
            (s.title, keys)
        })
        .collect()
}

/// The bindings worth showing under the current view, most useful first.
pub fn hints(app: &App) -> Vec<Hint> {
    let mut v = view_hints(app);
    if !app.pictures {
        for h in &mut v {
            if *h == ("space", "view") {
                h.1 = "open in browser";
            }
        }
    }
    v
}

fn view_hints(app: &App) -> Vec<Hint> {
    match &app.overlay {
        Some(Overlay::Help { .. }) => return vec![("j k", "scroll"), ("esc", "close")],
        Some(Overlay::Compose(c)) if c.browser.is_some() => {
            return vec![
                ("enter", "open / choose"),
                ("space", "mark"),
                ("h", "up"),
                (".", "hidden"),
                ("esc", "cancel"),
            ];
        }
        Some(Overlay::Compose(c)) => {
            let mut v = vec![
                ("ctrl+s", "send"),
                ("ctrl+o", "attach"),
                ("ctrl+w", "warning"),
                ("ctrl+p", "visibility"),
            ];
            if !c.media.is_empty() {
                v.push(("tab", "alt text"));
                v.push(("ctrl+x", "remove file"));
            }
            v.push(("esc", "cancel"));
            return v;
        }
        Some(Overlay::Reactions { .. }) => {
            return vec![
                ("type", "search emoji"),
                ("↑ ↓", "choose"),
                ("enter", "react"),
                ("esc", "close"),
            ];
        }
        Some(Overlay::Login(f)) if f.waiting.is_some() => {
            return vec![("o", "open the page again"), ("esc", "call it off")];
        }
        Some(Overlay::Login(f)) if f.typed.is_some() => {
            return vec![("enter", "log in"), ("esc", "back to the list")];
        }
        Some(Overlay::Login(_)) => {
            return vec![
                ("j k", "move"),
                ("enter", "log in"),
                ("/", "other server"),
                ("esc", "back"),
            ];
        }
        Some(Overlay::Actions { .. }) => {
            return vec![("j k", "move"), ("enter", "do it"), ("esc", "close")];
        }
        Some(Overlay::Accounts { .. }) => {
            return vec![
                ("j k", "move"),
                ("enter", "use"),
                ("a", "add"),
                ("x", "log out"),
                ("esc", "close"),
            ];
        }
        Some(Overlay::Viewer { files, index, .. }) => {
            let mut v = Vec::new();
            if files.len() > 1 {
                v.push(("← →", "previous / next"));
            }
            if files
                .get(*index)
                .is_some_and(|f| f.2.mime.starts_with("video/"))
            {
                v.push(("r", "replay"));
            }
            v.push(("d", "download"));
            v.push(("o", "open"));
            v.push(("esc", "back"));
            return v;
        }
        None => {}
    }
    if !app.threads.is_empty() {
        return vec![
            ("?", "help"),
            ("j k", "move"),
            (".", "actions"),
            ("space", "view"),
            ("esc", "back"),
        ];
    }
    let mut v: Vec<Hint> = match app.tab {
        Tab::Chat => match &app.chat.open {
            Some(o) if o.typing => return vec![("enter", "send"), ("esc", "stop writing")],
            Some(_) => vec![("i", "write"), ("j k", "scroll"), ("esc", "back")],
            None => vec![("j k", "move"), ("enter", "open"), ("R", "refresh")],
        },
        Tab::Search if app.search.editing => {
            return vec![
                ("enter", "search"),
                ("ctrl+t", "notes / accounts"),
                ("tab", "next tab"),
                ("esc", "done"),
            ];
        }
        Tab::Search if app.search.mode == crate::tui::app::SearchMode::Accounts => vec![
            ("j k", "move"),
            ("enter", "profile"),
            ("f", "follow"),
            ("/", "edit query"),
            ("t", "notes"),
        ],
        Tab::Search => vec![
            ("j k", "move"),
            (".", "actions"),
            ("/", "edit query"),
            ("t", "accounts"),
        ],
        Tab::Profile if app.profile.came_from.is_some() => vec![
            ("esc", "back"),
            ("j k", "move"),
            (".", "actions"),
            ("f", "follow"),
        ],
        Tab::Profile => vec![("j k", "move"), (".", "actions"), ("R", "reload")],
        Tab::Timeline(_) => vec![
            ("j k", "move"),
            ("tab", "next tab"),
            (".", "actions"),
            ("space", "view"),
            ("n", "note"),
            ("R", "refresh"),
        ],
        Tab::Notifications => vec![
            ("j k", "move"),
            (".", "actions"),
            ("space", "view"),
            ("R", "refresh"),
        ],
    };
    // Help comes first: a narrow terminal cuts the row from the right, and
    // `?` is the key that leads to every other one.
    v.insert(0, ("?", "help"));
    v.push(("q", "quit"));
    v
}

/// What `.` offers on the selected note: the keys of this view that act on
/// it, each saying what it would do now.
pub fn actions(app: &App) -> Vec<Hint> {
    if app.overlay.is_some() && !matches!(app.overlay, Some(Overlay::Actions { .. })) {
        return Vec::new();
    }
    let Some(n) = app.shown_note() else {
        return Vec::new();
    };
    let mut v: Vec<Hint> = Vec::new();
    let files = !n.attached().is_empty();
    v.push(("r", "reply to it"));
    v.push((
        "l",
        if n.my_reaction.is_some() {
            "take back your reaction"
        } else {
            "react ❤"
        },
    ));
    v.push(("e", "choose a reaction"));
    v.push((
        "b",
        if app.renoted.contains(&n.id)
            || app.selected_note().is_some_and(|s| {
                s.is_pure_renote()
                    && app
                        .session
                        .as_ref()
                        .is_some_and(|me| me.user_id == s.author_id())
            })
        {
            "take back your renote"
        } else {
            "renote it"
        },
    ));
    v.push(("Q", "quote it in a new note"));
    if app.pictures && files {
        v.push(("space", "view its pictures and files"));
    }
    if n.cw.is_some() {
        v.push((
            "w",
            if app.text_shown(n) {
                "hide the text behind its warning"
            } else {
                "show the text behind its warning"
            },
        ));
    }
    v.push(("v", "open the thread"));
    if files {
        v.push(("d", "save its files"));
    }
    v.push(("o", "open it in the web browser"));
    v.push(("c", "copy its address"));
    let mine = app.selected_note().is_some_and(|s| {
        !s.is_pure_renote()
            && app
                .session
                .as_ref()
                .is_some_and(|me| me.user_id == s.author_id())
    });
    if mine {
        v.push(("D", "delete your note"));
    }
    v
}

/// The key an entry of the actions list stands for.
pub fn action_key(name: &str) -> KeyEvent {
    let code = match name {
        "space" => KeyCode::Char(' '),
        "enter" => KeyCode::Enter,
        _ => KeyCode::Char(name.chars().next().unwrap_or('?')),
    };
    KeyEvent::from(code)
}
