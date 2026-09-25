//! Drawing. Every function here reads the [`App`] and writes to a frame;
//! the only state it changes is each list's scroll offset.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect, Size};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use std::collections::HashMap;

use unicode_width::UnicodeWidthStr;

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

/// The small version of a Bluesky avatar: a list draws avatars 4 cells
/// wide, and the full-size one is many times the bytes. Other URLs are kept.
fn small_avatar(url: &str) -> std::borrow::Cow<'_, str> {
    if url.contains("/img/avatar/") {
        url.replacen("/img/avatar/", "/img/avatar_thumbnail/", 1)
            .into()
    } else {
        url.into()
    }
}
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
const HELP_KEYS: usize = 18;
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
pub fn draw(frame: &mut Frame, app: &mut App, images: &mut Images) {
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
    if let Some(th) = app.threads.last_mut() {
        let me = app.session.as_ref().map(|s| s.did.as_str());
        draw_thread(frame, body, th, images, me, &t);
    } else {
        draw_tab(frame, body, app, images, &t);
    }
    frame.render_widget(Paragraph::new(hint_lines), hint_row);
    draw_status(frame, status_row, app, images);
    match &mut app.overlay {
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
    if !matches!(
        &app.overlay,
        Some(Overlay::Viewer { media, index, .. })
            if matches!(media.get(*index), Some(Media::Video { .. }))
    ) {
        images.stop_video();
    }
    if let Some(s) = app.status.as_ref().filter(|s| s.error) {
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
    let r = Rect {
        x: area.x + area.width.saturating_sub(w) / 2,
        y: area.y + area.height.saturating_sub(h) / 2,
        width: w.min(area.width),
        height: h.min(area.height),
    };
    frame.render_widget(Clear, r);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(format!(" {} ", i18n::t("Error")))
        .border_style(t.error())
        .style(t.base());
    let inner = block.inner(r);
    frame.render_widget(block, r);
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
        Tab::Timeline => draw_timeline(frame, body, app, images, &t),
        Tab::Search => draw_search(frame, body, app, images),
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
        let title = i18n::t(tab.title());
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
        spans.push(Span::raw(" "));
    }
    // With several accounts, the one in use is named at the right, where
    // it fits whole; with one it is on the Profile tab, and nowhere else.
    let used: usize = spans
        .iter()
        .map(|s| crate::tui::text::cells(&s.content))
        .sum();
    if app.accounts.len() > 1
        && let Some(s) = &app.session
    {
        let name = format!("@{} ", s.handle);
        let room = usize::from(area.width).saturating_sub(used);
        let name_w = crate::tui::text::cells(&name);
        if name_w < room {
            spans.push(Span::raw(" ".repeat(room - name_w)));
            spans.push(Span::styled(name, t.accent()));
        }
    }
    frame.render_widget(Line::from(spans), area);
}

/// The keys that work in the current view, packed into as many rows of
/// `width` as they need; a hint is never split across rows.
fn hint_lines(hints: &[keys::Hint], width: u16, t: &Theme) -> Vec<Line<'static>> {
    let width = usize::from(width.max(1));
    let mut lines: Vec<Vec<Span<'static>>> = vec![vec![Span::raw(" ")]];
    let mut used = 1;
    for (key, what) in hints {
        let w = crate::tui::text::cells(key) + 1 + crate::tui::text::cells(what);
        let row = lines.last_mut().expect("one row");
        let first = row.len() == 1;
        let gap = if first { 0 } else { 2 };
        if !first && used + gap + w > width {
            lines.push(vec![Span::raw(" ")]);
            used = 1;
        } else if !first {
            row.push(Span::raw("  "));
            used += 2;
        }
        let row = lines.last_mut().expect("one row");
        row.push(Span::styled(*key, t.accent().bold()));
        row.push(Span::styled(format!(" {what}"), t.dim()));
        used += w;
    }
    lines
        .into_iter()
        .map(|spans| truncate_line(Line::from(spans), width))
        .collect()
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
    let w = crate::tui::text::cells(&right) as u16;
    // Errors are drawn in the middle of the screen (draw_error); the row
    // keeps the passing news, cut short of what is on the right.
    if let Some(s) = app.status.as_ref().filter(|s| !s.error) {
        let room = usize::from(area.width.saturating_sub(w + 2));
        frame.render_widget(
            Line::from(Span::styled(
                format!(" {}", truncate(i18n::t(&s.text), room)),
                t.ok(),
            )),
            area,
        );
    }
    if w < area.width {
        frame.render_widget(
            Paragraph::new(right).style(t.dim()),
            Rect {
                x: area.right() - w,
                width: w,
                ..area
            },
        );
    }
}

/// The first item to draw so that `selected` is on screen: `offset` when it
/// already is, otherwise the lowest start that still fits the selection.
/// Only the items between the two are measured, so the cost is what one
/// screen holds, however long the list and however far the jump.
fn scroll_offset(
    selected: usize,
    offset: usize,
    len: usize,
    viewport: u16,
    mut height: impl FnMut(usize) -> u16,
) -> usize {
    let viewport = u32::from(viewport);
    let mut top = if selected <= offset {
        selected
    } else {
        let mut used = u32::from(height(selected));
        let mut top = selected;
        while top > offset {
            let h = u32::from(height(top - 1));
            if used + h > viewport {
                break;
            }
            used += h;
            top -= 1;
        }
        top
    };
    // When the list ends on screen with room to spare (the terminal grew, or
    // the selection is near the end), the room goes to the posts above
    // rather than staying empty. Only the rows that fit are measured.
    let mut used = 0;
    for i in top..len {
        used += u32::from(height(i));
        if used >= viewport {
            return top;
        }
    }
    while top > 0 {
        let h = u32::from(height(top - 1));
        if used + h > viewport {
            break;
        }
        used += h;
        top -= 1;
    }
    top
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

/// The first of a list's entries to draw in `rows` rows so the selected one
/// is among them: a list over the screen taller than its box scrolls with
/// its selection, or Enter would act on an entry that is not shown.
fn first_shown(selected: usize, rows: u16) -> usize {
    (selected + 1).saturating_sub(usize::from(rows.max(1)))
}

/// A box of `w` x `h` centered in `area`, cleared.
fn popup(frame: &mut Frame, area: Rect, w: u16, h: u16, title: &str, t: &Theme) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    let r = Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    };
    frame.render_widget(Clear, r);
    // A wide character just left of the box has its second half under the
    // left border, and a terminal draws it over the border: it goes.
    if r.x > area.x {
        let buf = frame.buffer_mut();
        for y in r.y..r.bottom() {
            let cell = &mut buf[(r.x - 1, y)];
            if cell.symbol().width() > 1 {
                cell.set_symbol(" ");
            }
        }
    }
    // Clear resets the cells to the terminal's colors; the block's style
    // paints them back in the theme's.
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .style(t.base())
        .border_style(t.accent())
        .title(format!(" {} ", i18n::t(title)));
    let inner = block.inner(r);
    frame.render_widget(block, r);
    inner
}

fn draw_single_input(frame: &mut Frame, area: Rect, input: &TextInput, focused: bool) {
    let width = usize::from(area.width.max(1));
    let layout = input.layout(width);
    // Show the visual line holding the cursor, so long input scrolls.
    let row = layout.cursor.0.min(layout.lines.len().saturating_sub(1));
    let line = layout.lines.get(row).cloned().unwrap_or_default();
    frame.render_widget(Paragraph::new(line), area);
    if focused {
        frame.set_cursor_position(Position::new(area.x + layout.cursor.1 as u16, area.y));
    }
}

fn draw_multi_input(frame: &mut Frame, area: Rect, input: &TextInput, focused: bool) {
    let layout = input.layout(usize::from(area.width.max(1)));
    let h = usize::from(area.height.max(1));
    let first = layout.cursor.0.saturating_sub(h - 1);
    let shown: Vec<Line> = layout
        .lines
        .iter()
        .skip(first)
        .take(h)
        .map(|l| Line::from(l.clone()))
        .collect();
    frame.render_widget(Paragraph::new(shown), area);
    if focused {
        let y = area.y + (layout.cursor.0 - first) as u16;
        frame.set_cursor_position(Position::new(area.x + layout.cursor.1 as u16, y));
    }
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
