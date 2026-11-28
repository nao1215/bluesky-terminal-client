//! Drawing. Every function here reads the [`App`] and writes to a frame;
//! the only state it changes is each list's scroll offset.

use terminal_kit::view::draw_multi_input;
use terminal_kit::view::draw_single_input;
use terminal_kit::view::first_visible as first_shown;
use terminal_kit::view::hint_lines;
use terminal_kit::view::scroll_offset;

use crate::tui::images::small_avatar;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect, Size};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use std::collections::HashMap;

use crate::api::types::{Embed, Media, Post, Profile, RefPost, ReplyContext};
use crate::api::{MAX_POST_BYTES, MAX_POST_GRAPHEMES, grapheme_len, post_length_problem};
use crate::config::ColumnSource;
use crate::i18n::{self, n};
use crate::media;
use crate::terminal::protocol_name;
use crate::tui::app::{
    App, Compose, EditProfile, List, LoginForm, Overlay, SearchMode, SettingEdit, SettingRow, Tab,
    ThreadView,
};
use crate::tui::columns::Rows;
use crate::tui::files::{Browser, EntryKind};
use crate::tui::images::Images;
use crate::tui::input::TextInput;
use crate::tui::keys;
use crate::tui::player::State;
use crate::tui::text::{drawable, format_time, truncate, wrap};
use crate::tui::theme::{THEMES, Theme};
use crate::tui::thread::{MAX_INDENT, RowKind, ThreadRow};
use crate::tui::worker::NotifItem;
use crate::video::format_seconds;

mod compose;
mod overlays;
mod posts;
#[cfg(test)]
mod state_fuzz;
mod tabs;
#[cfg(test)]
mod tests;

use compose::*;
use overlays::*;
use posts::*;
use tabs::*;

/// Width of the selection marker column.
const MARK_W: u16 = 2;
/// Avatar size in the lists, in cells.
const AVATAR: (u16, u16) = (4, 2);
/// Rows for a post's embedded images when the AppView gives no aspect ratio.
const IMAGE_ROWS: u16 = 8;
/// Bounds on the rows a post's embedded images take.
const IMAGE_ROWS_MIN: u16 = 3;
const IMAGE_ROWS_MAX: u16 = 12;
/// Widest a single embedded image may be, in cells.
const IMAGE_MAX_W: u16 = 36;
/// Avatar size on the profile tab, in cells.
const BIG_AVATAR: (u16, u16) = (12, 6);
/// Posts below the screen whose pictures are fetched and encoded ahead, so
/// they are ready when scrolled to.
const PREFETCH: usize = 4;
/// Posts (or rows) further on whose pictures are only downloaded ahead.
const WARM: usize = 20;

/// The key hints are one row: the keys that act on a post are behind `.`,
/// so the row stays short enough to read at a glance instead of becoming a
/// wall of text that hides the posts. A language whose words are longer
/// than English's may take a second row rather than lose a key such as
/// `esc` or `q` off its end.
const MAX_HINT_ROWS: u16 = 2;
/// Themes the picker shows at once; the rest scroll.
const THEME_ROWS: usize = 10;
/// Widest the error box gets, in cells.
const ERROR_W: u16 = 76;
/// Widest the actions list gets, in cells.
const ACTIONS_W: u16 = 44;
/// Widest the settings screen gets, in cells: room for a long path.
const SETTINGS_W: u16 = 72;
/// Widest the help box gets, in cells, and the column its keys take.
const HELP_W: u16 = 64;
#[cfg(test)]
const HELP_KEYS: usize = terminal_kit::view::HELP_KEYS;
/// Size of a picture's thumbnail in the composer, in cells.
const THUMB: (u16, u16) = (14, 5);
/// Smallest terminal the client draws in, in cells. The rows are the tab
/// bar, the key hints, the status line, and one whole post under them; the
/// columns are the selection marker, an avatar, and enough of a post to
/// read. Below either bound the screen would be shreds of all of them with
/// nothing to say why, so it says why instead.
const MIN_W: u16 = 24;
const MIN_H: u16 = 8;

/// Draw the whole UI.
pub(crate) fn draw(frame: &mut Frame, app: &mut App, images: &mut Images) {
    draw_screen(frame, app, images);
}

fn draw_screen(frame: &mut Frame, app: &mut App, images: &mut Images) {
    let area = frame.area();
    let t = app.theme;
    images.begin_frame(Size::new(area.width, area.height), t.dim());
    // Every cell starts in the theme's colors, so a theme with its own
    // background covers the whole terminal, not only the cells with text.
    frame.render_widget(Block::new().style(t.base()), area);
    if area.width < MIN_W || area.height < MIN_H {
        // Nothing of the client is drawn, so a video would decode pictures
        // that never reach the screen.
        images.stop_video();
        draw_too_small(frame, area, &t);
        return;
    }
    if let Some(form) = &app.login {
        draw_login(frame, area, form, &t);
        return;
    }
    // The key hints wrap onto more rows on a narrow screen rather than
    // being cut off.
    let hint_lines = hint_lines(&keys::hints(app), area.width, &t);
    // Read before the overlay is borrowed below: the list comes from the
    // whole app.
    let actions = keys::actions(app);
    let column_titles: Vec<String> = if matches!(app.overlay, Some(Overlay::AddColumn { .. })) {
        app.column_choices()
            .iter()
            .map(|s| match s {
                ColumnSource::Search { .. } => i18n::t("Search…").to_string(),
                ColumnSource::Author { handle, .. } => i18n::tf("Your posts (@{})", &[handle]),
                ColumnSource::Feed { name, .. } => i18n::tf("Feed: {}", &[name]),
                other => other.title(),
            })
            .collect()
    } else {
        Vec::new()
    };
    let settings = if matches!(app.overlay, Some(Overlay::Settings { .. })) {
        app.settings_rows()
    } else {
        Vec::new()
    };
    let hint_h = (hint_lines.len() as u16).clamp(1, MAX_HINT_ROWS);
    let [top, body, hint_row, status_row] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(hint_h),
        Constraint::Length(1),
    ])
    .areas(area);
    draw_tabs(frame, top, app);
    let inline = app.inline_video() && app.overlay.is_none();
    if let Some(th) = app.threads.last_mut() {
        let me = app.session.as_ref().map(|s| s.did.as_str());
        draw_thread(frame, body, th, images, PostContext { me, inline }, &t);
    } else if matches!(app.overlay, Some(Overlay::Search)) {
        // The box's hints can change the height available to the list. Do
        // not let drawing underneath it move the saved scroll position.
        draw_tab(frame, body, &mut app.clone(), images, &t);
    } else {
        draw_tab(frame, body, app, images, &t);
    }
    frame.render_widget(Paragraph::new(hint_lines), hint_row);
    draw_status(frame, status_row, app, images);
    match &mut app.overlay {
        Some(Overlay::Search) => draw_search_box(frame, body, &app.search, &t),
        Some(Overlay::Compose(c)) => {
            draw_compose(frame, area, c, images, &t);
            if let Some(b) = &mut c.browser {
                draw_browser(frame, area, b, images, &t);
            }
        }
        Some(Overlay::EditProfile(e)) => {
            draw_edit_profile(frame, area, e, &t);
            if let Some(b) = &mut e.browser {
                draw_browser(frame, area, b, images, &t);
            }
        }
        Some(Overlay::Help { scroll }) => draw_help(frame, area, scroll, app.pictures, &t),
        Some(Overlay::Actions { selected, .. }) => {
            draw_actions(frame, area, &actions, *selected, &t)
        }
        Some(Overlay::AddColumn { selected, query }) => {
            draw_add_column(frame, area, &column_titles, *selected, query.as_ref(), &t);
        }
        Some(Overlay::Accounts { selected }) => {
            let me = app.session.as_ref().map(|s| s.did.clone());
            draw_account_list(frame, area, &app.accounts, *selected, me.as_deref(), &t)
        }
        Some(Overlay::Languages { selected }) => {
            draw_languages(frame, area, *selected, i18n::current(), &t);
        }
        Some(Overlay::Settings { selected, edit }) => {
            let typing = match edit {
                Some(SettingEdit::Text(input)) => Some(&*input),
                _ => None,
            };
            draw_settings(frame, area, &settings, *selected, typing, &t);
            if let Some(SettingEdit::Folder(b)) = edit {
                draw_browser(frame, area, b.as_mut(), images, &t);
            }
        }
        Some(Overlay::Themes { selected, .. }) => draw_themes(frame, area, *selected, &t),
        Some(Overlay::Viewer {
            media,
            index,
            replay,
        }) => {
            let body = Rect {
                height: area.height.saturating_sub(hint_h + 1),
                ..area
            };
            draw_viewer(frame, body, media, *index, *replay, images, &t)
        }
        None => {}
    }
    let viewer_video = matches!(
        &app.overlay,
        Some(Overlay::Viewer { media, index, .. })
            if matches!(media.get(*index), Some(Media::Video { .. }))
    );
    // With nothing over the list, the selected post's video plays in its
    // picture box (posts::draw_posts); moving off it stops it here.
    let inline_playing = app.inline_video()
        && app.overlay.is_none()
        && app.selected_post().is_some_and(|p| {
            p.embed
                .as_ref()
                .is_some_and(|e| e.media().iter().any(|m| matches!(m, Media::Video { .. })))
        });
    if !viewer_video && !inline_playing {
        images.stop_video();
    }
    if let Some(s) = app.status.as_ref().filter(|s| s.error && s.popup) {
        draw_error(frame, area, &s.text, &t);
    }
}

/// The largest box of cells a `w` x `h` picture fits in within `area`, at
/// its own shape, centered.
fn fit(area: Rect, (w, h): (u32, u32), (cw, ch): (u16, u16)) -> Rect {
    let (cw, ch) = (f64::from(cw.max(1)), f64::from(ch.max(1)));
    let (aw, ah) = (f64::from(area.width) * cw, f64::from(area.height) * ch);
    let scale = (aw / f64::from(w.max(1))).min(ah / f64::from(h.max(1)));
    // At least a cell when there is one, never more than the area.
    let width = ((f64::from(w) * scale / cw).floor() as u16).clamp(area.width.min(1), area.width);
    let height =
        ((f64::from(h) * scale / ch).floor() as u16).clamp(area.height.min(1), area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

/// The screen for a terminal too small for the client: what is wrong, and
/// the size to reach. The numbers are there so the window can be dragged
/// until they meet, rather than guessed at.
fn draw_too_small(frame: &mut Frame, area: Rect, t: &Theme) {
    let w = usize::from(area.width).max(1);
    let mut lines: Vec<Line> = wrap(i18n::t("Terminal too small"), w)
        .into_iter()
        .map(|l| Line::styled(l, t.accent().bold()).centered())
        .collect();
    let size = i18n::tf(
        "{} needed, now {}",
        &[
            &format!("{MIN_W}x{MIN_H}"),
            &format!("{}x{}", area.width, area.height),
        ],
    );
    lines.extend(wrap(&size, w).into_iter().map(|l| Line::raw(l).centered()));
    let h = lines.len() as u16;
    frame.render_widget(
        Paragraph::new(lines),
        Rect {
            y: area.y + area.height.saturating_sub(h) / 2,
            height: h.min(area.height),
            ..area
        },
    );
}

/// An error in a box in the middle of the screen, over everything, where it
/// is seen. It goes with the next key, or by itself after a while.
fn draw_error(frame: &mut Frame, area: Rect, text: &str, t: &Theme) {
    let w = ERROR_W.min(area.width.saturating_sub(4)).max(10);
    let inner_w = usize::from(w.saturating_sub(4)).max(1);
    let mut lines: Vec<Line> = wrap(i18n::t(text), inner_w)
        .into_iter()
        .map(|l| Line::styled(l, t.error()))
        .collect();
    lines.push(Line::default());
    lines.push(Line::styled(i18n::t("any key closes this"), t.dim()));
    let h = lines.len() as u16 + 2;
    let r = terminal_kit::view::centered_rect(area, w, h);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(format!(" {} ", i18n::t("Error")))
        .border_style(t.error())
        .style(t.base());
    let inner = terminal_kit::view::draw_panel(frame, r, block, None);
    frame.render_widget(
        Paragraph::new(lines),
        Rect {
            x: inner.x + 1,
            width: inner.width.saturating_sub(2),
            ..inner
        },
    );
}

fn draw_tab(frame: &mut Frame, body: Rect, app: &mut App, images: &mut Images, t: &Theme) {
    let t = *t;
    match app.tab {
        Tab::Timeline if app.search.active.is_some() => {
            draw_search_results(frame, body, app, images)
        }
        Tab::Timeline => draw_timeline(frame, body, app, images, &t),
        Tab::Profile => draw_profile(frame, body, app, images),
        Tab::Notifications => draw_notifications(frame, body, app, images),
        Tab::Columns => draw_columns(frame, body, app, images, &t),
        Tab::Chat => draw_chat(frame, body, app, images, &t),
    }
}

fn draw_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme.clone();
    // No name of the app: the tabs start at the edge, and the room is theirs.
    let mut spans = Vec::new();
    for (i, tab) in Tab::ALL.iter().enumerate() {
        let title = if *tab == Tab::Timeline && app.search.active.is_some() {
            i18n::tf("Search: {}", &[&drawable(app.search_query())])
        } else {
            i18n::t(tab.title()).to_string()
        };
        let label = match (tab, app.unread) {
            (Tab::Notifications, n) if n > 0 => format!(" {} {title} ({n}) ", i + 1),
            (Tab::Chat, _) if app.chat.unread() > 0 => {
                format!(" {} {title} ({}) ", i + 1, app.chat.unread())
            }
            _ => format!(" {} {title} ", i + 1),
        };
        let style = if *tab == app.tab.shown_as() {
            t.selected()
        } else {
            t.dim()
        };
        spans.push(Span::styled(label, style));
    }
    let right = if app.accounts.len() > 1 {
        app.session
            .as_ref()
            .map(|s| Span::styled(format!("@{} ", s.handle), t.accent()))
    } else {
        None
    };
    frame.render_widget(
        terminal_kit::view::tab_line(spans, right, area.width, 1),
        area,
    );
}

/// The last message on the left; activity or the image protocol on the right.
fn draw_status(frame: &mut Frame, area: Rect, app: &App, images: &Images) {
    let t = &app.theme.clone();
    let right = if app.pending > 0 || images.loading() {
        format!("{} ", i18n::t("loading…"))
    } else {
        match images.shows() {
            true => format!("{} ", protocol_name(images.protocol_type())),
            false => format!("{} ", i18n::t("text")),
        }
    };
    let left = app
        .status
        .as_ref()
        .filter(|s| !s.error || !s.popup)
        .map(|s| Span::styled(i18n::t(&s.text), if s.error { t.error() } else { t.ok() }));
    terminal_kit::view::draw_status_line(frame, area, left, Span::styled(right, t.dim()));
}

/// `text` wrapped to `width` cells by the client's own rule. Ratatui's
/// wrapping lets a wide character start in the last column, which draws it
/// half off the edge and loses the words after it on that line.
fn wrapped(text: &str, width: u16) -> Paragraph<'static> {
    Paragraph::new(
        wrap(text, usize::from(width))
            .into_iter()
            .map(Line::from)
            .collect::<Vec<_>>(),
    )
}

/// What an empty list says: why it failed to load, or that it is empty.
fn empty_message<T>(frame: &mut Frame, area: Rect, list: &List<T>, empty: &str, t: &Theme) {
    let p = match &list.error {
        Some(e) => {
            wrapped(&format!(" {e}  {}", i18n::t("(R to retry)")), area.width).style(t.error())
        }
        None => Paragraph::new(format!(" {}", i18n::t(empty.trim_start()))).style(t.dim()),
    };
    frame.render_widget(p, area);
}

/// A box of `w` x `h` centered in `area`, cleared.
fn popup(frame: &mut Frame, area: Rect, w: u16, h: u16, title: &str, t: &Theme) -> Rect {
    let r = terminal_kit::view::centered_rect(area, w, h);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .style(t.base())
        .border_style(t.accent())
        .title(format!(" {} ", i18n::t(title)));
    terminal_kit::view::draw_panel(frame, r, block, Some(area.x))
}

/// A byte count the way a person reads it.
fn human_bytes(n: u64) -> String {
    match n {
        n if n >= 1024 * 1024 => format!("{:.1} MB", n as f64 / (1024.0 * 1024.0)),
        n if n >= 1024 => format!("{} KB", n / 1024),
        n => format!("{n} B"),
    }
}

use crate::tui::text::truncate_start;
