//! The key bindings as the user is told about them: the hint row under the
//! current view, and the sections of the `?` help. Both read from here so a
//! binding cannot be documented in one place and forgotten in the other.

use crate::api::types::Media;
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
            ("1 2 3 4", "Timeline, Search, Notifications, Profile"),
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
            ("v", "open the thread: the posts above it and every reply"),
            ("space", "view the post's pictures or video full screen"),
            ("enter", "open the selected account's profile"),
        ],
    },
    Section {
        title: "Thread",
        keys: &[
            ("j k", "move through the posts and replies"),
            ("l b r f", "like, repost, reply, follow on any of them"),
            ("v", "open the thread of the selected reply"),
            ("esc", "close the thread"),
        ],
    },
    Section {
        title: "Search",
        keys: &[
            ("type", "an empty search box has focus when you arrive"),
            ("enter", "search, then move through the results"),
            ("/ i", "type in the search box again"),
            ("ctrl+t t", "search posts or accounts"),
            ("f", "follow / unfollow the account, or the post's author"),
            ("esc", "leave the search box"),
        ],
    },
    Section {
        title: "Notifications",
        keys: &[
            ("enter", "open the profile of who it is from"),
            ("r l b", "reply, like, repost a reply, mention, or quote"),
            ("v", "open the thread it is about"),
            ("R", "load new notifications"),
        ],
    },
    Section {
        title: "Profile",
        keys: &[
            ("e", "edit your profile"),
            ("f", "follow / unfollow the account shown"),
            ("esc", "back to the list it was opened from"),
        ],
    },
    Section {
        title: "Composer and profile editor",
        keys: &[
            ("ctrl+s", "send the post / save the profile"),
            (
                "ctrl+o",
                "attach pictures (up to 4) or one video / choose a new avatar",
            ),
            (
                "tab",
                "next field: an attachment's alt text, the next profile field",
            ),
            (
                "ctrl+x",
                "remove the attachment being described, or the last one",
            ),
            ("ctrl+u", "clear to the start of the line"),
            ("esc", "close without sending"),
        ],
    },
    Section {
        title: "File browser",
        keys: &[
            ("j k", "move; a picture is previewed, a video described"),
            ("enter l", "open the folder / choose the file"),
            ("space", "mark pictures to choose together with enter"),
            ("h backspace", "the folder above"),
            (".", "show or hide hidden files"),
            ("~", "your home folder"),
            ("esc", "close without choosing"),
        ],
    },
    Section {
        title: "Viewer",
        keys: &[
            ("← → h l", "previous / next picture"),
            ("r", "play the video again"),
            ("d", "download it to the download folder (Downloads/bs)"),
            ("esc q", "back to where you were"),
        ],
    },
    Section {
        title: "Theme picker",
        keys: &[
            ("j k", "preview the next / previous theme"),
            ("g G pgdn pgup", "first / last / ten further"),
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
        Some(Overlay::Compose(c)) if c.browser.is_some() => return browser_hints(),
        Some(Overlay::EditProfile(e)) if e.browser.is_some() => return browser_hints(),
        Some(Overlay::Compose(c)) => {
            let mut v = vec![("ctrl+s", "send"), ("ctrl+o", "attach")];
            if !c.media.is_empty() {
                v.push(("tab", "alt text"));
                v.push(("ctrl+x", "remove picture"));
            }
            v.push(("esc", "cancel"));
            return v;
        }
        Some(Overlay::EditProfile(_)) => {
            return vec![
                ("tab", "next field"),
                ("ctrl+o", "choose avatar"),
                ("ctrl+s", "save"),
                ("esc", "cancel"),
            ];
        }
        Some(Overlay::Help { .. }) => return vec![("j k", "scroll"), ("esc", "close")],
        Some(Overlay::Viewer { media, index, .. }) => {
            let mut v = Vec::new();
            if media.len() > 1 {
                v.push(("← →", "previous / next"));
            }
            if matches!(media.get(*index), Some(Media::Video { .. })) {
                v.push(("r", "replay"));
            }
            v.push(("d", "download"));
            v.push(("esc", "back"));
            return v;
        }
        Some(Overlay::Themes { .. }) => {
            return vec![("j k", "preview"), ("enter", "apply"), ("esc", "cancel")];
        }
        None => {}
    }
    if !app.threads.is_empty() {
        return vec![
            ("esc", "back"),
            ("j k", "move"),
            ("l", "like"),
            ("b", "repost"),
            ("r", "reply"),
            ("v", "thread"),
            ("space", "view"),
            ("enter", "profile"),
            ("?", "help"),
        ];
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
            ("f", "follow"),
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
            ("v", "thread"),
            ("space", "view"),
            ("n", "post"),
            ("f", "unfollow"),
            ("enter", "profile"),
            ("R", "refresh"),
        ],
        Tab::Notifications => vec![
            ("j k", "move"),
            ("enter", "profile"),
            ("r", "reply"),
            ("l", "like"),
            ("v", "thread"),
            ("space", "view"),
            ("R", "refresh"),
        ],
        Tab::Profile if app.profile.actor.is_some() => vec![
            ("esc", back_label(app.profile.came_from)),
            ("j k", "move"),
            ("f", "follow"),
            ("l", "like"),
            ("r", "reply"),
        ],
        Tab::Profile => {
            let mut v = vec![
                ("j k", "move"),
                ("e", "edit profile"),
                ("l", "like"),
                ("r", "reply"),
                ("R", "reload"),
            ];
            // Your own profile, opened from a list: Esc still goes back.
            if app.profile.came_from.is_some() {
                v.insert(0, ("esc", back_label(app.profile.came_from)));
            }
            v
        }
    };
    // Help comes first: a narrow terminal cuts the row from the right, and `?`
    // is the key that leads to every other one.
    v.insert(0, ("?", "help"));
    v.push(("q", "quit"));
    v
}

fn browser_hints() -> Vec<Hint> {
    vec![
        ("enter", "open / choose"),
        ("space", "mark"),
        ("h", "up"),
        (".", "hidden"),
        ("esc", "cancel"),
    ]
}

/// What Esc on the Profile tab goes back to.
fn back_label(from: Option<Tab>) -> &'static str {
    match from {
        Some(Tab::Search) => "back to search",
        Some(Tab::Timeline) => "back to timeline",
        Some(Tab::Notifications) => "back to notifications",
        _ => "my profile",
    }
}
