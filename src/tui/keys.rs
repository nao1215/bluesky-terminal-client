//! The key bindings as the user is told about them: the hint row under the
//! current view, and the sections of the `?` help. Both read from here so a
//! binding cannot be documented in one place and forgotten in the other.

use crossterm::event::{KeyCode, KeyEvent};

use crate::api::types::Media;
use crate::tui::app::{App, Overlay, SearchMode, SettingEdit, Tab};

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
            ("A", "accounts: switch, add, log out"),
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
        keys: &[(
            "[ ]",
            "previous / next feed: Following, then the feeds you pinned",
        )],
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
            (
                "space",
                "view the post's pictures or video full screen, or open its link",
            ),
            (
                "o",
                "open the post's link in the web browser, or the post itself",
            ),
            (
                ".",
                "everything these keys do to the selected post, as a list",
            ),
            ("Q", "quote the selected post in a new post"),
            ("c", "copy the post's address to the clipboard"),
            (
                "D",
                "delete your own post: y confirms, any other key keeps it",
            ),
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
            (
                "s",
                "settings: theme, pictures, and where bsky keeps things",
            ),
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
            (
                "d",
                "download it to the download folder (Downloads/bsky, or the one in the settings)",
            ),
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
        title: "Accounts",
        keys: &[
            ("A", "the logged-in accounts, the one in use marked"),
            ("j k", "move"),
            ("enter", "use the selected account"),
            ("a", "log in another account"),
            ("x", "log the selected account out: y confirms"),
            ("esc", "close"),
        ],
    },
    Section {
        title: "Settings",
        keys: &[
            ("s", "open them, on the Profile tab of your own profile"),
            ("j k", "move"),
            (
                "enter space",
                "change it: a theme, pictures off or on, a folder, an address",
            ),
            (
                "x",
                "a setting kept in settings.json goes back to its default",
            ),
            ("", "a setting a BSKY_ variable fixes says so and stays"),
            ("esc", "close"),
        ],
    },
    Section {
        title: "Help",
        keys: &[("j k pgdn pgup", "scroll"), ("esc q ?", "close")],
    },
];

/// The help as it applies to this terminal. Without pictures there is no
/// viewer, and `space` opens a post in the web browser instead.
pub fn help(pictures: bool) -> Vec<(&'static str, Vec<Hint>)> {
    HELP.iter()
        .filter(|s| pictures || s.title != "Viewer")
        .map(|s| {
            let keys = s
                .keys
                .iter()
                .map(|&(k, d)| match (pictures, s.title, k) {
                    (false, "Posts", "space") => (
                        k,
                        "open the post in the web browser: on bsky.app when it has pictures or video, else its link",
                    ),
                    (false, "File browser", "j k") => (k, "move; a video is described"),
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
    if let Some(form) = &app.login {
        return vec![
            ("enter", "next / log in"),
            ("tab", "switch field"),
            ("esc", if form.adding { "back" } else { "quit" }),
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
        Some(Overlay::Settings {
            edit: Some(SettingEdit::Folder(_)),
            ..
        }) => {
            return vec![
                ("enter", "open"),
                ("space", "choose this folder"),
                ("h", "up"),
                ("esc", "cancel"),
            ];
        }
        Some(Overlay::Settings {
            edit: Some(SettingEdit::Text(_)),
            ..
        }) => return vec![("enter", "keep"), ("esc", "cancel")],
        Some(Overlay::Settings { selected, .. }) => {
            let mut v = vec![("j k", "move"), ("enter", "change")];
            if app
                .settings_rows()
                .get(*selected)
                .is_some_and(|r| r.resettable)
            {
                v.push(("x", "default"));
            }
            v.push(("esc", "close"));
            return v;
        }
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
            ("?", "help"),
            ("j k", "move"),
            (".", "actions"),
            ("space", "view"),
            ("esc", "back"),
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
            (".", "actions"),
            ("/", "edit query"),
            ("t", "posts"),
        ],
        Tab::Search => vec![
            ("j k", "move"),
            (".", "actions"),
            ("/", "edit query"),
            ("t", "accounts"),
        ],
        Tab::Timeline => vec![
            ("j k", "move"),
            (".", "actions"),
            ("space", "view"),
            ("n", "post"),
            ("R", "refresh"),
        ],
        Tab::Notifications => vec![
            ("j k", "move"),
            (".", "actions"),
            ("space", "view"),
            ("R", "refresh"),
        ],
        Tab::Profile if app.profile.actor.is_some() => vec![
            ("esc", back_label(app.profile.came_from)),
            ("j k", "move"),
            (".", "actions"),
            ("space", "view"),
        ],
        Tab::Profile => {
            let mut v = vec![
                ("j k", "move"),
                (".", "actions"),
                ("e", "edit profile"),
                ("s", "settings"),
                ("R", "reload"),
            ];
            // Your own profile, opened from a list: Esc still goes back.
            if app.profile.came_from.is_some() {
                v.insert(0, ("esc", back_label(app.profile.came_from)));
            }
            v
        }
    };
    // With feeds pinned, the Timeline tab goes through them; in a custom
    // feed the author is often not followed, so f follows.
    if app.tab == Tab::Timeline && app.threads.is_empty() && app.overlay.is_none() {
        if app.feed > 0
            && let Some(f) = v.iter_mut().find(|(k, _)| *k == "f")
        {
            f.1 = "follow";
        }
        if !app.feeds.is_empty() {
            v.insert(1, ("[ ]", "feed"));
        }
    }
    // Help comes first: a narrow terminal cuts the row from the right, and `?`
    // is the key that leads to every other one.
    v.insert(0, ("?", "help"));
    v.push(("q", "quit"));
    v
}

/// What `.` offers on the selected post or account: the keys of this view
/// that act on it, each saying what it would do now. The list is the one
/// place the keys of a post are all together, so the hint row does not have
/// to carry them.
pub fn actions(app: &App) -> Vec<Hint> {
    if app.login.is_some()
        || app.overlay.is_some() && !matches!(app.overlay, Some(Overlay::Actions { .. }))
    {
        return Vec::new();
    }
    let mut v: Vec<Hint> = Vec::new();
    if let Some(post) = app.shown_post() {
        v.push(("r", "reply to it"));
        v.push((
            "l",
            if post.like_uri().is_some() {
                "remove your like"
            } else {
                "like it"
            },
        ));
        v.push((
            "b",
            if post.viewer.as_ref().is_some_and(|x| x.repost.is_some()) {
                "remove your repost"
            } else {
                "repost it"
            },
        ));
        v.push(("Q", "quote it in a new post"));
        v.push((
            "space",
            if app.pictures {
                "view its pictures or video"
            } else {
                "open it in the web browser"
            },
        ));
        v.push(("o", "open its link in the web browser"));
        v.push(("v", "open the thread"));
        v.push(("c", "copy its address"));
    }
    if let Some(account) = app.shown_account() {
        v.push(("enter", "open the profile"));
        v.push((
            "f",
            if account
                .viewer
                .as_ref()
                .is_some_and(|x| x.following.is_some())
            {
                "unfollow"
            } else {
                "follow"
            },
        ));
    }
    if app.own_post_selected() {
        v.push(("D", "delete your post"));
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
