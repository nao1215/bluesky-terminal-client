//! The key bindings as the user is told about them: the hint row under the
//! current view, and the sections of the `?` help. Both read from here so a
//! binding cannot be documented in one place and forgotten in the other.

use crossterm::event::{KeyCode, KeyEvent};

use crate::api::types::Media;
use crate::i18n::{n, t};
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
        title: n!("Global"),
        keys: &[
            ("1 2 3 4 5", n!("the tabs, in the order shown")),
            ("tab shift+tab", n!("next / previous tab")),
            ("T", n!("choose a color theme")),
            ("A", n!("accounts: switch, add, log out")),
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
            ("l", n!("like / remove like")),
            ("b", n!("repost / remove repost")),
            ("f", n!("follow / unfollow the selected account")),
            ("M", n!("mute / unmute the selected account")),
            ("B", n!("block / unblock the account: y confirms")),
            ("v", n!("open the thread with every reply")),
            ("space", n!("view pictures or video, or open the link")),
            ("o", n!("open the link, or the post, in the browser")),
            (".", n!("list what the keys do to the post")),
            ("Q", n!("quote the selected post in a new post")),
            ("c", n!("copy the post's address to the clipboard")),
            ("D", n!("delete your own post: y confirms")),
            ("enter", n!("open the selected account's profile")),
        ],
    },
    Section {
        title: n!("Thread"),
        keys: &[
            ("j k", n!("move through the posts and replies")),
            ("l b r f", n!("like, repost, reply, follow on any of them")),
            ("v", n!("open the thread of the selected reply")),
            ("esc", n!("close the thread")),
        ],
    },
    Section {
        title: n!("Search"),
        keys: &[
            ("type", n!("the search box has focus when you arrive")),
            ("enter", n!("search, then move through the results")),
            ("/ i", n!("type in the search box again")),
            ("ctrl+t t", n!("search posts or accounts")),
            ("f", n!("follow / unfollow the account or author")),
            ("esc", n!("leave the search box")),
        ],
    },
    Section {
        title: n!("Notifications"),
        keys: &[
            ("enter", n!("open the profile of who it is from")),
            ("r l b", n!("reply, like, repost a reply or mention")),
            ("v", n!("open the thread it is about")),
            ("R", n!("load new notifications")),
        ],
    },
    Section {
        title: n!("Profile"),
        keys: &[
            ("e", n!("edit your profile")),
            ("m", n!("message the account shown")),
            ("s", n!("settings: theme, pictures, folders")),
            ("f", n!("follow / unfollow the account shown")),
            ("M B", n!("mute / block the account shown")),
            ("esc", n!("back to the list it was opened from")),
        ],
    },
    Section {
        title: n!("Composer and profile editor"),
        keys: &[
            ("ctrl+s", n!("send the post / save the profile")),
            ("ctrl+o", n!("attach pictures or a video / an avatar")),
            ("tab", n!("next field, or a picture's alt text")),
            ("ctrl+x", n!("remove the attachment")),
            ("ctrl+u", n!("clear to the start of the line")),
            ("esc", n!("close without sending")),
        ],
    },
    Section {
        title: n!("File browser"),
        keys: &[
            ("j k", n!("move; pictures are previewed")),
            ("enter l", n!("open the folder / choose the file")),
            ("space", n!("mark pictures to choose together with enter")),
            ("h backspace", n!("the folder above")),
            (".", n!("show or hide hidden files")),
            ("~", n!("your home folder")),
            ("esc", n!("close without choosing")),
        ],
    },
    Section {
        title: n!("Viewer"),
        keys: &[
            ("← → h l", n!("previous / next picture")),
            ("r", n!("play the video again")),
            ("d", n!("save it in the download folder")),
            ("esc q", n!("back to where you were")),
        ],
    },
    Section {
        title: n!("Theme picker"),
        keys: &[
            ("j k", n!("preview the next / previous theme")),
            ("g G pgdn pgup", n!("first / last / ten further")),
            ("enter", n!("use it and remember it")),
            ("esc", n!("go back to the theme you had")),
        ],
    },
    Section {
        title: n!("Columns"),
        keys: &[
            ("← → H L", n!("the column to the left / right")),
            ("+", n!("add a column: a pinned feed, notifications…")),
            ("", n!("the last one removed, the timeline is alone")),
            ("< >", n!("move the column left / right")),
            ("x", n!("remove the column: y confirms")),
            ("R", n!("load the column again")),
            ("", n!("post keys act on the column's post")),
        ],
    },
    Section {
        title: n!("Chat"),
        keys: &[
            ("enter", n!("open the conversation; it is marked read")),
            ("i enter", n!("write a message; enter sends it")),
            ("j k g G", n!("read further back / forward")),
            ("esc", n!("back to the conversations")),
            ("", n!("read again every 15 s while shown")),
        ],
    },
    Section {
        title: n!("Accounts"),
        keys: &[
            ("A", n!("the accounts, the one in use marked")),
            ("j k", n!("move")),
            ("enter", n!("use the selected account")),
            ("a", n!("log in another account")),
            ("x", n!("log the selected account out: y confirms")),
            ("esc", n!("close")),
        ],
    },
    Section {
        title: n!("Settings"),
        keys: &[
            ("s", n!("open them from your own Profile tab")),
            ("j k", n!("move")),
            ("enter space", n!("change the selected setting")),
            ("x", n!("put the setting back to its default")),
            ("", n!("a setting a BSKY_ variable sets stays")),
            ("esc", n!("close")),
        ],
    },
    Section {
        title: n!("Help"),
        keys: &[("j k pgdn pgup", n!("scroll")), ("esc q ?", n!("close"))],
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
                    (false, "Posts", "space") => {
                        (k, n!("open the post or its link in the browser"))
                    }
                    (false, "File browser", "j k") => (k, n!("move; a video is described")),
                    _ => (k, d),
                })
                .map(|(k, d)| (k, t(d)))
                .collect();
            (t(s.title), keys)
        })
        .collect()
}

/// The bindings worth showing under the current view, most useful first.
pub fn hints(app: &App) -> Vec<Hint> {
    let mut v = view_hints(app);
    if !app.pictures {
        for h in &mut v {
            if *h == ("space", "view") {
                h.1 = n!("open in browser");
            }
        }
    }
    translated(v)
}

/// The hints with what each does in the language in use.
fn translated(mut v: Vec<Hint>) -> Vec<Hint> {
    for h in &mut v {
        h.1 = t(h.1);
    }
    v
}

fn view_hints(app: &App) -> Vec<Hint> {
    if let Some(form) = &app.login {
        return vec![
            ("enter", n!("next / log in")),
            ("tab", n!("switch field")),
            ("esc", if form.adding { n!("back") } else { n!("quit") }),
        ];
    }
    match &app.overlay {
        Some(Overlay::Compose(c)) if c.browser.is_some() => return browser_hints(),
        Some(Overlay::EditProfile(e)) if e.browser.is_some() => return browser_hints(),
        Some(Overlay::Compose(c)) => {
            let mut v = vec![("ctrl+s", n!("send")), ("ctrl+o", n!("attach"))];
            if !c.media.is_empty() {
                v.push(("tab", n!("alt text")));
                v.push(("ctrl+x", n!("remove picture")));
            }
            v.push(("esc", n!("cancel")));
            return v;
        }
        Some(Overlay::EditProfile(_)) => {
            return vec![
                ("tab", n!("next field")),
                ("ctrl+o", n!("choose avatar")),
                ("ctrl+s", n!("save")),
                ("esc", n!("cancel")),
            ];
        }
        Some(Overlay::Help { .. }) => return vec![("j k", n!("scroll")), ("esc", n!("close"))],
        Some(Overlay::Actions { .. }) => {
            return vec![
                ("j k", n!("move")),
                ("enter", n!("do it")),
                ("esc", n!("close")),
            ];
        }
        Some(Overlay::AddColumn { query: Some(_), .. }) => {
            return vec![("enter", n!("add")), ("esc", n!("back"))];
        }
        Some(Overlay::AddColumn { .. }) => {
            return vec![
                ("j k", n!("move")),
                ("enter", n!("add")),
                ("esc", n!("close")),
            ];
        }
        Some(Overlay::Languages { .. }) => {
            return vec![
                ("j k", n!("move")),
                ("enter", n!("use")),
                ("esc", n!("back")),
            ];
        }
        Some(Overlay::Accounts { .. }) => {
            return vec![
                ("j k", n!("move")),
                ("enter", n!("use")),
                ("a", n!("add")),
                ("x", n!("log out")),
                ("esc", n!("close")),
            ];
        }
        Some(Overlay::Settings {
            edit: Some(SettingEdit::Folder(_)),
            ..
        }) => {
            return vec![
                ("enter", n!("open")),
                ("space", n!("choose this folder")),
                ("h", n!("up")),
                ("esc", n!("cancel")),
            ];
        }
        Some(Overlay::Settings {
            edit: Some(SettingEdit::Text(_)),
            ..
        }) => return vec![("enter", n!("keep")), ("esc", n!("cancel"))],
        Some(Overlay::Settings { selected, .. }) => {
            let mut v = vec![("j k", n!("move")), ("enter", n!("change"))];
            if app
                .settings_rows()
                .get(*selected)
                .is_some_and(|r| r.resettable)
            {
                v.push(("x", n!("default")));
            }
            v.push(("esc", n!("close")));
            return v;
        }
        Some(Overlay::Viewer { media, index, .. }) => {
            let mut v = Vec::new();
            if media.len() > 1 {
                v.push(("← →", n!("previous / next")));
            }
            if matches!(media.get(*index), Some(Media::Video { .. })) {
                v.push(("r", n!("replay")));
            }
            v.push(("d", n!("download")));
            v.push(("esc", n!("back")));
            return v;
        }
        Some(Overlay::Themes { .. }) => {
            return vec![
                ("j k", n!("preview")),
                ("enter", n!("apply")),
                ("esc", n!("cancel")),
            ];
        }
        None => {}
    }
    if !app.threads.is_empty() {
        return vec![
            ("?", n!("help")),
            ("j k", n!("move")),
            (".", n!("actions")),
            ("space", n!("view")),
            ("esc", n!("back")),
        ];
    }
    let mut v: Vec<Hint> = match app.tab {
        Tab::Search if app.search.editing => {
            return vec![
                ("enter", n!("search")),
                ("ctrl+t", n!("posts / accounts")),
                ("tab", n!("next tab")),
                ("esc", n!("done")),
            ];
        }
        Tab::Search if app.search.mode == SearchMode::Accounts => vec![
            ("j k", n!("move")),
            (".", n!("actions")),
            ("/", n!("edit query")),
            ("t", n!("posts")),
        ],
        Tab::Search => vec![
            ("j k", n!("move")),
            (".", n!("actions")),
            ("/", n!("edit query")),
            ("t", n!("accounts")),
        ],
        Tab::Timeline => vec![
            ("j k", n!("move")),
            (".", n!("actions")),
            ("space", n!("view")),
            ("n", n!("post")),
            ("R", n!("refresh")),
        ],
        Tab::Chat => match &app.chat.open {
            Some(o) if o.typing => return vec![("enter", n!("send")), ("esc", n!("stop writing"))],
            Some(_) => vec![
                ("i", n!("write")),
                ("j k", n!("scroll")),
                ("esc", n!("back")),
            ],
            None => vec![
                ("j k", n!("move")),
                ("enter", n!("open")),
                ("R", n!("refresh")),
            ],
        },
        Tab::Columns => vec![
            ("← →", n!("column")),
            ("j k", n!("move")),
            (".", n!("actions")),
            ("+", n!("add")),
            ("x", n!("remove")),
        ],
        Tab::Notifications => vec![
            ("j k", n!("move")),
            (".", n!("actions")),
            ("space", n!("view")),
            ("R", n!("refresh")),
        ],
        Tab::Profile if app.profile.actor.is_some() => vec![
            ("esc", back_label(app.profile.came_from)),
            ("j k", n!("move")),
            (".", n!("actions")),
            ("space", n!("view")),
        ],
        Tab::Profile => {
            let mut v = vec![
                ("j k", n!("move")),
                (".", n!("actions")),
                ("e", n!("edit profile")),
                ("s", n!("settings")),
                ("R", n!("reload")),
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
    v.insert(0, ("?", n!("help")));
    v.push(("q", n!("quit")));
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
        v.push(("r", n!("reply to it")));
        v.push((
            "l",
            if post.like_uri().is_some() {
                n!("remove your like")
            } else {
                n!("like it")
            },
        ));
        v.push((
            "b",
            if post.viewer.as_ref().is_some_and(|x| x.repost.is_some()) {
                n!("remove your repost")
            } else {
                n!("repost it")
            },
        ));
        v.push(("Q", n!("quote it in a new post")));
        v.push((
            "space",
            if app.pictures {
                n!("view its pictures or video")
            } else {
                n!("open it in the web browser")
            },
        ));
        v.push(("o", n!("open its link in the web browser")));
        v.push(("v", n!("open the thread")));
        v.push(("c", n!("copy its address")));
    } else if app.subject_shown().is_some() {
        // A like or a repost: these keys act on the post it is about.
        v.push((
            "space",
            if app.pictures {
                n!("view the post's pictures or video")
            } else {
                n!("open the post in the web browser")
            },
        ));
        v.push(("o", n!("open the post's link in the web browser")));
        v.push(("v", n!("open the post's thread")));
        v.push(("c", n!("copy the post's address")));
    }
    if let Some(account) = app.shown_account() {
        if app.tab == Tab::Profile
            && app.threads.is_empty()
            && app.profile.actor.is_some()
            && app.session.as_ref().is_none_or(|s| s.did != account.did)
        {
            v.push(("m", n!("message them")));
        }
        // On the Profile tab the profile is open already, and Enter opens
        // nothing.
        if app.tab != Tab::Profile || !app.threads.is_empty() {
            v.push(("enter", n!("open the profile")));
        }
        let yourself = app.session.as_ref().is_some_and(|s| s.did == account.did);
        if !yourself {
            v.push((
                "f",
                if account
                    .viewer
                    .as_ref()
                    .is_some_and(|x| x.following.is_some())
                {
                    n!("unfollow")
                } else {
                    n!("follow")
                },
            ));
        }
        if !yourself {
            v.push((
                "M",
                if account.muted() {
                    n!("unmute them")
                } else {
                    n!("mute them")
                },
            ));
            v.push((
                "B",
                if account.blocking_uri().is_some() {
                    n!("unblock them")
                } else {
                    n!("block them (y confirms)")
                },
            ));
        }
    }
    if app.own_post_selected() {
        v.push(("D", n!("delete your post")));
    }
    translated(v)
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
        ("enter", n!("open / choose")),
        ("space", n!("mark")),
        ("h", n!("up")),
        (".", n!("hidden")),
        ("esc", n!("cancel")),
    ]
}

/// What Esc on the Profile tab goes back to.
fn back_label(from: Option<Tab>) -> &'static str {
    match from {
        Some(Tab::Search) => n!("back to search"),
        Some(Tab::Timeline) => n!("back to timeline"),
        Some(Tab::Notifications) => n!("back to notifications"),
        Some(Tab::Columns) => n!("back to timeline"),
        _ => n!("my profile"),
    }
}
