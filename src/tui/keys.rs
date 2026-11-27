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
            ("1 4", "the Timeline and Notifications tabs"),
            ("tab shift+tab", "next / previous tab"),
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
        title: "Timeline",
        keys: &[
            (
                "← → H L",
                "the previous / next timeline: home, local, social, global",
            ),
            ("+", "choose the timeline"),
        ],
    },
    Section {
        title: "Notes",
        keys: &[
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
        title: "Notifications",
        keys: &[
            ("v space", "the thread or the files of the note it is about"),
            ("R", "load new notifications"),
        ],
    },
    Section {
        title: "Viewer",
        keys: &[
            ("← → h l", "previous / next file"),
            ("d", "save it in the download folder"),
            ("o", "open it in the browser"),
            ("esc q", "back to where you were"),
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
        Some(Overlay::Timelines { .. }) => {
            return vec![("j k", "move"), ("enter", "show"), ("esc", "close")];
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
        Some(Overlay::Viewer { files, .. }) => {
            let mut v = Vec::new();
            if files.len() > 1 {
                v.push(("← →", "previous / next"));
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
        Tab::Timeline => vec![
            ("j k", "move"),
            ("← →", "timeline"),
            (".", "actions"),
            ("space", "view"),
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
