//! The key bindings as the user is told about them: the hint row under the
//! current view, the sections of the `?` help, and what `.` lists. All read
//! from here, as in bsky, so a binding cannot be documented in one place and
//! forgotten in the other. The keys are bsky's where bsky has the action.

use crossterm::event::{KeyCode, KeyEvent};

use crate::i18n::{n, t};
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
        title: n!("General"),
        keys: &[
            ("1 2 3 …", n!("the tabs, in the order shown")),
            ("tab shift+tab", n!("next / previous tab")),
            ("", n!("which tabs, and their order: the tabs setting")),
            (
                "+",
                n!("add a tab: one not shown, an antenna, a list, a channel"),
            ),
            ("x", n!("close the tab shown: y confirms")),
            ("A", n!("accounts: switch, log out")),
            (
                "s",
                n!("settings: theme, language, download folder, browser, tabs"),
            ),
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
        title: n!("Notes"),
        keys: &[
            ("n", n!("new note")),
            ("r", n!("reply to the selected note")),
            ("Q", n!("quote the selected note in a new note")),
            ("l", n!("react ❤ / take back your reaction")),
            (
                "e",
                n!("choose a reaction: recent, default, or the server's emoji by name"),
            ),
            ("b", n!("renote / take back your renote")),
            ("D", n!("delete your own note: y confirms")),
            (
                "!",
                n!("report the note and its author to the moderators: y confirms"),
            ),
            ("enter", n!("open the profile of its author")),
            ("v", n!("open the thread with its replies")),
            ("space", n!("view the pictures and files")),
            ("w", n!("show / hide the text behind a content warning")),
            ("d", n!("save the note's files in the download folder")),
            ("o", n!("open the note in the browser")),
            ("c", n!("copy the note's address to the clipboard")),
            (".", n!("list what the keys do to the note")),
        ],
    },
    Section {
        title: n!("Thread"),
        keys: &[
            ("j k", n!("move through the notes and replies")),
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
            ("ctrl+t t", n!("search notes or accounts")),
            ("enter", n!("open the account's profile")),
            ("f", n!("follow / unfollow the account")),
            ("esc", n!("leave the search box")),
        ],
    },
    Section {
        title: n!("Profile"),
        keys: &[
            ("", n!("its tab is your own profile")),
            (
                "e",
                n!("edit your profile: name, description, avatar, banner"),
            ),
            ("m", n!("message the account shown")),
            ("f", n!("follow / unfollow the account shown")),
            ("M", n!("mute / unmute the account shown")),
            ("B", n!("block / unblock the account: y confirms")),
            (
                "!",
                n!("report the account, or the note selected, to the moderators: y confirms"),
            ),
            (
                "",
                n!("an account that approves followers, or of another server, is asked"),
            ),
            ("esc", n!("back to the list it was opened from")),
        ],
    },
    Section {
        title: n!("Notifications"),
        keys: &[
            ("enter", n!("open the profile of who it is from")),
            (
                "v space",
                n!("the thread or the files of the note it is about"),
            ),
            ("r l b", n!("reply, react, renote the note it is about")),
            ("", n!("they are marked read when the tab is shown")),
            ("R", n!("load new notifications")),
        ],
    },
    Section {
        title: n!("Composer"),
        keys: &[
            ("ctrl+s", n!("send the note")),
            ("ctrl+o", n!("attach pictures or a video")),
            ("tab", n!("next field: the warning, a file's alt text")),
            ("ctrl+w", n!("add / take off a content warning")),
            ("ctrl+p", n!("who may see it: public, home, followers")),
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
            ("space", n!("mark files to choose together with enter")),
            ("h backspace", n!("the folder above")),
            (".", n!("show or hide hidden files")),
            ("~", n!("your home folder")),
            ("esc", n!("close without choosing")),
        ],
    },
    Section {
        title: n!("Viewer"),
        keys: &[
            ("← → h l", n!("previous / next file")),
            ("r", n!("play the video again")),
            ("d", n!("save it in the download folder")),
            ("o", n!("open it in the browser")),
            ("esc q", n!("back to where you were")),
        ],
    },
    Section {
        title: n!("Chat"),
        keys: &[
            ("enter", n!("open the conversation")),
            ("i enter", n!("write a message; enter sends it")),
            ("k j", n!("read further back / forward")),
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
            ("x", n!("log the selected account out: y confirms")),
            ("a", n!("log in another account")),
            ("esc", n!("close")),
        ],
    },
    Section {
        title: n!("Log in"),
        keys: &[
            ("j k", n!("choose a server from the list")),
            ("/ i", n!("type another server's host")),
            ("enter", n!("log in there: allow msky in the browser")),
            ("o", n!("open the page that allows it again")),
            ("esc", n!("back, or call the login off")),
        ],
    },
    Section {
        title: n!("Help"),
        keys: &[("j k pgdn pgup", n!("scroll")), ("esc q ?", n!("close"))],
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
                    (false, "Notes", "space") => (k, n!("open the note in the browser")),
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
            if *h == ("space", n!("view")) {
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
    match &app.overlay {
        Some(Overlay::EditProfile(e)) if e.browser.is_some() => {
            return vec![
                ("enter", n!("open / choose")),
                ("h", n!("up")),
                (".", n!("hidden")),
                ("esc", n!("close without choosing")),
            ];
        }
        Some(Overlay::EditProfile(_)) => {
            return vec![
                ("tab", n!("next field")),
                ("ctrl+o", n!("choose a picture")),
                ("ctrl+s", n!("save")),
                ("esc", n!("cancel")),
            ];
        }
        Some(Overlay::Help { .. }) => return vec![("j k", n!("scroll")), ("esc", n!("close"))],
        Some(Overlay::Settings { edit: Some(_), .. }) => {
            return vec![("enter", n!("keep")), ("esc", n!("cancel"))];
        }
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
        Some(Overlay::Themes { .. }) => {
            return vec![
                ("j k", n!("preview")),
                ("enter", n!("apply")),
                ("esc", n!("cancel")),
            ];
        }
        Some(Overlay::Report(r)) if r.writing => {
            return vec![
                ("tab", n!("the reasons")),
                ("ctrl+enter", n!("send")),
                ("esc", n!("stop writing")),
            ];
        }
        Some(Overlay::Report(_)) => {
            return vec![
                ("j k", n!("reason")),
                ("tab", n!("write a comment")),
                ("enter", n!("send: y confirms")),
                ("esc", n!("cancel")),
            ];
        }
        Some(Overlay::AddTab { .. }) => {
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
        Some(Overlay::Compose(c)) if c.browser.is_some() => {
            return vec![
                ("enter", n!("open / choose")),
                ("space", n!("mark")),
                ("h", n!("up")),
                (".", n!("hidden")),
                ("esc", n!("cancel")),
            ];
        }
        Some(Overlay::Compose(c)) => {
            let mut v = vec![
                ("ctrl+s", n!("send")),
                ("ctrl+o", n!("attach")),
                ("ctrl+w", n!("warning")),
                ("ctrl+p", n!("visibility")),
            ];
            if !c.media.is_empty() {
                v.push(("tab", n!("alt text")));
                v.push(("ctrl+x", n!("remove file")));
            }
            v.push(("esc", n!("cancel")));
            return v;
        }
        Some(Overlay::Reactions { .. }) => {
            return vec![
                ("type", n!("search emoji")),
                ("↑ ↓", n!("choose")),
                ("enter", n!("react")),
                ("esc", n!("close")),
            ];
        }
        Some(Overlay::Login(f)) if f.waiting.is_some() => {
            return vec![("o", n!("open the page again")), ("esc", n!("call it off"))];
        }
        Some(Overlay::Login(f)) if f.typed.is_some() => {
            return vec![("enter", n!("log in")), ("esc", n!("back to the list"))];
        }
        Some(Overlay::Login(_)) => {
            return vec![
                ("j k", n!("move")),
                ("enter", n!("log in")),
                ("/", n!("other server")),
                ("esc", n!("back")),
            ];
        }
        Some(Overlay::Actions { .. }) => {
            return vec![
                ("j k", n!("move")),
                ("enter", n!("do it")),
                ("esc", n!("close")),
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
        Some(Overlay::Viewer { files, index, .. }) => {
            let mut v = Vec::new();
            if files.len() > 1 {
                v.push(("← →", n!("previous / next")));
            }
            if files
                .get(*index)
                .is_some_and(|f| f.2.mime.starts_with("video/"))
            {
                v.push(("r", n!("replay")));
            }
            v.push(("d", n!("download")));
            v.push(("o", n!("open")));
            v.push(("esc", n!("back")));
            return v;
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
        Tab::Search if app.search.editing => {
            return vec![
                ("enter", n!("search")),
                ("ctrl+t", n!("notes / accounts")),
                ("tab", n!("next tab")),
                ("esc", n!("done")),
            ];
        }
        Tab::Search if app.search.mode == crate::tui::app::SearchMode::Accounts => vec![
            ("j k", n!("move")),
            (".", n!("actions")),
            ("/", n!("edit query")),
            ("t", n!("notes")),
        ],
        Tab::Search => vec![
            ("j k", n!("move")),
            (".", n!("actions")),
            ("/", n!("edit query")),
            ("t", n!("accounts")),
        ],
        Tab::Profile if !app.profile.own(app.my_user_id()) => vec![
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
            // Your own profile, opened from a list: esc still goes back.
            if app.profile.came_from.is_some() {
                v.insert(0, ("esc", back_label(app.profile.came_from)));
            }
            v
        }
        Tab::Feed(_) => vec![
            ("j k", n!("move")),
            (".", n!("actions")),
            ("space", n!("view")),
            ("R", n!("refresh")),
            ("x", n!("close tab")),
        ],
        Tab::Timeline(_) => vec![
            ("j k", n!("move")),
            ("tab", n!("next tab")),
            (".", n!("actions")),
            ("space", n!("view")),
            ("n", n!("note")),
            ("R", n!("refresh")),
        ],
        Tab::Notifications => vec![
            ("j k", n!("move")),
            (".", n!("actions")),
            ("space", n!("view")),
            ("R", n!("refresh")),
        ],
    };
    // Help comes first: a narrow terminal cuts the row from the right, and
    // `?` is the key that leads to every other one.
    v.insert(0, ("?", n!("help")));
    v.push(("q", n!("quit")));
    v
}

/// What `.` offers on the selected note: the keys of this view that act on
/// it, each saying what it would do now.
pub fn actions(app: &App) -> Vec<Hint> {
    if app.overlay.is_some() && !matches!(app.overlay, Some(Overlay::Actions { .. })) {
        return Vec::new();
    }
    let mut v: Vec<Hint> = Vec::new();
    if let Some(n) = app.shown_note() {
        note_actions(app, n, &mut v);
    }
    account_actions(app, &mut v);
    translated(v)
}

/// What `.` offers on the account shown or found, as bsky's: the keys
/// that act on it, each saying what it would do now.
fn account_actions(app: &App, v: &mut Vec<Hint>) {
    let in_list = app.threads.is_empty();
    let me = app.my_user_id();
    if app.tab == Tab::Profile && in_list {
        if let Some(u) = &app.profile.user
            && !app.profile.own(me)
        {
            v.push(("m", n!("message them")));
            push_guards(v, u);
        }
        return;
    }
    if app.tab == Tab::Search && in_list && app.search.mode == crate::tui::app::SearchMode::Accounts
    {
        if let Some(u) = app.search.users.current() {
            v.push(("enter", n!("open the profile")));
            if Some(u.id.as_str()) != me {
                push_guards(v, u);
            }
        }
        return;
    }
    if app.shown_note().is_some() || (app.tab == Tab::Notifications && in_list) {
        v.push(("enter", n!("open the profile")));
    }
}

/// f, M and B on an account, each saying what it would do now.
fn push_guards(v: &mut Vec<Hint>, u: &crate::api::types::User) {
    v.push((
        "f",
        if u.is_following {
            n!("unfollow")
        } else if u.has_pending_follow_request_from_you {
            n!("take back the follow request")
        } else {
            n!("follow")
        },
    ));
    v.push((
        "M",
        if u.is_muted {
            n!("unmute them")
        } else {
            n!("mute them")
        },
    ));
    v.push((
        "B",
        if u.is_blocking {
            n!("unblock them")
        } else {
            n!("block them (y confirms)")
        },
    ));
}

/// What `.` offers on the selected note.
fn note_actions(app: &App, n: &crate::api::types::Note, v: &mut Vec<Hint>) {
    let files = !n.attached().is_empty();
    v.push(("r", n!("reply to it")));
    v.push((
        "l",
        if n.my_reaction.is_some() {
            n!("take back your reaction")
        } else {
            n!("react ❤")
        },
    ));
    v.push(("e", n!("choose a reaction")));
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
            n!("take back your renote")
        } else {
            n!("renote it")
        },
    ));
    v.push(("Q", n!("quote it in a new note")));
    if app.pictures && files {
        v.push(("space", n!("view its pictures and files")));
    }
    if n.cw.is_some() {
        v.push((
            "w",
            if app.text_shown(n) {
                n!("hide the text behind its warning")
            } else {
                n!("show the text behind its warning")
            },
        ));
    }
    v.push(("v", n!("open the thread")));
    if files {
        v.push(("d", n!("save its files")));
    }
    v.push(("o", n!("open it in the web browser")));
    v.push(("c", n!("copy its address")));
    let mine = app.selected_note().is_some_and(|s| {
        !s.is_pure_renote()
            && app
                .session
                .as_ref()
                .is_some_and(|me| me.user_id == s.author_id())
    });
    if mine {
        v.push(("D", n!("delete your note")));
    } else {
        v.push(("!", n!("report it")));
    }
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

/// What esc on the Profile tab goes back to, as bsky's says it.
fn back_label(from: Option<Tab>) -> &'static str {
    match from {
        Some(Tab::Search) => n!("back to search"),
        Some(Tab::Timeline(_) | Tab::Feed(_)) => n!("back to timeline"),
        Some(Tab::Notifications) => n!("back to notifications"),
        Some(Tab::Chat) => n!("back to chat"),
        _ => n!("my profile"),
    }
}
