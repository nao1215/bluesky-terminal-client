//! English and Japanese key help for the full-screen client.
pub(super) const HELP: &[(&str, &str)] = &[
    (
        "1…4 / tab",
        terminal_kit::n!("Home / Notifications / Profile / Chat"),
    ),
    ("j k / ↓ ↑", terminal_kit::n!("move the selection")),
    ("g G / home end", terminal_kit::n!("first / last note")),
    (
        "pgdn pgup",
        terminal_kit::n!("move five notes; more loads near the end"),
    ),
    ("R F5", terminal_kit::n!("refresh")),
    (
        "v / V / esc",
        terminal_kit::n!("open thread / quoted note / back"),
    ),
    ("n / r", terminal_kit::n!("new note / reply")),
    (
        "ctrl+s",
        terminal_kit::n!("send note / message; save profile"),
    ),
    (
        "ctrl+u / esc",
        terminal_kit::n!("clear line / cancel composer"),
    ),
    ("l", terminal_kit::n!("reaction / deletion request")),
    ("b", terminal_kit::n!("repost")),
    ("c", terminal_kit::n!("copy nostr:nevent identifier")),
    ("space", terminal_kit::n!("open pictures / video")),
    (
        "h l / ← →",
        terminal_kit::n!("previous / next picture in viewer"),
    ),
    ("s / T", terminal_kit::n!("settings / themes")),
    ("j k / enter", terminal_kit::n!("choose a setting or theme")),
    ("x", terminal_kit::n!("reset the selected setting")),
    ("? / esc", terminal_kit::n!("help / close")),
    ("enter / esc", terminal_kit::n!("author profile / back")),
    (
        "/ / m / i",
        terminal_kit::n!("search / chat recipient / message"),
    ),
    (
        "Esc",
        terminal_kit::n!("back to timeline from search results"),
    ),
    (". / A", terminal_kit::n!("actions / accounts")),
    ("e", terminal_kit::n!("edit own profile / reaction")),
    ("Q / w", terminal_kit::n!("quote / reveal content warning")),
    (
        "ctrl+o / tab / ctrl+x",
        terminal_kit::n!("attach / alt text (shift+tab previous) / remove attachment"),
    ),
    ("ctrl+w", terminal_kit::n!("edit content warning")),
    (
        "D / f / M B / !",
        terminal_kit::n!("delete / follow / mute / public report (confirm)"),
    ),
    (
        "d / viewer r",
        terminal_kit::n!("save attachments / replay video"),
    ),
    (
        "+ / H L / < >",
        terminal_kit::n!("pin search / select column / reorder"),
    ),
    (
        "I",
        terminal_kit::n!("publish configured DM relay list (confirm)"),
    ),
    ("q ctrl+c", terminal_kit::n!("quit")),
];

use super::app::{App, Overlay};
pub(super) fn search_hints(app: &App) -> Option<Vec<(&'static str, &'static str)>> {
    let lang =
        terminal_kit::i18n::Lang::from_code(app.language).unwrap_or(terminal_kit::i18n::Lang::En);
    if matches!(
        app.extra.dialog,
        Some(super::extras::Dialog::Input {
            purpose: super::extras::Purpose::Search,
            ..
        })
    ) {
        return Some(terminal_kit::view::search_edit_hints(false, lang));
    }
    if app.searching()
        && app.threads.is_empty()
        && app.overlay.is_none()
        && app.extra.dialog.is_none()
    {
        return Some(terminal_kit::view::search_result_hints(false, lang));
    }
    None
}

pub(super) fn hint(app: &App) -> &str {
    match app.overlay {
        Some(Overlay::Compose { .. }) => app.t("ctrl+s send   esc cancel"),
        Some(Overlay::Themes { .. }) => app.t("j k preview   enter apply   esc cancel"),
        Some(Overlay::Settings(_)) => app.t("j k move   enter choose   x reset   esc close"),
        Some(Overlay::Languages(_)) => app.t("j k move   enter choose   esc close"),
        Some(Overlay::Pictures { .. }) => app.t("h l previous / next   d save   r replay   esc close"),
        Some(Overlay::Help(_)) => app.t("j k scroll   esc close"),
        None => app.t("? help   / search   1–4 tabs   j k move   tab next tab   n post   r reply   l like   b repost   v thread   c copy   s settings   T themes   q quit"),
    }
}

pub(super) fn browser_ja(value: &str) -> &str {
    crate::i18n::CATALOG.t(terminal_kit::i18n::Lang::Ja, value)
}

pub(super) const ACTIONS: &[(char, &str)] = &[
    ('r', terminal_kit::n!("Reply")),
    ('Q', terminal_kit::n!("Quote")),
    ('l', terminal_kit::n!("Like / undo")),
    ('b', terminal_kit::n!("Repost / undo")),
    ('f', terminal_kit::n!("Follow / unfollow")),
    ('M', terminal_kit::n!("Mute / unmute")),
    ('D', terminal_kit::n!("Deletion request")),
    ('!', terminal_kit::n!("Public report")),
    ('d', terminal_kit::n!("Save attachments")),
];
