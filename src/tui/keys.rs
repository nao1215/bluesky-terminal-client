//! The key bindings as the user is told about them: the hint row under the
//! current view, and the sections of the `?` help. Both read from here so a
//! binding cannot be documented in one place and forgotten in the other.

use crate::tui::app::{App, Overlay, SearchMode, Tab};

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
            ("1 2 3", "Timeline, Search, Profile"),
            ("tab shift+tab", "next / previous tab"),
            ("T", "choose a color theme"),
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
        title: "Posts",
        keys: &[
            ("n", "new post"),
            ("r", "reply to the selected post"),
            ("l", "like / remove like"),
            ("b", "repost / remove repost"),
            ("f", "follow / unfollow the selected account"),
            ("enter", "open the selected account's profile"),
        ],
    },
    Section {
        title: "Search",
        keys: &[
            ("type", "the search box has focus when you arrive"),
            ("enter", "search, then move through the results"),
            ("/ i", "type in the search box again"),
            ("ctrl+t t", "search posts or accounts"),
            ("esc", "leave the search box"),
        ],
    },
    Section {
        title: "Profile",
        keys: &[
            ("e", "edit your profile"),
            ("f", "follow / unfollow the account shown"),
            ("esc", "back to your own profile"),
        ],
    },
    Section {
        title: "Composer and profile editor",
        keys: &[
            ("ctrl+s", "send the post / save the profile"),
            ("tab", "next field (profile editor)"),
            ("ctrl+u", "clear to the start of the line"),
            ("esc", "close without sending"),
        ],
    },
    Section {
        title: "Theme picker",
        keys: &[
            ("j k", "preview the next / previous theme"),
            ("enter", "use it and remember it"),
            ("esc", "go back to the theme you had"),
        ],
    },
    Section {
        title: "Help",
        keys: &[("j k pgdn pgup", "scroll"), ("esc q ?", "close")],
    },
];

/// The bindings worth showing under the current view, most useful first.
pub fn hints(app: &App) -> Vec<Hint> {
    if app.login.is_some() {
        return vec![
            ("enter", "next / log in"),
            ("tab", "switch field"),
            ("esc", "quit"),
        ];
    }
    match &app.overlay {
        Some(Overlay::Compose(_)) => {
            return vec![
                ("ctrl+s", "send"),
                ("ctrl+u", "clear line"),
                ("esc", "cancel"),
            ];
        }
        Some(Overlay::EditProfile(_)) => {
            return vec![("tab", "next field"), ("ctrl+s", "save"), ("esc", "cancel")];
        }
        Some(Overlay::Help { .. }) => return vec![("j k", "scroll"), ("esc", "close")],
        Some(Overlay::Themes { .. }) => {
            return vec![("j k", "preview"), ("enter", "apply"), ("esc", "cancel")];
        }
        None => {}
    }
    let mut v: Vec<Hint> = match app.tab {
        Tab::Search if app.search.editing => {
            return vec![
                ("enter", "search"),
                ("ctrl+t", "posts / accounts"),
                ("tab", "next tab"),
                ("esc", "done"),
            ];
        }
        Tab::Search if app.search.mode == SearchMode::Accounts => vec![
            ("j k", "move"),
            ("f", "follow"),
            ("enter", "profile"),
            ("/", "edit query"),
            ("t", "posts"),
        ],
        Tab::Search => vec![
            ("j k", "move"),
            ("l", "like"),
            ("b", "repost"),
            ("r", "reply"),
            ("enter", "profile"),
            ("/", "edit query"),
            ("t", "accounts"),
        ],
        Tab::Timeline => vec![
            ("j k", "move"),
            ("l", "like"),
            ("b", "repost"),
            ("r", "reply"),
            ("n", "post"),
            ("f", "unfollow"),
            ("enter", "profile"),
            ("R", "refresh"),
        ],
        Tab::Profile if app.profile.actor.is_some() => vec![
            ("j k", "move"),
            ("f", "follow"),
            ("l", "like"),
            ("r", "reply"),
            ("esc", "my profile"),
        ],
        Tab::Profile => vec![
            ("j k", "move"),
            ("e", "edit profile"),
            ("l", "like"),
            ("r", "reply"),
            ("R", "reload"),
        ],
    };
    // Help comes first: a narrow terminal cuts the row from the right, and `?`
    // is the key that leads to every other one.
    v.insert(0, ("?", "help"));
    v.push(("q", "quit"));
    v
}
