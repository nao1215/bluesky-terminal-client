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
use crate::media;
use crate::terminal::protocol_name;
use crate::tui::app::{
    App, Compose, EditProfile, List, LoginForm, Overlay, SearchMode, SettingEdit, SettingRow, Tab,
    ThreadView,
};
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
/// wall of text that hides the posts.
const MAX_HINT_ROWS: u16 = 1;
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
        Some(Overlay::Actions { selected }) => draw_actions(frame, area, &actions, *selected, &t),
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
    let width = ((f64::from(w) * scale / cw).floor() as u16).clamp(1, area.width.max(1));
    let height = ((f64::from(h) * scale / ch).floor() as u16).clamp(1, area.height.max(1));
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

/// A post's pictures (or video) full screen, the one at `index` shown at its
/// own shape, centered, with its number and alt text under it.
fn draw_viewer(
    frame: &mut Frame,
    area: Rect,
    media: &[Media],
    index: usize,
    replay: u32,
    images: &mut Images,
    t: &Theme,
) {
    frame.render_widget(Clear, area);
    frame.render_widget(Block::new().style(t.base()), area);
    let Some(item) = media.get(index) else {
        return;
    };
    let [pic, caption] = Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(area);
    let (url, alt, aspect) = match item {
        Media::Image {
            url, alt, aspect, ..
        } => (Some(url.as_str()), alt, *aspect),
        Media::Video {
            thumbnail,
            alt,
            aspect,
            ..
        } => (thumbnail.as_deref(), alt, *aspect),
    };
    let shape = url.and_then(|u| images.dims(u)).or(aspect);
    let r = match shape {
        Some(d) => fit(pic, d, images.cell_size()),
        None => pic,
    };
    let mut head = vec![Span::styled(
        format!(" {}/{}", index + 1, media.len()),
        t.accent().bold(),
    )];
    match item {
        Media::Video { playlist, .. } => {
            // The thumbnail until the first picture arrives, and if the
            // video cannot be played at all.
            let state = images.draw_video(frame, r, playlist, replay);
            if !images.video_shown()
                && let Some(url) = url
            {
                images.draw(frame, r, url);
            }
            let (text, style) = match state {
                State::Loading => ("  loading the video…".to_string(), t.dim()),
                State::Playing => ("  ▶ playing (no sound)".to_string(), t.accent()),
                State::Ended => ("  ■ ended  r plays it again".to_string(), t.dim()),
                State::Warning(why) => (format!("  ⚠ {why}; showing its thumbnail"), t.error()),
            };
            head.push(Span::styled(text, style));
        }
        Media::Image { url, thumb, .. } => {
            // The thumbnail at once, the full size as soon as it is here.
            images.draw_first(frame, r, &[url.as_str(), thumb.as_str()]);
        }
    }
    let alt = if alt.is_empty() {
        Line::styled(" (no alt text)", t.dim())
    } else {
        Line::from(format!(
            " {}",
            truncate(alt, usize::from(caption.width.saturating_sub(2)))
        ))
    };
    frame.render_widget(Paragraph::new(vec![Line::from(head), alt]), caption);
}

/// The list `.` opens: every key of this view that acts on the selected
/// post, with what it would do now beside it.
fn draw_actions(frame: &mut Frame, area: Rect, entries: &[keys::Hint], selected: usize, t: &Theme) {
    let inner = popup(
        frame,
        area,
        ACTIONS_W,
        entries.len() as u16 + 2,
        "Actions",
        t,
    );
    let lines: Vec<Line> = entries
        .iter()
        .enumerate()
        .map(|(i, (key, what))| {
            let marker = if i == selected { "▶ " } else { "  " };
            let key = Span::styled(format!("{marker}{key:<7}"), t.accent().bold());
            let what = if i == selected {
                Span::styled(*what, t.base().bold())
            } else {
                Span::styled(*what, t.dim())
            };
            Line::from(vec![key, what])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

/// The settings screen: a row per setting, `name  value`, and under the
/// list what the selected one's value comes from or what Enter does, or the
/// line its new value is typed in.
fn draw_settings(
    frame: &mut Frame,
    area: Rect,
    rows: &[SettingRow],
    selected: usize,
    typing: Option<&TextInput>,
    t: &Theme,
) {
    const NAME_W: usize = 16;
    let inner = popup(
        frame,
        area,
        SETTINGS_W,
        rows.len() as u16 + 5,
        "Settings",
        t,
    );
    let width = usize::from(inner.width);
    // The note of the selected row, whole, below the list: two rows when
    // there is room, one on a short screen.
    let note_h = match inner.height {
        0..=2 => 0,
        3..=5 => 1,
        _ => 2,
    };
    let list_h = usize::from(inner.height.saturating_sub(note_h + u16::from(note_h > 0)));
    let top = (selected + 1).saturating_sub(list_h);
    let mut lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .skip(top)
        .take(list_h)
        .map(|(i, r)| {
            let marker = if i == selected { "▶ " } else { "  " };
            let name = truncate(r.name, NAME_W);
            let head = format!("{marker}{name:<NAME_W$} ");
            let room = width.saturating_sub(head.width());
            // A path is cut at its start: its end names the folder.
            let value = truncate_start(&r.value, room);
            let style = if i == selected {
                t.base().bold()
            } else {
                t.dim()
            };
            Line::from(vec![
                Span::styled(head, t.accent().bold()),
                Span::styled(value, style),
            ])
        })
        .collect();
    if note_h > 0
        && let Some(input) = typing
    {
        lines.push(Line::raw(""));
        if note_h > 1 {
            lines.push(Line::styled(
                " enter keeps it, empty is the default, esc cancels",
                t.dim(),
            ));
        }
        frame.render_widget(Paragraph::new(lines), inner);
        let field = Rect {
            x: inner.x + 1,
            y: inner.bottom().saturating_sub(1),
            width: inner.width.saturating_sub(2),
            height: 1,
        };
        draw_single_input(frame, field, input, true);
        return;
    }
    if note_h > 0
        && let Some(r) = rows.get(selected)
    {
        lines.push(Line::raw(""));
        let note: Vec<String> = wrap(&r.note, width.saturating_sub(1).max(1))
            .into_iter()
            .take(usize::from(note_h))
            .collect();
        lines.extend(
            note.into_iter()
                .map(|l| Line::styled(format!(" {l}"), t.dim())),
        );
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

/// The screen for a terminal too small for the client: what is wrong, and
/// the size to reach. The numbers are there so the window can be dragged
/// until they meet, rather than guessed at.
fn draw_too_small(frame: &mut Frame, area: Rect, t: &Theme) {
    let w = usize::from(area.width).max(1);
    let mut lines: Vec<Line> = wrap("Terminal too small", w)
        .into_iter()
        .map(|l| Line::styled(l, t.accent().bold()).centered())
        .collect();
    let size = format!("{MIN_W}x{MIN_H} needed, now {}x{}", area.width, area.height);
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
    let mut lines: Vec<Line> = wrap(text, inner_w)
        .into_iter()
        .map(|l| Line::styled(l, t.error()))
        .collect();
    lines.push(Line::default());
    lines.push(Line::styled("any key closes this", t.dim()));
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
        .title(" Error ")
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
    }
}

/// The Timeline tab: the following timeline or a pinned feed, with a row
/// naming them all when there are feeds to choose from.
fn draw_timeline(frame: &mut Frame, body: Rect, app: &mut App, images: &mut Images, t: &Theme) {
    let area = if app.feeds.is_empty() {
        body
    } else {
        let [bar, rest] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(body);
        let names: Vec<&str> = std::iter::once("Following")
            .chain(app.feeds.iter().map(|f| f.info.name.as_str()))
            .collect();
        frame.render_widget(feed_bar(&names, app.feed, usize::from(bar.width), t), bar);
        rest
    };
    let empty = if app.feed == 0 {
        "No posts from accounts you follow yet. Press R to refresh."
    } else {
        "No posts in this feed yet. Press R to refresh."
    };
    draw_posts(frame, area, app.feed_list(), images, empty, None, t);
}

/// The feeds to choose from, the shown one highlighted, with `[ ] feeds`
/// at the end. When they do not all fit, the row starts as far left as it
/// can with the shown one in sight, and an ellipsis marks each side where
/// feeds are left out.
fn feed_bar(names: &[&str], shown: usize, width: usize, t: &Theme) -> Line<'static> {
    const HINT: &str = "  [ ] feeds";
    let room = width.saturating_sub(HINT.width() + 4);
    // A name is cut to fit what room there is, so the shown one always does.
    let max = room.saturating_sub(3).clamp(4, 24);
    let labels: Vec<String> = names.iter().map(|n| truncate(n, max)).collect();
    // " name " and the space after it.
    let cost = |i: usize| labels[i].width() + 3;
    let span = |from: usize, to: usize| (from..=to).map(cost).sum::<usize>();
    let from = (0..=shown)
        .find(|&f| span(f, shown) <= room)
        .unwrap_or(shown);
    let mut to = shown;
    while to + 1 < labels.len() && span(from, to + 1) <= room {
        to += 1;
    }
    let mut spans = vec![Span::raw(if from > 0 { "…" } else { " " })];
    for (i, label) in labels.iter().enumerate().take(to + 1).skip(from) {
        let style = if i == shown { t.selected() } else { t.dim() };
        spans.push(Span::styled(format!(" {label} "), style));
        spans.push(Span::raw(" "));
    }
    if to + 1 < labels.len() {
        spans.push(Span::raw("…"));
    }
    spans.push(Span::styled(HINT, t.dim()));
    truncate_line(Line::from(spans), width)
}

/// A thread over the current tab: a title row, then the posts, the opened
/// one among them where the selection starts.
fn draw_thread(
    frame: &mut Frame,
    area: Rect,
    th: &mut ThreadView,
    images: &mut Images,
    me: Option<&str>,
    t: &Theme,
) {
    let [title, list] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
    frame.render_widget(
        Line::from(vec![
            Span::styled(" Thread ", t.selected()),
            Span::styled("  esc back", t.dim()),
        ]),
        title,
    );
    if let Some(e) = &th.error {
        frame.render_widget(
            Paragraph::new(format!(" could not load the thread: {e}  (R to retry)"))
                .style(t.error())
                .wrap(ratatui::widgets::Wrap { trim: true }),
            list,
        );
        return;
    }
    draw_posts(
        frame,
        list,
        &mut th.list,
        images,
        "The thread is empty.",
        me,
        t,
    );
}

fn draw_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme.clone();
    // No name of the app: the tabs start at the edge, and the room is theirs.
    let mut spans = Vec::new();
    for (i, tab) in Tab::ALL.iter().enumerate() {
        let label = match (tab, app.unread) {
            (Tab::Notifications, n) if n > 0 => format!(" {} {} ({n}) ", i + 1, tab.title()),
            _ => format!(" {} {} ", i + 1, tab.title()),
        };
        let style = if *tab == app.tab {
            t.selected()
        } else {
            t.dim()
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }
    // The account's handle is on the Profile tab; up here a long one would
    // be cut off.
    frame.render_widget(Line::from(spans), area);
}

/// The keys that work in the current view, packed into as many rows of
/// `width` as they need; a hint is never split across rows.
fn hint_lines(hints: &[keys::Hint], width: u16, t: &Theme) -> Vec<Line<'static>> {
    let width = usize::from(width.max(1));
    let mut lines: Vec<Vec<Span<'static>>> = vec![vec![Span::raw(" ")]];
    let mut used = 1;
    for (key, what) in hints {
        let w = key.width() + 1 + what.width();
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
        "loading… ".to_string()
    } else {
        match images.shows() {
            true => format!("{} ", protocol_name(images.protocol_type())),
            false => "text ".to_string(),
        }
    };
    // Errors are drawn in the middle of the screen (draw_error); the row
    // keeps the passing news.
    if let Some(s) = app.status.as_ref().filter(|s| !s.error) {
        frame.render_widget(
            Line::from(Span::styled(format!(" {}", s.text), t.ok())),
            area,
        );
    }
    let w = right.width() as u16;
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

/// What an empty list says: why it failed to load, or that it is empty.
fn empty_message<T>(frame: &mut Frame, area: Rect, list: &List<T>, empty: &str, t: &Theme) {
    let p = match &list.error {
        Some(e) => Paragraph::new(format!(" {e}  (R to retry)"))
            .style(t.error())
            .wrap(ratatui::widgets::Wrap { trim: true }),
        None => Paragraph::new(format!(" {}", empty.trim_start())).style(t.dim()),
    };
    frame.render_widget(p, area);
}

/// Everything a post draws besides images, precomputed for a width.
struct PostLines {
    /// The thread above a reply, one dim line per post.
    context: Vec<Line<'static>>,
    header: Line<'static>,
    body: Vec<Line<'static>>,
    images: Vec<String>,
    /// Rows the images take: enough for the tallest at its box width.
    image_rows: u16,
    /// Whether an avatar is drawn beside it, which the row must be tall
    /// enough for.
    avatar: bool,
    stats: Line<'static>,
}

/// Width in cells of each image box when `n` images share `width` columns.
fn image_box_width(width: u16, n: u16) -> u16 {
    if n == 0 || width < n {
        return 0;
    }
    ((width - (n - 1)) / n).min(IMAGE_MAX_W)
}

/// Rows an image of aspect `w:h` needs at `box_w` cells, given the cell size
/// in pixels, so a picture is drawn at its own shape instead of in a fixed
/// band with blank rows under it.
fn image_rows(aspect: Option<(u32, u32)>, box_w: u16, cell: (u16, u16)) -> u16 {
    let Some((w, h)) = aspect else {
        return IMAGE_ROWS;
    };
    let (cw, ch) = (u64::from(cell.0.max(1)), u64::from(cell.1.max(1)));
    let px_h = u64::from(box_w) * cw * u64::from(h) / u64::from(w);
    let rows = px_h.div_ceil(ch);
    rows.clamp(u64::from(IMAGE_ROWS_MIN), u64::from(IMAGE_ROWS_MAX)) as u16
}

impl PostLines {
    /// `me` is the viewer's DID when the post should say whether its author
    /// is followed (a search, a thread); `None` where that goes without
    /// saying (the timeline is only followed accounts, a profile says it once).
    /// `cell` is the terminal's cell size in pixels, or `None` when it
    /// shows no pictures: then a line says what the post carries instead.
    fn new(post: &Post, width: u16, cell: Option<(u16, u16)>, me: Option<&str>, t: &Theme) -> Self {
        let width = usize::from(width.max(1));
        let record = post.record();
        let time = format_time(record.created_at.as_deref().unwrap_or(&post.indexed_at));
        let mut header = vec![
            Span::styled(post.author.name().to_string(), Style::new().bold()),
            Span::styled(format!(" @{}", post.author.handle), t.dim()),
        ];
        match me {
            Some(me) if me != post.author.did => {
                header.push(if post.author.following_uri().is_some() {
                    Span::styled(" ✓ following", t.accent())
                } else {
                    Span::styled(" not following", t.dim())
                })
            }
            _ => {}
        }
        header.push(Span::styled(format!(" · {time}"), t.dim()));
        if record.reply.is_some() {
            header.push(Span::styled(" ↩ reply", t.dim()));
        }
        let header = Line::from(header);
        let header = truncate_line(header, width);

        let mut body: Vec<Line<'static>> = wrap(&record.text, width)
            .into_iter()
            .map(Line::from)
            .collect();
        let mut images = Vec::new();
        let mut rows = 0;
        if let Some(embed) = &post.embed
            && cell.is_none()
        {
            // The media line names a video with its description.
            if !matches!(embed, Embed::Video { .. }) {
                body.extend(embed_lines(embed, width, t));
            }
            body.extend(media_line(embed, width, t));
        } else if let Some(embed) = &post.embed
            && let Some(cell) = cell
        {
            body.extend(embed_lines(embed, width, t));
            let shown: Vec<_> = embed.images().into_iter().take(4).collect();
            let box_w = image_box_width(width as u16, shown.len() as u16);
            rows = shown
                .iter()
                .map(|i| image_rows(i.aspect, box_w, cell))
                .max()
                .unwrap_or(0);
            images = shown.iter().map(|i| i.url.to_string()).collect();
        }
        let liked = post.like_uri().is_some();
        let heart = if liked { "♥" } else { "♡" };
        let heart_style = if liked { t.like() } else { t.dim() };
        let reposted = post.viewer.as_ref().is_some_and(|v| v.repost.is_some());
        let repost_style = if reposted { t.repost() } else { t.dim() };
        let stats = Line::from(vec![
            Span::styled(format!("{heart} {}", post.like_count), heart_style),
            Span::raw("   "),
            Span::styled(format!("⟳ {}", post.repost_count), repost_style),
            Span::styled(format!("   ↩ {}", post.reply_count), t.dim()),
        ]);
        let context = post
            .context
            .as_deref()
            .map(|c| context_lines(c, width, t))
            .unwrap_or_default();
        Self {
            context,
            header,
            body,
            images,
            image_rows: rows,
            avatar: cell.is_some(),
            stats,
        }
    }

    /// A row that stands in for a post that cannot be shown.
    fn placeholder(text: &'static str, t: &Theme) -> Self {
        Self {
            context: Vec::new(),
            header: Line::styled(text, t.dim()),
            body: Vec::new(),
            images: Vec::new(),
            image_rows: 0,
            avatar: false,
            stats: Line::default(),
        }
    }

    fn height(&self) -> u16 {
        let ctx = self.context.len() as u16;
        let content = ctx + 2 + self.body.len() as u16 + self.image_rows;
        let avatar = if self.avatar { ctx + AVATAR.1 } else { 0 };
        content.max(avatar) + 1
    }
}

/// The posts above a reply, each on one dim line: the root when the reply is
/// not directly under it, a gap when more of the thread sits in between, and
/// the parent.
fn context_lines(c: &ReplyContext, width: usize, t: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if let Some(root) = &c.root {
        lines.push(ref_post_line(root, width, t));
        if c.gap {
            lines.push(Line::styled("┆ ⋮", t.dim()));
        }
    }
    lines.push(ref_post_line(&c.parent, width, t));
    lines
}

fn ref_post_line(p: &RefPost, width: usize, t: &Theme) -> Line<'static> {
    let text = match p {
        RefPost::Post(post) => {
            let first = post.record().text.lines().next().unwrap_or("").to_string();
            format!("┆ {} @{}: {first}", post.author.name(), post.author.handle)
        }
        RefPost::NotFound { .. } => "┆ (post not found)".into(),
        RefPost::Blocked { .. } => "┆ (blocked post)".into(),
        RefPost::Other => "┆ (post not shown)".into(),
    };
    Line::styled(truncate(&text, width), t.dim())
}

/// `line` cut to `width` columns as one piece: whole grapheme clusters up
/// to the cut and one ellipsis there, in the style of the span it cuts.
fn truncate_line(line: Line<'static>, width: usize) -> Line<'static> {
    let spans: Vec<(String, Style)> = line
        .spans
        .into_iter()
        .map(|s| (drawable(&s.content).into_owned(), s.style))
        .collect();
    if spans.iter().map(|(t, _)| t.width()).sum::<usize>() <= width {
        return Line::from(
            spans
                .into_iter()
                .map(|(t, s)| Span::styled(t, s))
                .collect::<Vec<_>>(),
        );
    }
    // One column is kept for the ellipsis.
    let mut left = width.saturating_sub(1);
    let mut out = Vec::new();
    for (text, style) in spans {
        let mut kept = String::new();
        for g in unicode_segmentation::UnicodeSegmentation::graphemes(text.as_str(), true) {
            let gw = g.width();
            if gw > left {
                if width > 0 {
                    kept.push('…');
                }
                out.push(Span::styled(kept, style));
                return Line::from(out);
            }
            kept.push_str(g);
            left -= gw;
        }
        out.push(Span::styled(kept, style));
    }
    Line::from(out)
}

/// What a post carries, for a terminal that cannot show it: the number of
/// pictures and their descriptions, or the video's.
fn media_line(embed: &Embed, width: usize, t: &Theme) -> Option<Line<'static>> {
    let media = embed.media();
    let alts = |alt: &str| {
        let alt = alt.trim();
        (!alt.is_empty()).then(|| alt.to_string())
    };
    let text = match media.first()? {
        Media::Video { alt, .. } => match alts(alt) {
            Some(alt) => format!("▶ video: {alt}"),
            None => "▶ video".to_string(),
        },
        Media::Image { .. } => {
            let described: Vec<String> = media
                .iter()
                .filter_map(|m| match m {
                    Media::Image { alt, .. } => alts(alt),
                    Media::Video { .. } => None,
                })
                .collect();
            let n = media.len();
            let what = if n == 1 {
                "1 picture".to_string()
            } else {
                format!("{n} pictures")
            };
            if described.is_empty() {
                format!("▣ {what}")
            } else {
                format!("▣ {what}: {}", described.join(" · "))
            }
        }
    };
    Some(Line::styled(truncate(&text, width), t.dim()))
}

fn embed_lines(embed: &Embed, width: usize, t: &Theme) -> Vec<Line<'static>> {
    let dim = t.dim();
    match embed {
        Embed::External { external } => {
            let title = if external.title.is_empty() {
                &external.uri
            } else {
                &external.title
            };
            vec![
                Line::styled(truncate(&format!("🔗 {title}"), width), t.accent()),
                Line::styled(truncate(&external.uri, width), dim),
            ]
        }
        // A quote beside a picture is a recordWithMedia, and says whose
        // post it quotes just as a plain quote does.
        Embed::Record { .. } | Embed::RecordWithMedia { .. } => embed
            .quoted()
            .map(|r| quote_line(r, width, t))
            .into_iter()
            .collect(),
        Embed::Video { .. } => vec![Line::styled("▶ video", dim)],
        Embed::Images { .. } | Embed::Other => Vec::new(),
    }
}

/// The one line a quote gets: who is quoted and the start of what they
/// wrote. A quote whose post cannot be shown says why, since a line that is
/// simply missing reads as a post that quotes nothing.
fn quote_line(record: &serde_json::Value, width: usize, t: &Theme) -> Line<'static> {
    let kind = record
        .get("$type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let name = |p: &str| {
        record
            .pointer(p)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let text = match kind {
        "app.bsky.embed.record#viewNotFound" => "❝ quoted post not found".to_string(),
        "app.bsky.embed.record#viewBlocked" => {
            "❝ quoted post from an account you cannot see".to_string()
        }
        "app.bsky.embed.record#viewDetached" => "❝ quote removed by the post's author".to_string(),
        "app.bsky.feed.defs#generatorView" => format!("❝ feed: {}", name("/displayName")),
        "app.bsky.graph.defs#listView" => format!("❝ list: {}", name("/name")),
        _ => match record.pointer("/author/handle").and_then(|h| h.as_str()) {
            Some(handle) => {
                let said = name("/value/text");
                let first = said.lines().next().unwrap_or("");
                format!("❝ @{handle}: {first}")
            }
            None => "❝ quoted post cannot be shown".to_string(),
        },
    };
    Line::styled(truncate(&text, width), t.dim())
}

/// Width left for a row's text once its indentation is taken off.
fn content_width<T: PostRow>(content: Rect, row: &T) -> u16 {
    content.width.saturating_sub(row.indent() * 2)
}

/// The content column of a list row, right of the marker and avatar.
fn content_rect(area: Rect, avatars: bool) -> Rect {
    let left = if avatars {
        MARK_W + AVATAR.0 + 1
    } else {
        MARK_W + 1
    };
    Rect {
        x: area.x + left.min(area.width),
        width: area.width.saturating_sub(left),
        ..area
    }
}

fn draw_marker(frame: &mut Frame, area: Rect, height: u16, selected: bool, t: &Theme) {
    if !selected {
        return;
    }
    let h = height.saturating_sub(1).min(area.height).max(1);
    let bar = vec![Line::from("▌"); usize::from(h)];
    frame.render_widget(
        Paragraph::new(bar).style(t.accent()),
        Rect {
            width: 1,
            height: h,
            ..area
        },
    );
}

/// Something drawn as one entry of a post list: a feed item, or a row of a
/// thread (indented, possibly a placeholder for a post that cannot be shown).
pub trait PostRow {
    fn post(&self) -> Option<&Post>;
    /// Reply depth, drawn as indentation with guide lines.
    fn indent(&self) -> u16 {
        0
    }
    /// What to show instead of a post that is not there.
    fn placeholder(&self) -> &'static str {
        "(post not shown)"
    }
}

impl PostRow for Post {
    fn post(&self) -> Option<&Post> {
        Some(self)
    }
}

impl PostRow for ThreadRow {
    fn post(&self) -> Option<&Post> {
        ThreadRow::post(self)
    }
    fn indent(&self) -> u16 {
        self.depth.min(MAX_INDENT)
    }
    fn placeholder(&self) -> &'static str {
        match self.kind {
            RowKind::Blocked(_) => "(blocked post)",
            _ => "(post not found)",
        }
    }
}

fn row_lines<T: PostRow>(
    row: &T,
    width: u16,
    cell: Option<(u16, u16)>,
    me: Option<&str>,
    t: &Theme,
) -> PostLines {
    match row.post() {
        Some(post) => PostLines::new(post, width, cell, me, t),
        None => PostLines::placeholder(row.placeholder(), t),
    }
}

fn draw_posts<T: PostRow>(
    frame: &mut Frame,
    area: Rect,
    list: &mut List<T>,
    images: &mut Images,
    empty: &str,
    me: Option<&str>,
    t: &Theme,
) {
    if !list.loaded {
        frame.render_widget(Paragraph::new(" loading…").style(t.dim()), area);
        return;
    }
    if list.items.is_empty() {
        empty_message(frame, area, list, empty, t);
        return;
    }
    let content = content_rect(area, images.shows());
    let cell = images.shows().then(|| images.cell_size());
    // Lay out only what this frame can need: the posts above the selection
    // that fit with it on screen, then as many as fill the screen. A long,
    // paged list costs no more than a short one.
    let mut lines: HashMap<usize, PostLines> = HashMap::new();
    let selected = list.selected.min(list.items.len() - 1);
    let items = &list.items;
    list.offset = scroll_offset(selected, list.offset, list.items.len(), area.height, |i| {
        lines
            .entry(i)
            .or_insert_with(|| row_lines(&items[i], content_width(content, &items[i]), cell, me, t))
            .height()
    });

    let mut y = area.y;
    let mut below = list.offset;
    for (i, item) in list.items.iter().enumerate().skip(list.offset) {
        if y >= area.bottom() {
            break;
        }
        let pl = lines
            .entry(i)
            .or_insert_with(|| row_lines(item, content_width(content, item), cell, me, t));
        let h = pl.height();
        let visible = (area.bottom() - y).min(h);
        let row = Rect {
            y,
            height: visible,
            ..area
        };
        let whole = visible == h;
        // A post cut off at the bottom draws no pictures, so it is the first
        // one to get ready.
        below = if whole { i + 1 } else { i };
        draw_marker(frame, row, h, i == list.selected, t);

        // Replies are indented, with a guide line for each level.
        let indent = item.indent() * 2;
        for level in 0..item.indent() {
            let guide = vec![Line::from("│"); usize::from(visible.saturating_sub(1).max(1))];
            let x = area.x + MARK_W + level * 2;
            frame.render_widget(
                Paragraph::new(guide).style(t.dim()),
                Rect {
                    x,
                    y,
                    width: 1,
                    height: visible,
                }
                .intersection(area),
            );
        }
        if let Some(url) = item.post().and_then(|p| p.author.avatar.as_ref())
            && whole
        {
            // Beside the post itself, below the thread lines above it.
            let avatar = Rect {
                x: area.x + MARK_W + indent,
                y: y + pl.context.len() as u16,
                width: AVATAR.0,
                height: AVATAR.1,
            };
            images.draw(frame, avatar, &small_avatar(url));
        }
        let content = Rect {
            x: content.x + indent.min(content.width),
            width: content.width.saturating_sub(indent),
            ..content
        };
        let c = Rect {
            y,
            height: visible,
            ..content
        };
        let mut text: Vec<Line> = pl.context.clone();
        text.push(pl.header.clone());
        text.extend(pl.body.iter().cloned());
        let body_rows = text.len() as u16;
        frame.render_widget(Paragraph::new(text), c);

        let mut next = y + body_rows;
        if !pl.images.is_empty() {
            let img_area = Rect {
                y: next,
                height: pl.image_rows,
                ..content
            };
            if whole {
                draw_image_row(frame, img_area, &pl.images, images);
            }
            next += pl.image_rows;
        }
        if next < area.bottom() {
            frame.render_widget(
                pl.stats.clone(),
                Rect {
                    y: next,
                    height: 1,
                    ..content
                },
            );
        }
        y += h;
    }
    for (i, item) in list.items.iter().enumerate().skip(below).take(PREFETCH) {
        let pl = lines
            .entry(i)
            .or_insert_with(|| row_lines(item, content_width(content, item), cell, me, t));
        if let Some(url) = item.post().and_then(|p| p.author.avatar.as_ref()) {
            images.prefetch(&small_avatar(url), AVATAR.0, AVATAR.1);
        }
        // The same box sizes draw_image_row gives them.
        let width = content.width.saturating_sub(item.indent() * 2);
        let each = image_box_width(width, pl.images.len() as u16);
        for url in &pl.images {
            images.prefetch(url, each, pl.image_rows);
        }
    }
    // Further on, only the downloads: the layout is not needed for them.
    for item in list.items.iter().skip(below + PREFETCH).take(WARM) {
        let Some(post) = item.post() else { continue };
        if let Some(url) = &post.author.avatar {
            images.warm(&small_avatar(url));
        }
        if let Some(embed) = &post.embed {
            for i in embed.images().into_iter().take(4) {
                images.warm(i.url);
            }
        }
    }
}

fn draw_image_row(frame: &mut Frame, area: Rect, urls: &[String], images: &mut Images) {
    let each = image_box_width(area.width, urls.len() as u16);
    if each == 0 {
        return;
    }
    for (i, url) in urls.iter().enumerate() {
        let x = area.x + i as u16 * (each + 1);
        images.draw(
            frame,
            Rect {
                x,
                width: each,
                ..area
            },
            url,
        );
    }
}

fn account_lines(p: &Profile, width: u16, t: &Theme) -> Vec<Line<'static>> {
    let width = usize::from(width.max(1));
    let mut head = vec![
        Span::styled(p.name().to_string(), Style::new().bold()),
        Span::styled(format!(" @{}", p.handle), t.dim()),
    ];
    if p.following_uri().is_some() {
        head.push(Span::styled("  ✓ following", t.accent()));
    }
    let desc = p
        .description
        .as_deref()
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    vec![
        truncate_line(Line::from(head), width),
        Line::styled(truncate(&desc, width), t.dim()),
    ]
}

fn draw_accounts(
    frame: &mut Frame,
    area: Rect,
    list: &mut List<Profile>,
    images: &mut Images,
    t: &Theme,
) {
    let messages = (" searching…", " No accounts found.");
    draw_two_line_rows(
        frame,
        area,
        list,
        images,
        messages,
        t,
        |p| p.avatar.as_deref(),
        |p, w| account_lines(p, w, t),
    );
}

/// A list whose entries are an avatar beside two lines of text.
#[allow(clippy::too_many_arguments)]
fn draw_two_line_rows<T>(
    frame: &mut Frame,
    area: Rect,
    list: &mut List<T>,
    images: &mut Images,
    (loading, empty): (&str, &str),
    t: &Theme,
    avatar: impl Fn(&T) -> Option<&str>,
    lines: impl Fn(&T, u16) -> Vec<Line<'static>>,
) {
    if !list.loaded {
        frame.render_widget(Paragraph::new(loading).style(t.dim()), area);
        return;
    }
    if list.items.is_empty() {
        empty_message(frame, area, list, empty, t);
        return;
    }
    const H: u16 = 3;
    list.offset = scroll_offset(
        list.selected,
        list.offset,
        list.items.len(),
        area.height,
        |_| H,
    );
    let content = content_rect(area, images.shows());
    let mut y = area.y;
    for (i, item) in list.items.iter().enumerate().skip(list.offset) {
        if y + H > area.bottom() {
            break;
        }
        let row = Rect {
            y,
            height: H,
            ..area
        };
        draw_marker(frame, row, H, i == list.selected, t);
        if let Some(url) = avatar(item) {
            let r = Rect {
                x: area.x + MARK_W,
                y,
                width: AVATAR.0,
                height: AVATAR.1,
            };
            images.draw(frame, r, &small_avatar(url));
        }
        frame.render_widget(
            Paragraph::new(lines(item, content.width)),
            Rect {
                y,
                height: 2,
                ..content
            },
        );
        y += H;
    }
    // The rows below the screen: their avatars are ready when scrolled to.
    let shown = usize::from(area.height / H);
    for (n, item) in list
        .items
        .iter()
        .skip(list.offset + shown)
        .take(WARM)
        .enumerate()
    {
        if let Some(url) = avatar(item) {
            let url = small_avatar(url);
            if n < PREFETCH * 2 {
                images.prefetch(&url, AVATAR.0, AVATAR.1);
            } else {
                images.warm(&url);
            }
        }
    }
}

/// What a notification says its author did.
fn notif_action(reason: &str) -> String {
    match reason {
        "like" => "liked your post".into(),
        "repost" => "reposted your post".into(),
        "follow" => "followed you".into(),
        "mention" => "mentioned you".into(),
        "reply" => "replied to you".into(),
        "quote" => "quoted your post".into(),
        "like-via-repost" => "liked your repost".into(),
        "repost-via-repost" => "reposted your repost".into(),
        "starterpack-joined" => "joined through your starter pack".into(),
        "verified" => "verified you".into(),
        "unverified" => "removed your verification".into(),
        "subscribed-post" => "posted".into(),
        other => format!("({other})"),
    }
}

fn notif_lines(item: &NotifItem, width: u16, t: &Theme) -> Vec<Line<'static>> {
    let width = usize::from(width.max(1));
    let n = &item.n;
    let mut head = Vec::new();
    if item.fresh {
        head.push(Span::styled("● ", t.accent()));
    }
    head.push(Span::styled(
        n.author.name().to_string(),
        Style::new().bold(),
    ));
    head.push(Span::styled(format!(" @{}", n.author.handle), t.dim()));
    head.push(Span::raw(format!(" {}", notif_action(&n.reason))));
    head.push(Span::styled(
        format!(" · {}", format_time(&n.indexed_at)),
        t.dim(),
    ));
    // The post itself for a reply or mention, the viewer's own post (dim)
    // for a like or repost; nothing for a follow.
    let text = |p: &Post| p.record().text.lines().next().unwrap_or("").to_string();
    let second = match (&item.post, &item.subject) {
        (Some(p), _) => Line::from(truncate(&text(p), width)),
        (None, Some(p)) => Line::styled(truncate(&text(p), width), t.dim()),
        (None, None) => Line::default(),
    };
    vec![truncate_line(Line::from(head), width), second]
}

fn draw_notifications(frame: &mut Frame, area: Rect, app: &mut App, images: &mut Images) {
    let t = app.theme;
    let messages = (" loading…", " No notifications yet.");
    draw_two_line_rows(
        frame,
        area,
        &mut app.notifications,
        images,
        messages,
        &t,
        |i| i.n.author.avatar.as_deref(),
        |i, w| notif_lines(i, w, &t),
    );
}

fn draw_search(frame: &mut Frame, area: Rect, app: &mut App, images: &mut Images) {
    let t = &app.theme.clone();
    let [bar, input, gap, results] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas(area);
    let mode = app.search.mode;
    let tab = |m: SearchMode, label: &'static str| {
        if m == mode {
            Span::styled(format!(" {label} "), t.selected())
        } else {
            Span::styled(format!(" {label} "), t.dim())
        }
    };
    frame.render_widget(
        Line::from(vec![
            Span::raw(" "),
            tab(SearchMode::Posts, "Posts"),
            Span::raw(" "),
            tab(SearchMode::Accounts, "Accounts"),
        ]),
        bar,
    );
    let prompt = " search: ";
    let field = Rect {
        x: input.x + prompt.width() as u16,
        width: input.width.saturating_sub(prompt.width() as u16 + 1),
        ..input
    };
    // A focused box looks different from one that is only showing the last
    // query: reversed prompt and a cursor, against a dim prompt and a hint.
    let prompt_style = if app.search.editing {
        t.selected()
    } else {
        t.dim()
    };
    frame.render_widget(Paragraph::new(prompt).style(prompt_style), input);
    match (app.search.input.is_empty(), app.search.editing) {
        (true, false) => {
            frame.render_widget(Paragraph::new("press / or i to type").style(t.dim()), field)
        }
        (true, true) => {
            frame.render_widget(Paragraph::new("type, then enter").style(t.dim()), field);
            frame.set_cursor_position(Position::new(field.x, field.y));
        }
        _ => draw_single_input(frame, field, &app.search.input, app.search.editing),
    }
    let _ = gap;
    match mode {
        SearchMode::Posts => {
            if app.search.posts.loaded || !app.search.posts.items.is_empty() || app.pending > 0 {
                let me = app.session.as_ref().map(|s| s.did.as_str());
                draw_posts(
                    frame,
                    results,
                    &mut app.search.posts,
                    images,
                    "No posts found.",
                    me,
                    t,
                );
            }
        }
        SearchMode::Accounts => {
            if app.search.actors.loaded || app.pending > 0 {
                draw_accounts(frame, results, &mut app.search.actors, images, t);
            }
        }
    }
}

fn draw_profile(frame: &mut Frame, area: Rect, app: &mut App, images: &mut Images) {
    let t = &app.theme.clone();
    let Some(p) = app.profile.profile.clone() else {
        let msg = match &app.profile.error {
            Some(e) => Paragraph::new(format!(" could not load the profile: {e}  (R to retry)"))
                .style(t.error()),
            None => Paragraph::new(" loading…").style(t.dim()),
        };
        frame.render_widget(msg.wrap(ratatui::widgets::Wrap { trim: true }), area);
        return;
    };
    let big = if images.shows() { BIG_AVATAR } else { (0, 0) };
    let text_x = if images.shows() {
        MARK_W + big.0 + 2
    } else {
        MARK_W + 1
    };
    let text_w = area.width.saturating_sub(text_x);
    let desc: Vec<String> = wrap(
        p.description.as_deref().unwrap_or(""),
        usize::from(text_w.max(1)),
    )
    .into_iter()
    .take(4)
    .collect();
    let mut lines = vec![
        Line::styled(p.name().to_string(), Style::new().bold()),
        Line::styled(format!("@{}", p.handle), t.dim()),
        Line::from(vec![
            Span::styled(
                format!("{}", p.followers_count.unwrap_or(0)),
                Style::new().bold(),
            ),
            Span::raw(" followers  "),
            Span::styled(
                format!("{}", p.follows_count.unwrap_or(0)),
                Style::new().bold(),
            ),
            Span::raw(" following  "),
            Span::styled(
                format!("{}", p.posts_count.unwrap_or(0)),
                Style::new().bold(),
            ),
            Span::raw(" posts"),
        ]),
    ];
    let own = app.profile.actor.is_none();
    if !own {
        let viewer = p.viewer.clone().unwrap_or_default();
        let mut rel = vec![if viewer.following.is_some() {
            Span::styled("✓ following", t.accent().bold())
        } else {
            Span::styled("not following", t.dim())
        }];
        if viewer.followed_by.is_some() {
            rel.push(Span::styled("  · follows you", t.dim()));
        }
        rel.push(Span::styled("  (f to toggle)", t.dim()));
        lines.push(Line::from(rel));
    } else {
        // Buttons that say the keys exist: bsky does not read the mouse, so
        // they are pressed with the key they name.
        let button = |key: &'static str, what: &'static str| {
            [
                Span::styled("[ ", t.accent()),
                Span::styled(key, t.accent().bold()),
                Span::styled(format!(" {what} ]"), t.accent()),
            ]
        };
        let mut row = vec![Span::styled("this is you  ", t.dim())];
        row.extend(button("e", "Edit profile"));
        row.push(Span::raw("  "));
        row.extend(button("s", "Settings"));
        lines.push(Line::from(row));
    }
    lines.extend(desc.into_iter().map(Line::from));
    let head_h = (lines.len() as u16).max(big.1) + 1;
    let [head, feed] =
        Layout::vertical([Constraint::Length(head_h), Constraint::Min(1)]).areas(area);
    if let Some(url) = &p.avatar {
        let a = Rect {
            x: head.x + MARK_W,
            y: head.y,
            width: BIG_AVATAR.0,
            height: BIG_AVATAR.1,
        };
        images.draw(frame, a.intersection(head), url);
    }
    frame.render_widget(
        Paragraph::new(lines),
        Rect {
            x: head.x + text_x.min(head.width),
            width: text_w,
            height: head_h - 1,
            ..head
        },
    );
    draw_posts(
        frame,
        feed,
        &mut app.profile.posts,
        images,
        "No posts yet.",
        None,
        t,
    );
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
    // Clear resets the cells to the terminal's colors; the block's style
    // paints them back in the theme's.
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .style(t.base())
        .border_style(t.accent())
        .title(format!(" {title} "));
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

fn draw_login(frame: &mut Frame, area: Rect, form: &LoginForm, t: &Theme) {
    let inner = popup(frame, area, 64, 17, "Log in to Bluesky", t);
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(" Your Bluesky password works; an app password is safer."),
            Line::styled(" Settings → Privacy and security → App passwords", t.dim()),
            Line::styled(
                " bsky is an unofficial client, not made by Bluesky.",
                t.dim(),
            ),
        ]),
        rows[0],
    );
    for (i, label) in LoginForm::LABELS.iter().enumerate() {
        let focused = form.focus == i && !form.pending;
        let style = if focused {
            t.accent().bold()
        } else {
            Style::new()
        };
        frame.render_widget(
            Paragraph::new(format!(" {label}")).style(style),
            rows[2 + i * 2],
        );
        let field = rows[3 + i * 2];
        let field = Rect {
            x: field.x + 3,
            width: field.width.saturating_sub(4),
            ..field
        };
        frame.render_widget(
            Paragraph::new("›").style(t.accent()),
            Rect {
                x: field.x - 2,
                width: 1,
                ..field
            },
        );
        draw_single_input(frame, field, &form.fields[i], focused);
    }
    // The message area takes whatever rows are left, so a server's reason,
    // which is the part the user needs, is never cut off by the wrap.
    let msg = if form.pending {
        Line::styled(" logging in…", t.dim())
    } else if let Some(e) = &form.error {
        Line::styled(format!(" {e}"), t.error())
    } else {
        Line::raw("")
    };
    frame.render_widget(
        Paragraph::new(msg).wrap(ratatui::widgets::Wrap { trim: true }),
        rows[9],
    );
    frame.render_widget(
        Paragraph::new(" enter next/submit  tab switch field  esc quit").style(t.dim()),
        rows[10],
    );
}

fn draw_compose(frame: &mut Frame, area: Rect, c: &Compose, images: &mut Images, t: &Theme) {
    let (title, quoted) = match (&c.reply, &c.quote) {
        (Some((_, handle, excerpt)), _) => (format!("Reply to @{handle}"), Some(excerpt)),
        (None, Some((_, handle, excerpt))) => (format!("Quote @{handle}"), Some(excerpt)),
        (None, None) => ("New post".to_string(), None),
    };
    let n = c.media.len() as u16;
    // A row of thumbnails, then a line per picture for its alt text.
    let pics_h = if n > 0 { THUMB.1 + n } else { 0 };
    let inner = popup(frame, area, 72, 14 + pics_h, &title, t);
    let quote_h = if quoted.is_some() { 2 } else { 0 };
    // On a box too short for all of it, the pictures take everything left
    // after the quote, one row of text, and the footer. Asking for more
    // than the box has left them nothing at all, so what the post would
    // send was not on the screen anywhere.
    let pics_h = pics_h.min(inner.height.saturating_sub(quote_h + 2));
    let [quote, text, pics, foot] = Layout::vertical([
        Constraint::Length(quote_h),
        Constraint::Min(1),
        Constraint::Length(pics_h),
        Constraint::Length(1),
    ])
    .areas(inner);
    if let Some(excerpt) = quoted {
        frame.render_widget(
            Paragraph::new(truncate(&format!("❝ {excerpt}"), usize::from(quote.width)))
                .style(t.dim()),
            quote,
        );
    }
    let typing = !c.sending && c.browser.is_none();
    let text_area = Rect {
        x: text.x + 1,
        width: text.width.saturating_sub(2),
        ..text
    };
    draw_multi_input(frame, text_area, &c.input, typing && c.focus == 0);
    if n > 0 {
        draw_attachments(frame, pics, c, images, typing, t);
    }
    let text = c.input.text();
    let text = text.trim_end();
    let len = grapheme_len(text);
    let count_style = if post_length_problem(text).is_some() {
        t.error()
    } else {
        t.dim()
    };
    // Bytes are shown only once they are what limits the post.
    let bytes = if text.len() > MAX_POST_BYTES - 500 {
        format!(" {}/{MAX_POST_BYTES} bytes", text.len())
    } else {
        String::new()
    };
    let action = if c.sending {
        "sending…"
    } else if n > 0 {
        "ctrl+s send  ctrl+o attach  tab alt text  ctrl+x remove  esc cancel"
    } else {
        "ctrl+s send  ctrl+o attach pictures or a video  esc cancel"
    };
    frame.render_widget(
        truncate_line(
            Line::from(vec![
                Span::styled(format!(" {len}/{MAX_POST_GRAPHEMES}{bytes}  "), count_style),
                Span::styled(action, t.dim()),
            ]),
            usize::from(foot.width),
        ),
        foot,
    );
}

/// The composer's pictures: thumbnails in a row, then each one's number,
/// file name, and alt text.
fn draw_attachments(
    frame: &mut Frame,
    area: Rect,
    c: &Compose,
    images: &mut Images,
    typing: bool,
    t: &Theme,
) {
    let n = c.media.len() as u16;
    // Room for a strip of thumbnails above a line per picture.
    let room = area.height >= THUMB.1 + n;
    // No thumbnails behind the browser (they would only cost encodes), but
    // their rows stay where they are, so the list does not jump when the
    // browser opens and closes.
    let thumbs = c.browser.is_none() && room;
    // Where there is no room the strip is not kept empty either: the names
    // start at the top of the area instead of below a strip that is not
    // drawn, which put them past the bottom of the box.
    let top = if room { THUMB.1 } else { 0 };
    let rows = area.height.saturating_sub(top);
    // Fewer rows than pictures: the last row counts the ones left out, so
    // the post never carries a picture the screen does not mention.
    let shown = if n > rows { rows.saturating_sub(1) } else { n };
    for (i, a) in c.media.iter().enumerate().take(usize::from(shown)) {
        let i16 = i as u16;
        let x = area.x + 1 + i16 * (THUMB.0 + 1);
        if thumbs && x + THUMB.0 <= area.right() {
            let r = Rect {
                x,
                y: area.y,
                width: THUMB.0,
                height: THUMB.1,
            };
            draw_media_box(frame, r, &a.path, a.info, images, t);
        }
        let row = Rect {
            y: area.y + top + i16,
            height: 1,
            ..area
        };
        if row.y >= area.bottom() {
            break;
        }
        let focused = typing && c.focus == i + 1;
        let name = a
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let what = match (a.info.animated_gif, a.info.seconds) {
            (true, _) => " (GIF, posted as a video)".to_string(),
            (false, Some(s)) if a.info.kind == media::Kind::Video => {
                format!(" (video {})", format_seconds(s))
            }
            (false, None) if a.info.kind == media::Kind::Video => " (video)".to_string(),
            _ => String::new(),
        };
        let label = format!(" {} {}{what}  alt: ", i + 1, truncate(&name, 20));
        let label_w = label.width() as u16;
        let style = if focused { t.accent().bold() } else { t.dim() };
        frame.render_widget(Paragraph::new(label).style(style), row);
        let field = Rect {
            x: row.x + label_w.min(row.width),
            width: row.width.saturating_sub(label_w + 1),
            ..row
        };
        if a.alt.is_empty() && !focused {
            frame.render_widget(
                Paragraph::new("(none; tab to describe it)").style(t.dim()),
                field,
            );
        } else {
            draw_single_input(frame, field, &a.alt, focused);
        }
    }
    if n > shown {
        frame.render_widget(
            Paragraph::new(format!(" and {} more", n - shown)).style(t.dim()),
            Rect {
                y: area.y + top + shown,
                height: 1,
                ..area
            },
        );
    }
}

/// A picture drawn in its box. A video has no frame to show (bsky does not
/// decode video), so its box says what it is and how long it runs; an
/// animated GIF shows its first frame.
fn draw_media_box(
    frame: &mut Frame,
    area: Rect,
    path: &std::path::Path,
    info: media::Info,
    images: &mut Images,
    t: &Theme,
) {
    if info.kind == media::Kind::Picture || info.animated_gif {
        images.draw_file(frame, area, path);
        return;
    }
    let mut lines = vec![Line::styled("▶ video", t.accent().bold())];
    if let Some(s) = info.seconds {
        lines.push(Line::styled(format_seconds(s), t.dim()));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(ratatui::layout::Alignment::Center)
            .block(Block::bordered().border_style(t.dim())),
        area,
    );
}

/// What a picture or video is, in one line: its kind, length, size.
fn describe(i: media::Info, bytes: u64) -> String {
    let mut parts = Vec::new();
    if i.animated_gif {
        parts.push("animated GIF, posted as a video".to_string());
    } else if i.kind == media::Kind::Video {
        parts.push("video".to_string());
    }
    if let Some(s) = i.seconds {
        parts.push(format_seconds(s));
    }
    if let Some((w, h)) = i.dims {
        parts.push(format!("{w}×{h}"));
    }
    parts.push(human_bytes(bytes));
    parts.join(" · ")
}

/// A byte count the way a person reads it.
fn human_bytes(n: u64) -> String {
    match n {
        n if n >= 1024 * 1024 => format!("{:.1} MB", n as f64 / (1024.0 * 1024.0)),
        n if n >= 1024 => format!("{} KB", n / 1024),
        n => format!("{n} B"),
    }
}

/// Keep the end of `s` within `width` columns: the end of a path is the part
/// that tells where it is.
fn truncate_start(s: &str, width: usize) -> String {
    if s.width() <= width {
        return s.to_string();
    }
    // Whole grapheme clusters, so an emoji keeps its modifier and a flag
    // both of its letters.
    let mut out: Vec<&str> = Vec::new();
    let mut used = 1;
    for g in unicode_segmentation::UnicodeSegmentation::graphemes(s, true).rev() {
        let w = g.width();
        if used + w > width {
            break;
        }
        used += w;
        out.push(g);
    }
    out.push("…");
    out.into_iter().rev().collect()
}

/// The picture browser: the folder on top, its folders and pictures on the
/// left, the selected picture previewed on the right.
fn draw_browser(frame: &mut Frame, area: Rect, b: &mut Browser, images: &mut Images, t: &Theme) {
    let title = match (b.videos, b.room) {
        _ if b.folders => "Choose a folder".to_string(),
        (true, n) => format!("Attach pictures (up to {n}) or a video"),
        (false, 1) => "Choose a picture".to_string(),
        (false, n) => format!("Attach pictures (up to {n})"),
    };
    let w = area.width.saturating_sub(4).min(110);
    let h = area.height.saturating_sub(2).min(34);
    let inner = popup(frame, area, w, h, &title, t);
    let [path_row, body, foot] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(inner);
    let dir = b.dir.display().to_string();
    frame.render_widget(
        Paragraph::new(format!(
            " {}",
            truncate_start(&dir, usize::from(path_row.width.saturating_sub(1)))
        ))
        .style(t.accent()),
        path_row,
    );
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(45), Constraint::Min(10)]).areas(body);
    let mut rows = left;
    if let Some(e) = &b.error {
        frame.render_widget(
            Paragraph::new(format!(" {e}")).style(t.error()),
            Rect { height: 1, ..left },
        );
        rows.y += 1;
        rows.height = rows.height.saturating_sub(1);
    }
    if !b.list.items.is_empty() {
        let selected = b.list.selected.min(b.list.items.len() - 1);
        b.list.offset = scroll_offset(
            selected,
            b.list.offset,
            b.list.items.len(),
            rows.height,
            |_| 1,
        );
    }
    let width = usize::from(rows.width);
    for (row, (i, e)) in b
        .list
        .items
        .iter()
        .enumerate()
        .skip(b.list.offset)
        .take(usize::from(rows.height))
        .enumerate()
    {
        let mark = if b.marked.contains(&e.path) {
            "✓ "
        } else {
            "  "
        };
        let (name, size) = match e.kind {
            EntryKind::Parent => ("../".to_string(), String::new()),
            EntryKind::Dir => (format!("{}/", e.name), String::new()),
            EntryKind::Media => (e.name.clone(), human_bytes(e.bytes)),
        };
        let size_w = size.width();
        let name = truncate(&name, width.saturating_sub(size_w + 4));
        let pad = width.saturating_sub(2 + name.width() + size_w + 1);
        let line = format!("{mark}{name}{}{size} ", " ".repeat(pad));
        let style = if i == b.list.selected {
            t.selected()
        } else if e.kind == EntryKind::Media {
            Style::new()
        } else {
            t.accent()
        };
        frame.render_widget(
            Paragraph::new(line).style(style),
            Rect {
                y: rows.y + row as u16,
                height: 1,
                ..rows
            },
        );
    }
    let preview = Rect {
        x: right.x + 1,
        width: right.width.saturating_sub(2),
        ..right
    };
    match b.current() {
        Some(e) if e.kind == EntryKind::Media => {
            let about = b.current_info();
            let info = about.map_or_else(|| human_bytes(e.bytes), |i| describe(i, e.bytes));
            let pic = Rect {
                height: preview.height.saturating_sub(3),
                ..preview
            };
            if let Some(i) = about {
                draw_media_box(frame, pic, &e.path, i, images, t);
            }
            frame.render_widget(
                Paragraph::new(vec![
                    Line::styled(
                        truncate(&e.name, usize::from(preview.width)),
                        Style::new().bold(),
                    ),
                    Line::styled(info, t.dim()),
                ]),
                Rect {
                    y: pic.bottom() + 1,
                    height: 2,
                    ..preview
                },
            );
        }
        Some(_) if b.folders => frame.render_widget(
            Paragraph::new("enter opens the folder; space chooses the one you are in")
                .style(t.dim())
                .wrap(ratatui::widgets::Wrap { trim: true }),
            preview,
        ),
        Some(_) => frame.render_widget(
            Paragraph::new("enter opens the folder").style(t.dim()),
            preview,
        ),
        None if b.folders => frame.render_widget(
            Paragraph::new("no folders here; space chooses this one")
                .style(t.dim())
                .wrap(ratatui::widgets::Wrap { trim: true }),
            preview,
        ),
        None => frame.render_widget(
            Paragraph::new("no folders, pictures, or videos here").style(t.dim()),
            preview,
        ),
    }
    let foot_line = match &b.note {
        Some(n) => Line::styled(format!(" {n}"), t.error()),
        None if b.folders => Line::styled(
            " enter open  space choose this folder  h up  . hidden  ~ home  esc cancel",
            t.dim(),
        ),
        None => {
            let marked = if b.marked.is_empty() {
                String::new()
            } else {
                format!("{} marked  ", b.marked.len())
            };
            Line::styled(
                format!(
                    " {marked}enter open/choose  space mark  h up  . hidden  ~ home  esc cancel"
                ),
                t.dim(),
            )
        }
    };
    frame.render_widget(truncate_line(foot_line, usize::from(foot.width)), foot);
}

fn draw_edit_profile(frame: &mut Frame, area: Rect, e: &EditProfile, t: &Theme) {
    let inner = popup(frame, area, 72, 16, "Edit profile", t);
    if e.loading {
        frame.render_widget(Paragraph::new(" loading…").style(t.dim()), inner);
        return;
    }
    let [l0, f0, _, l1, f1, _, l2, f2, foot] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(4),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas(inner);
    let fields = [(l0, f0), (l1, f1), (l2, f2)];
    for (i, (label, field)) in fields.into_iter().enumerate() {
        let focused = e.focus == i && !e.saving;
        let style = if focused {
            t.accent().bold()
        } else {
            Style::new()
        };
        frame.render_widget(
            Paragraph::new(format!(" {}", EditProfile::LABELS[i])).style(style),
            label,
        );
        let field = Rect {
            x: field.x + 3,
            width: field.width.saturating_sub(4),
            ..field
        };
        if i == 1 {
            draw_multi_input(frame, field, &e.fields[i], focused);
        } else {
            draw_single_input(frame, field, &e.fields[i], focused);
        }
    }
    let action = if e.saving {
        " saving…"
    } else {
        " tab next field  ctrl+o choose avatar  ctrl+s save  esc cancel"
    };
    frame.render_widget(
        Paragraph::new(action).style(t.dim()),
        Rect { height: 1, ..foot },
    );
}

/// The theme picker: every theme's name with a strip of its colors, the
/// selected one marked. The rest of the screen already shows it, since
/// moving the selection applies it.
fn draw_themes(frame: &mut Frame, area: Rect, selected: usize, t: &Theme) {
    let n = THEMES.len();
    // A window of a few themes that scrolls with the selection, keeping it in
    // the middle where it can; not a list from the top of the screen to the
    // bottom.
    let rows = usize::from(area.height.saturating_sub(6)).clamp(1, THEME_ROWS.min(n));
    let first = selected.saturating_sub(rows / 2).min(n - rows);
    let width = THEMES.iter().map(|t| t.name.len()).max().unwrap_or(0) + 1;
    let lines: Vec<Line> = THEMES
        .iter()
        .enumerate()
        .skip(first)
        .take(rows)
        .map(|(i, theme)| {
            let marker = if i == selected { "▶ " } else { "  " };
            let name = format!("{marker}{:<width$}", theme.name);
            let mut spans = vec![if i == selected {
                Span::styled(name, t.accent().bold())
            } else {
                Span::raw(name)
            }];
            // A swatch in the theme's own colors, whatever the current one is.
            for c in [theme.accent, theme.like, theme.repost, theme.ok, theme.dim] {
                spans.push(Span::styled("██", Style::new().fg(c).bg(theme.bg)));
            }
            Line::from(spans)
        })
        .collect();
    let w = (width + 2 + 10 + 4) as u16;
    let inner = popup(frame, area, w.max(34), rows as u16 + 4, "Theme", t);
    let [body, foot] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
    frame.render_widget(Paragraph::new(lines), body);
    // Arrows on the frame say there is more above or below.
    let more = |show: bool, y: u16, mark: &'static str| {
        if show { Some((mark, y)) } else { None }
    };
    for (mark, y) in [
        more(first > 0, body.y.saturating_sub(1), "▲"),
        more(first + rows < n, foot.y + 1, "▼"),
    ]
    .into_iter()
    .flatten()
    {
        frame.render_widget(
            Paragraph::new(mark).style(t.accent()),
            Rect {
                x: body.right().saturating_sub(2),
                y,
                width: 1,
                height: 1,
            },
        );
    }
    frame.render_widget(
        Paragraph::new(format!(" {}/{n}  enter apply  esc cancel", selected + 1)).style(t.dim()),
        foot,
    );
}

fn draw_help(frame: &mut Frame, area: Rect, scroll: &mut u16, pictures: bool, t: &Theme) {
    // The popup is 64 wide where the screen allows it; knowing the width
    // here decides whether a description fits beside its keys.
    let inner_w = usize::from(HELP_W.min(area.width).saturating_sub(2));
    // A description beside the keys needs the key column and enough cells
    // after it to read a few words. On a narrower screen it goes under
    // them, wrapped, rather than being cut to nothing.
    let beside = inner_w >= HELP_KEYS + 20;
    let mut lines: Vec<Line> = Vec::new();
    for (i, (title, keys)) in keys::help(pictures).into_iter().enumerate() {
        if i > 0 {
            lines.push(Line::raw(""));
        }
        lines.push(Line::styled(format!(" {title}"), Style::new().bold()));
        for (k, d) in keys {
            if beside {
                // A description too long for its column wraps onto more
                // rows under itself instead of being cut off.
                let mut parts = wrap(d, inner_w - HELP_KEYS).into_iter();
                let first = parts.next().unwrap_or_default();
                lines.push(Line::from(vec![
                    Span::styled(format!("   {k:<15}"), t.accent().bold()),
                    Span::raw(first),
                ]));
                lines.extend(parts.map(|l| Line::raw(format!("{:HELP_KEYS$}{l}", ""))));
                continue;
            }
            lines.push(Line::from(vec![Span::styled(
                format!("   {k}"),
                t.accent().bold(),
            )]));
            lines.extend(
                wrap(d, inner_w.saturating_sub(5).max(1))
                    .into_iter()
                    .map(|l| Line::raw(format!("     {l}"))),
            );
        }
    }
    let inner = popup(frame, area, HELP_W, lines.len() as u16 + 3, "Keys", t);
    let [body, foot] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
    // Clamp here, where the viewport is known, and write it back so scrolling
    // up after overshooting starts at once.
    let max = (lines.len() as u16).saturating_sub(body.height);
    *scroll = (*scroll).min(max);
    frame.render_widget(Paragraph::new(lines).scroll((*scroll, 0)), body);
    let more = if *scroll < max { "  j/pgdn more" } else { "" };
    frame.render_widget(
        Paragraph::new(format!(" esc/q/? close{more}")).style(t.dim()),
        foot,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Session;
    use crate::tui::worker::Event;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui_image::picker::Picker;
    use serde_json::json;

    fn render(app: &mut App, w: u16, h: u16) -> String {
        let mut images = Images::new(Picker::halfblocks(), None);
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw(f, app, &mut images)).unwrap();
        let buf = term.backend().buffer().clone();
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn session() -> Session {
        Session {
            service: "https://pds.test".into(),
            did: "did:plc:me".into(),
            handle: "me.test".into(),
            access_jwt: "a".into(),
            refresh_jwt: "r".into(),
        }
    }

    fn posts(n: usize) -> Vec<Post> {
        (0..n)
            .map(|i| {
                serde_json::from_value(json!({
                    "uri": format!("at://p/{i}"), "cid": "c",
                    "author": {"did": "did:plc:a", "handle": "alice.test", "displayName": "Alice"},
                    "record": {"text": format!("post number {i}"), "createdAt": "2026-09-22T00:00:00Z"},
                    "likeCount": i,
                }))
                .unwrap()
            })
            .collect()
    }

    #[test]
    fn login_screen_explains_app_passwords() {
        let (mut app, _) = App::new(None, "https://bsky.social");
        let screen = render(&mut app, 80, 24);
        assert!(screen.contains("Log in to Bluesky"), "{screen}");
        assert!(screen.contains("an app password is safer"), "{screen}");
        assert!(screen.contains("unofficial client"), "{screen}");
        assert!(screen.contains("Password"));
        assert!(screen.contains("https://bsky.social"));
    }

    #[test]
    fn a_long_login_error_is_shown_whole() {
        let (mut app, _) = App::new(None, "https://bsky.social");
        app.login.as_mut().unwrap().error = Some(
            "com.atproto.server.createSession failed: AuthenticationRequired: Invalid identifier or password".into(),
        );
        let screen = render(&mut app, 80, 24);
        assert!(
            screen.contains("Invalid identifier or password"),
            "{screen}"
        );
        assert!(screen.contains("esc quit"), "{screen}");
    }

    fn render_text_only(app: &mut App, w: u16, h: u16) -> String {
        let mut images = Images::none();
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw(f, app, &mut images)).unwrap();
        let buf = term.backend().buffer().clone();
        // The cells a wide character covers are skipped, so the text reads
        // as the terminal shows it.
        (0..h)
            .map(|y| {
                let mut row = String::new();
                let mut x = 0;
                while x < w {
                    let sym = buf[(x, y)].symbol();
                    x += sym.width().max(1) as u16;
                    row.push_str(sym);
                }
                row.trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    // Without pictures the text starts where the avatar was, and a line
    // says what each post carries; descriptions keep their emoji whole.
    #[test]
    fn without_pictures_a_post_says_what_it_carries() {
        let (mut app, _) = App::new(Some(session()), "x");
        app.without_pictures();
        let mut photos = posts(1).remove(0);
        photos.embed =
            serde_json::from_value(json!({"$type": "app.bsky.embed.images#view", "images": [
                {"thumb": "https://t/1", "fullsize": "https://f/1", "alt": "家族👨‍👩‍👧 🇯🇵 1️⃣ ❤️"},
                {"thumb": "https://t/2", "fullsize": "https://f/2", "alt": ""}
            ]}))
            .ok();
        let mut clip = posts(2).remove(1);
        clip.embed = serde_json::from_value(json!({"$type": "app.bsky.embed.video#view",
            "playlist": "https://v/p.m3u8", "alt": "a cat"}))
        .ok();
        app.handle_event(Event::Timeline(Ok(vec![photos, clip].into())));
        let screen = render_text_only(&mut app, 80, 24);
        assert!(screen.contains("▣ 2 pictures: 家族👨‍👩‍👧 🇯🇵 1️⃣ ❤️"), "{screen}");
        assert!(screen.contains("▶ video: a cat"), "{screen}");
        // The name starts right after the selection marker and one space.
        assert!(
            !screen.contains("▶ video\n"),
            "the video is named once: {screen}"
        );
        assert!(
            screen.lines().any(|l| l.starts_with("▌  Alice")),
            "{screen}"
        );
        assert!(screen.contains("space open in browser"), "{screen}");
        // A narrow screen cuts the line as one piece.
        let narrow = render_text_only(&mut app, 24, 24);
        assert!(narrow.contains("▣ 2 pictures: 家族👨‍👩‍👧…"), "{narrow}");
    }

    #[test]
    fn timeline_scrolls_to_keep_the_selection_visible() {
        let (mut app, _) = App::new(Some(session()), "x");
        app.handle_event(Event::Timeline(Ok(posts(20).into())));
        let screen = render(&mut app, 80, 24);
        assert!(screen.contains("post number 0"));
        for _ in 0..15 {
            app.handle_key(crossterm::event::KeyEvent::from(
                crossterm::event::KeyCode::Char('j'),
            ));
        }
        let screen = render(&mut app, 80, 24);
        assert!(screen.contains("post number 15"), "{screen}");
        assert!(!screen.contains("post number 0\n"));
        assert!(app.timeline.offset > 0);
    }

    /// Each row's cells, skipping the cells a wide character covers, so the
    /// text reads as a terminal shows it.
    fn cells(app: &mut App, w: u16, h: u16) -> Vec<Vec<String>> {
        let mut images = Images::new(Picker::halfblocks(), None);
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw(f, app, &mut images)).unwrap();
        let buf = term.backend().buffer().clone();
        (0..h)
            .map(|y| {
                let mut row = Vec::new();
                let mut x = 0;
                while x < w {
                    let sym = buf[(x, y)].symbol().to_string();
                    x += sym.width().max(1) as u16;
                    row.push(sym);
                }
                row
            })
            .collect()
    }

    /// A piece of a cluster that only shows when the cluster was cut: a
    /// skin tone alone, one letter of a flag, a joiner or a variation
    /// selector at an end, a combining mark alone.
    fn is_fragment(sym: &str) -> bool {
        let first = sym.chars().next();
        let last = sym.chars().next_back();
        let lone_flag_letter =
            sym.chars().count() == 1 && matches!(first, Some('\u{1F1E6}'..='\u{1F1FF}'));
        matches!(
            first,
            Some(
                '\u{1F3FB}'..='\u{1F3FF}'
                | '\u{200D}'
                | '\u{FE0F}'
                | '\u{20E3}'
                | '\u{300}'..='\u{36F}',
            )
        ) || matches!(last, Some('\u{200D}'))
            || lone_flag_letter
    }

    #[test]
    fn emoji_in_names_and_text_are_never_cut_apart() {
        let name = "👨‍👩‍👧‍👦 Family 🇯🇵";
        let text = "今日は👍🏽 1️⃣ ❤️ e\u{301}t\u{e9} 🇯🇵🇺🇸 😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀";
        let post: Post = serde_json::from_value(json!({
            "uri": "at://p/e", "cid": "c",
            "author": {"did": "did:plc:a", "handle": "alice.test", "displayName": name},
            "record": {"text": text, "createdAt": "2026-09-22T00:00:00Z"},
            "likeCount": 3,
        }))
        .unwrap();
        for width in MIN_W..=80 {
            let (mut app, _) = App::new(Some(session()), "x");
            app.handle_event(Event::Timeline(Ok(vec![post.clone()].into())));
            let rows = cells(&mut app, width, 24);
            for row in &rows {
                let shown: usize = row.iter().map(|c| c.width().max(1)).sum();
                assert!(shown <= width as usize, "{width}: {row:?}");
                for c in row {
                    assert!(!is_fragment(c), "{width}: a cut cluster {c:?} in {row:?}");
                }
            }
            let screen: String = rows.iter().map(|r| r.concat() + "\n").collect();
            for whole in ["👍🏽", "1️⃣", "🇯🇵", "🇺🇸", "e\u{301}"] {
                assert!(
                    screen.contains(whole),
                    "{width}: {whole} is not whole in\n{screen}"
                );
            }
            if width >= 41 {
                assert!(screen.contains("👨‍👩‍👧‍👦 Family 🇯🇵"), "{screen}");
            }
        }
    }

    #[test]
    fn the_feed_bar_names_the_feeds_and_keeps_the_shown_one_in_sight() {
        let names = [
            "Discover",
            "Science 🔬",
            "日本語👨\u{200d}👩\u{200d}👧\u{200d}👦フィード",
            "Cats",
            "A feed with a very long name that goes on",
        ];
        let (mut app, _) = App::new(Some(session()), "x");
        app.handle_event(Event::Timeline(Ok(posts(3).into())));
        app.handle_event(Event::PinnedFeeds(Ok(names
            .iter()
            .map(|n| crate::api::types::FeedInfo {
                uri: format!("at://f/{n}"),
                name: n.to_string(),
            })
            .collect())));
        let rows = cells(&mut app, 100, 12);
        let bar = rows[1].concat();
        assert!(
            bar.starts_with("  Following   Discover   Science 🔬 "),
            "{bar:?}"
        );
        assert!(bar.trim_end().ends_with("…  [ ] feeds"), "{bar:?}");
        let screen = render(&mut app, 100, 12);
        assert!(screen.contains("post number 0"), "{screen}");
        // The last feed shown on a narrow screen: still in sight.
        for _ in 0..5 {
            app.handle_key(crossterm::event::KeyEvent::from(
                crossterm::event::KeyCode::Char(']'),
            ));
        }
        for width in [30u16, 40, 60, 100] {
            let rows = cells(&mut app, width, 12);
            let bar: String = rows[1].concat();
            assert!(bar.contains("A feed with"), "{width}: {bar:?}");
            assert!(bar.contains("[ ] feeds"), "{width}: {bar:?}");
            for row in &rows {
                for c in row {
                    assert!(!is_fragment(c), "{width}: a cut cluster {c:?} in {row:?}");
                }
            }
        }
        // Until it has loaded, the feed says so; empty, it says that.
        assert!(render(&mut app, 100, 12).contains("loading…"));
        app.handle_event(Event::CustomFeed {
            uri: format!("at://f/{}", names[4]),
            result: Ok(Vec::new().into()),
        });
        let screen = render(&mut app, 100, 12);
        assert!(screen.contains("No posts in this feed yet"), "{screen}");
    }

    #[test]
    fn empty_timeline_says_why() {
        let (mut app, _) = App::new(Some(session()), "x");
        app.handle_event(Event::Timeline(Ok(vec![].into())));
        assert!(render(&mut app, 80, 10).contains("No posts from accounts you follow"));
    }

    #[test]
    fn a_terminal_below_the_minimum_says_so_and_gives_both_sizes() {
        let (mut app, _) = App::new(Some(session()), "x");
        app.handle_event(Event::Timeline(Ok(posts(3).into())));
        let small = render(&mut app, 23, 7);
        assert!(small.contains("Terminal too small"), "{small}");
        assert!(small.contains("24x8 needed, now 23x7"), "{small}");
        // Only the notice: no shreds of the tabs, the posts, or the hints.
        assert!(!small.contains("Timeline"), "{small}");
        assert!(!small.contains("post number"), "{small}");
        assert!(!small.contains("? help"), "{small}");
        // One cell short in either direction is still too small.
        assert!(render(&mut app, 24, 7).contains("Terminal too small"));
        assert!(render(&mut app, 23, 8).contains("Terminal too small"));
        // At the minimum the client is drawn.
        let ok = render(&mut app, 24, 8);
        assert!(!ok.contains("Terminal too small"), "{ok}");
        assert!(ok.contains("post number 0"), "{ok}");
    }

    #[test]
    fn the_notice_replaces_the_login_form_too_and_fits_the_narrowest_screen() {
        let (mut login, _) = App::new(None, "x");
        let small = render(&mut login, 20, 7);
        assert!(small.contains("Terminal too small"), "{small}");
        assert!(!small.contains("Handle"), "{small}");
        // Narrower than the words: they wrap instead of being cut off.
        let thin = render(&mut login, 9, 6);
        assert!(thin.contains("Terminal"), "{thin}");
        assert!(thin.contains("small"), "{thin}");
        assert!(thin.contains("now 9x6"), "{thin}");
    }

    /// Every view, on every screen from the smallest the client draws in up
    /// to a comfortable one, stays inside the screen and never cuts a
    /// cluster apart. The narrow screens are where the columns a layout
    /// reserves (an avatar, a key column, a row of thumbnails) stop fitting.
    #[test]
    fn every_view_on_a_cramped_screen_stays_inside_it() {
        use crossterm::event::{KeyCode, KeyEvent};
        fn ch(c: char) -> KeyEvent {
            KeyEvent::from(KeyCode::Char(c))
        }
        fn emoji_posts() -> Vec<Post> {
            (0..4)
                .map(|i| {
                    serde_json::from_value(json!({
                        "uri": format!("at://p/{i}"), "cid": "c",
                        "author": {"did": "did:plc:a", "handle": "alice.test",
                                   "displayName": "👨‍👩‍👧‍👦 家族 🇯🇵 Alice"},
                        "record": {"text": "今日は👍🏽 1️⃣ ❤️ e\u{301}te\u{301} 🇯🇵🇺🇸 https://example.com/a?b=1 @bob.test #タグ",
                                   "createdAt": "2026-09-22T00:00:00Z"},
                        "likeCount": 12,
                        "embed": {"$type": "app.bsky.embed.images#view", "images": [
                            {"thumb": "https://t/1", "fullsize": "https://f/1", "alt": "山の頂上👨‍👩‍👧", "aspectRatio": {"width": 4, "height": 3}},
                            {"thumb": "https://t/2", "fullsize": "https://f/2", "alt": ""}
                        ]}
                    }))
                    .unwrap()
                })
                .collect()
        }
        /// A named view, built from scratch for each size.
        type State = (&'static str, Box<dyn Fn() -> App>);
        let states: Vec<State> = vec![
            (
                "timeline",
                Box::new(|| {
                    let (mut a, _) = App::new(Some(session()), "x");
                    a.handle_event(Event::Timeline(Ok(emoji_posts().into())));
                    a
                }),
            ),
            (
                "compose",
                Box::new(move || {
                    let (mut a, _) = App::new(Some(session()), "x");
                    a.handle_event(Event::Timeline(Ok(emoji_posts().into())));
                    a.handle_key(ch('n'));
                    for c in "今日は👍🏽 1️⃣ ❤️ 🇯🇵 a rather long draft that wraps".chars()
                    {
                        a.handle_key(ch(c));
                    }
                    a
                }),
            ),
            (
                "compose+pictures",
                Box::new(move || {
                    let (mut a, _) = App::new(Some(session()), "x");
                    a.handle_event(Event::Timeline(Ok(emoji_posts().into())));
                    a.handle_key(ch('n'));
                    let Some(Overlay::Compose(c)) = &mut a.overlay else {
                        panic!()
                    };
                    for i in 0..4 {
                        c.media
                            .push(crate::tui::app::Attached::new(std::path::PathBuf::from(
                                format!("/tmp/写真👨\u{200d}👩\u{200d}👧{i}.png"),
                            )));
                    }
                    a
                }),
            ),
            (
                "reply",
                Box::new(move || {
                    let (mut a, _) = App::new(Some(session()), "x");
                    a.handle_event(Event::Timeline(Ok(emoji_posts().into())));
                    a.handle_key(ch('r'));
                    a
                }),
            ),
            (
                "help",
                Box::new(move || {
                    let (mut a, _) = App::new(Some(session()), "x");
                    a.handle_event(Event::Timeline(Ok(emoji_posts().into())));
                    a.handle_key(ch('?'));
                    a
                }),
            ),
            (
                "themes",
                Box::new(move || {
                    let (mut a, _) = App::new(Some(session()), "x");
                    a.handle_event(Event::Timeline(Ok(emoji_posts().into())));
                    a.handle_key(ch('T'));
                    a
                }),
            ),
            (
                "viewer",
                Box::new(move || {
                    let (mut a, _) = App::new(Some(session()), "x");
                    a.handle_event(Event::Timeline(Ok(emoji_posts().into())));
                    a.handle_key(KeyEvent::from(KeyCode::Char(' ')));
                    a
                }),
            ),
            (
                "thread",
                Box::new(move || {
                    let (mut a, _) = App::new(Some(session()), "x");
                    a.handle_event(Event::Timeline(Ok(emoji_posts().into())));
                    a.handle_key(ch('v'));
                    a
                }),
            ),
            (
                "search",
                Box::new(move || {
                    let (mut a, _) = App::new(Some(session()), "x");
                    a.handle_key(ch('2'));
                    a.handle_key(ch('/'));
                    for c in "検索👍🏽 word".chars() {
                        a.handle_key(ch(c));
                    }
                    a
                }),
            ),
            (
                "profile",
                Box::new(move || {
                    let (mut a, _) = App::new(Some(session()), "x");
                    a.handle_key(ch('4'));
                    a
                }),
            ),
            (
                "own profile",
                Box::new(move || {
                    let (mut a, _) = App::new(Some(session()), "x");
                    a.handle_key(ch('4'));
                    a.handle_event(Event::Profile(Ok((own_profile(), emoji_posts().into()))));
                    a
                }),
            ),
            (
                "settings",
                Box::new(move || {
                    let (mut a, _) = App::new(Some(session()), "x");
                    a.env.download_dir =
                        Some("/home/me/写真/👨\u{200d}👩\u{200d}👧 家族 🇯🇵/1️⃣ ❤️ e\u{301}".into());
                    a.handle_key(ch('4'));
                    a.handle_event(Event::Profile(Ok((own_profile(), emoji_posts().into()))));
                    a.handle_key(ch('s'));
                    a.handle_key(ch('j'));
                    a.handle_key(ch('j'));
                    a
                }),
            ),
            (
                "settings typing",
                Box::new(move || {
                    let (mut a, _) = App::new(Some(session()), "x");
                    a.handle_key(ch('4'));
                    a.handle_key(ch('s'));
                    for _ in 0..5 {
                        a.handle_key(ch('j'));
                    }
                    a.handle_key(KeyEvent::from(KeyCode::Enter));
                    for c in "ブラウザ👨\u{200d}👩\u{200d}👧 🇯🇵 1️⃣ ❤️ e\u{301} مرحبا".chars()
                    {
                        a.handle_key(ch(c));
                    }
                    a
                }),
            ),
            (
                "settings folder",
                Box::new(move || {
                    let (mut a, _) = App::new(Some(session()), "x");
                    a.browse_from = Some(std::env::temp_dir());
                    a.env.download_dir = None;
                    a.handle_key(ch('4'));
                    a.handle_key(ch('s'));
                    a.handle_key(ch('j'));
                    a.handle_key(ch('j'));
                    a.handle_key(KeyEvent::from(KeyCode::Enter));
                    a
                }),
            ),
            (
                "login",
                Box::new(|| {
                    let (a, _) = App::new(None, "https://bsky.social");
                    a
                }),
            ),
        ];
        for (name, make) in &states {
            for w in [MIN_W, MIN_W + 1, 30, 33, 40, 47, 56] {
                for h in [MIN_H, MIN_H + 1, 12, 16] {
                    let mut app = make();
                    let rows = cells(&mut app, w, h);
                    for (y, row) in rows.iter().enumerate() {
                        let shown: usize = row.iter().map(|c| c.width().max(1)).sum();
                        assert!(shown <= w as usize, "{name} {w}x{h} row {y}: {row:?}");
                        for c in row {
                            assert!(
                                !is_fragment(c),
                                "{name} {w}x{h} row {y}: cut {c:?} in {row:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    fn own_profile() -> Profile {
        serde_json::from_value(json!({
            "did": "did:plc:me", "handle": "me.test",
            "displayName": "👨\u{200d}👩\u{200d}👧 Me 🇯🇵",
            "description": "مرحبا 1️⃣ 今日は👍🏽",
            "followersCount": 12, "followsCount": 3, "postsCount": 42
        }))
        .unwrap()
    }

    /// Your own profile says that e and s exist, as buttons; someone
    /// else's has neither.
    #[test]
    fn your_own_profile_shows_the_edit_and_settings_buttons() {
        let (mut app, _) = App::new(Some(session()), "x");
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('4'),
        ));
        app.handle_event(Event::Profile(Ok((own_profile(), posts(1).into()))));
        let screen = render(&mut app, 100, 24);
        assert!(
            screen.contains("this is you  [ e Edit profile ]  [ s Settings ]"),
            "{screen}"
        );
        assert!(screen.contains("s settings"), "{screen}");
        // Still there with the screen open over it.
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('s'),
        ));
        let screen = render(&mut app, 100, 30);
        assert!(screen.contains("[ s Settings ]"), "{screen}");
        assert!(screen.contains(" Settings "), "{screen}");
    }

    /// The screen names every setting with its value, and says where the
    /// selected one comes from.
    #[test]
    fn the_settings_screen_lists_each_setting_and_where_it_comes_from() {
        let (mut app, _) = App::new(Some(session()), "x");
        app.env.graphics = Some("kitty".into());
        app.env.browser = Some("firefox".into());
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('4'),
        ));
        app.handle_event(Event::Profile(Ok((own_profile(), posts(1).into()))));
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('s'),
        ));
        let screen = render(&mut app, 100, 30);
        for row in app.settings_rows() {
            assert!(screen.contains(row.name), "{}:\n{screen}", row.name);
        }
        assert!(screen.contains("▶ Theme"), "{screen}");
        assert!(screen.contains("bluesky"), "{screen}");
        assert!(screen.contains("Pictures         kitty"), "{screen}");
        assert!(screen.contains("Browser          firefox"), "{screen}");
        assert!(
            screen.contains("enter chooses one from the list"),
            "{screen}"
        );
        assert!(
            screen.contains("j k move  enter change  esc close"),
            "{screen}"
        );
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('j'),
        ));
        let screen = render(&mut app, 100, 30);
        assert!(
            screen.contains("set by BSKY_GRAPHICS for this run"),
            "{screen}"
        );
    }

    /// Typing a setting shows the line it is typed in, and choosing a
    /// folder shows the folder browser, which lists folders only.
    #[test]
    fn a_setting_is_typed_on_its_own_line_and_a_folder_chosen_in_the_browser() {
        use crossterm::event::{KeyCode, KeyEvent};
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("写真👨\u{200d}👩\u{200d}👧")).unwrap();
        std::fs::write(dir.path().join("photo.png"), "not listed").unwrap();
        let (mut app, _) = App::new(Some(session()), "x");
        app.settings.download_dir = Some(dir.path().display().to_string());
        app.handle_key(KeyEvent::from(KeyCode::Char('4')));
        app.handle_key(KeyEvent::from(KeyCode::Char('s')));
        for _ in 0..4 {
            app.handle_key(KeyEvent::from(KeyCode::Char('j')));
        }
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        let screen = render(&mut app, 100, 30);
        assert!(
            screen.contains("enter keeps it, empty is the default, esc cancels"),
            "{screen}"
        );
        assert!(screen.contains("https://video.bsky.app"), "{screen}");
        assert!(screen.contains("enter keep  esc cancel"), "{screen}");
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        for _ in 0..2 {
            app.handle_key(KeyEvent::from(KeyCode::Char('k')));
        }
        let screen = render(&mut app, 100, 30);
        assert!(screen.contains("x goes back to the default"), "{screen}");
        assert!(screen.contains("x default"), "{screen}");
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        let screen = render_text_only(&mut app, 100, 30);
        assert!(screen.contains("Choose a folder"), "{screen}");
        assert!(screen.contains("写真👨\u{200d}👩\u{200d}👧/"), "{screen}");
        assert!(!screen.contains("photo.png"), "{screen}");
        assert!(
            screen.contains("enter open  space choose this folder"),
            "{screen}"
        );
    }

    /// A long path keeps its end, where the folder's own name is, and the
    /// selected row stays on a screen too short for the whole list.
    #[test]
    fn a_short_screen_keeps_the_selected_setting_in_sight() {
        let (mut app, _) = App::new(Some(session()), "x");
        app.env.download_dir = Some(format!(
            "/very/{}/写真👨\u{200d}👩\u{200d}👧",
            "long/".repeat(20)
        ));
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('4'),
        ));
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('s'),
        ));
        for _ in 0..5 {
            app.handle_key(crossterm::event::KeyEvent::from(
                crossterm::event::KeyCode::Char('j'),
            ));
        }
        let screen = render_text_only(&mut app, 40, MIN_H);
        assert!(screen.contains("▶ Browser"), "{screen}");
        for _ in 0..3 {
            app.handle_key(crossterm::event::KeyEvent::from(
                crossterm::event::KeyCode::Char('j'),
            ));
        }
        let screen = render_text_only(&mut app, 40, 12);
        assert!(screen.contains("▶ Download folder"), "{screen}");
        assert!(screen.contains("写真👨\u{200d}👩\u{200d}👧"), "{screen}");
    }

    /// The key column is 18 cells wide, which leaves nothing for the
    /// description on a narrow screen: the help was the one screen that
    /// could not be read where it is needed most.
    #[test]
    fn help_on_a_narrow_screen_puts_each_description_under_its_keys() {
        let (mut app, _) = App::new(Some(session()), "x");
        app.handle_event(Event::Timeline(Ok(posts(1).into())));
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('?'),
        ));
        let narrow = render(&mut app, MIN_W, 16);
        assert!(narrow.contains("1 2 3 4"), "{narrow}");
        // The words are whole and on their own rows, not cut to four cells.
        assert!(narrow.contains("Timeline,"), "{narrow}");
        assert!(narrow.contains("Search,"), "{narrow}");
        assert!(!narrow.contains("Time\n"), "{narrow}");
        // The two columns come back where they fit, and a description too
        // long for its column wraps rather than losing its end.
        let forty = render(&mut app, 40, 16);
        assert!(
            forty.contains("1 2 3 4        Timeline, Search,"),
            "{forty}"
        );
        assert!(
            forty
                .lines()
                .any(|l| l.contains("Notifications,") && !l.contains("1 2 3 4")),
            "{forty}"
        );
        assert!(
            forty
                .lines()
                .any(|l| l.trim_matches('│').trim() == "Profile"),
            "{forty}"
        );
        let wide = render(&mut app, 80, 24);
        assert!(
            wide.contains("1 2 3 4        Timeline, Search, Notifications, Profile"),
            "{wide}"
        );
    }

    /// A terminal that draws no pictures says what a quote carries, the
    /// same way it does for a post's own pictures.
    #[test]
    fn a_quote_of_a_picture_post_says_what_it_carries_as_text() {
        let post: Post = serde_json::from_value(json!({
            "uri": "at://p/q", "cid": "c",
            "author": {"did": "did:plc:a", "handle": "alice.test", "displayName": "Alice"},
            "record": {"text": "look at this", "createdAt": "2026-09-22T00:00:00Z"},
            "embed": {"$type": "app.bsky.embed.record#view", "record": {
                "$type": "app.bsky.embed.record#viewRecord",
                "uri": "at://did:plc:bob/app.bsky.feed.post/q", "cid": "cq",
                "author": {"did": "did:plc:bob", "handle": "bob.test"},
                "value": {"text": "from the top"},
                "embeds": [{"$type": "app.bsky.embed.images#view", "images": [
                    {"thumb": "https://t/1", "fullsize": "https://f/1", "alt": "山の頂上👨\u{200d}👩\u{200d}👧"}
                ]}]
            }}
        }))
        .unwrap();
        let (mut app, _) = App::new(Some(session()), "x");
        app.without_pictures();
        app.handle_event(Event::Timeline(Ok(vec![post].into())));
        let screen = render_text_only(&mut app, 80, 24);
        assert!(screen.contains("❝ @bob.test: from the top"), "{screen}");
        assert!(
            screen.contains("▣ 1 picture: 山の頂上👨\u{200d}👩\u{200d}👧"),
            "{screen}"
        );
    }

    /// A quote post says whose post it quotes. Beside a picture the quote
    /// used to be dropped, and a quote of a post that is gone or blocked
    /// showed nothing at all, which reads as an empty post.
    #[test]
    fn a_quote_names_who_it_quotes_beside_a_picture_and_says_when_it_cannot() {
        let quote_of = |embed: serde_json::Value| -> Post {
            serde_json::from_value(json!({
                "uri": "at://p/q", "cid": "c",
                "author": {"did": "did:plc:a", "handle": "alice.test", "displayName": "Alice"},
                "record": {"text": "look at this", "createdAt": "2026-09-22T00:00:00Z"},
                "embed": embed,
            }))
            .unwrap()
        };
        let viewed = json!({
            "$type": "app.bsky.embed.record#viewRecord",
            "uri": "at://did:plc:bob/app.bsky.feed.post/q", "cid": "c",
            "author": {"did": "did:plc:bob", "handle": "bob.test"},
            "value": {"text": "今日は👨\u{200d}👩\u{200d}👧 the quoted words"}
        });
        let with_picture = quote_of(json!({
            "$type": "app.bsky.embed.recordWithMedia#view",
            "record": {"$type": "app.bsky.embed.record#view", "record": viewed},
            "media": {"$type": "app.bsky.embed.images#view", "images": [
                {"thumb": "https://t/1", "fullsize": "https://f/1", "alt": ""}
            ]}
        }));
        let gone = quote_of(json!({
            "$type": "app.bsky.embed.record#view",
            "record": {"$type": "app.bsky.embed.record#viewNotFound",
                       "uri": "at://did:plc:bob/app.bsky.feed.post/x", "notFound": true}
        }));
        let blocked = quote_of(json!({
            "$type": "app.bsky.embed.record#view",
            "record": {"$type": "app.bsky.embed.record#viewBlocked",
                       "uri": "at://did:plc:bob/app.bsky.feed.post/y", "blocked": true,
                       "author": {"did": "did:plc:bob"}}
        }));
        let (mut app, _) = App::new(Some(session()), "x");
        app.handle_event(Event::Timeline(
            Ok(vec![with_picture, gone, blocked].into()),
        ));
        let screen = render_text_only(&mut app, 80, 40);
        assert!(
            screen.contains("❝ @bob.test: 今日は👨\u{200d}👩\u{200d}👧 the quoted words"),
            "{screen}"
        );
        assert!(screen.contains("quoted post not found"), "{screen}");
        assert!(
            screen.contains("quoted post from an account you cannot see"),
            "{screen}"
        );
    }

    /// The actions list offers D only where it works: on a post of your own.
    #[test]
    fn the_actions_list_offers_delete_only_on_your_own_post() {
        let (mut app, _) = App::new(Some(session()), "x");
        let mine: Post = serde_json::from_value(json!({
            "uri": "at://did:plc:me/app.bsky.feed.post/mine", "cid": "c",
            "author": {"did": "did:plc:me", "handle": "me.test", "displayName": "Me"},
            "record": {"text": "my own post", "createdAt": "2026-09-22T00:00:00Z"},
        }))
        .unwrap();
        app.handle_event(Event::Timeline(Ok(vec![posts(1).remove(0), mine].into())));
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('.'),
        ));
        let screen = render(&mut app, 100, 24);
        assert!(screen.contains("Actions"), "{screen}");
        assert!(!screen.contains("delete your post"), "{screen}");
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Esc,
        ));
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('j'),
        ));
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('.'),
        ));
        let screen = render(&mut app, 100, 24);
        assert!(screen.contains("D      delete your post"), "{screen}");
        // The help says how it is confirmed.
        app.overlay = Some(Overlay::Help { scroll: 0 });
        let help = render(&mut app, 100, 40);
        assert!(help.contains("delete your own post"), "{help}");
    }

    #[test]
    fn tiny_terminals_do_not_panic() {
        let (mut app, _) = App::new(Some(session()), "x");
        app.handle_event(Event::Timeline(Ok(posts(3).into())));
        for (w, h) in [(1, 1), (5, 3), (10, 2), (20, 5)] {
            render(&mut app, w, h);
        }
        app.overlay = Some(Overlay::Help { scroll: 0 });
        render(&mut app, 8, 4);
        let (mut login, _) = App::new(None, "x");
        render(&mut login, 3, 3);
    }

    #[test]
    fn image_rows_follow_the_aspect_ratio_within_bounds() {
        // 36 cells of 10px is 360px wide; 4:3 is 270px tall, 14 rows of 20px,
        // capped at the maximum.
        assert_eq!(image_rows(Some((4, 3)), 36, (10, 20)), IMAGE_ROWS_MAX);
        // 16:9 at 36 cells: 202.5px, 11 rows.
        assert_eq!(image_rows(Some((16, 9)), 36, (10, 20)), 11);
        // A panorama still gets a visible band.
        assert_eq!(image_rows(Some((10, 1)), 36, (10, 20)), IMAGE_ROWS_MIN);
        assert_eq!(image_rows(None, 36, (10, 20)), IMAGE_ROWS);
        // A degenerate cell size does not divide by zero.
        assert_eq!(image_rows(Some((1, 1)), 10, (0, 0)), 10);
    }

    #[test]
    fn image_boxes_share_the_width() {
        assert_eq!(image_box_width(80, 1), IMAGE_MAX_W);
        assert_eq!(image_box_width(61, 4), 14);
        assert_eq!(image_box_width(3, 4), 0);
        assert_eq!(image_box_width(80, 0), 0);
    }

    #[test]
    fn hints_stay_visible_while_a_status_message_shows() {
        let (mut app, _) = App::new(Some(session()), "x");
        app.handle_event(Event::Timeline(Ok(posts(2).into())));
        app.handle_event(Event::Liked {
            post_uri: "at://p/0".into(),
            result: Ok("at://l".into()),
        });
        let screen = render(&mut app, 100, 24);
        assert!(screen.contains("liked"), "{screen}");
        assert!(screen.contains("? help"), "{screen}");
    }

    #[test]
    fn help_is_clamped_to_its_last_page() {
        let (mut app, _) = App::new(Some(session()), "x");
        app.overlay = Some(Overlay::Help { scroll: u16::MAX });
        let screen = render(&mut app, 80, 16);
        assert!(screen.contains("close"), "{screen}");
        let Some(Overlay::Help { scroll }) = app.overlay else {
            panic!()
        };
        // No more rows than three per key (a description wraps onto two at
        // most at this width) and two per section.
        let rows: usize = keys::HELP.iter().map(|s| s.keys.len() * 3 + 2).sum();
        assert!(
            usize::from(scroll) < rows,
            "scroll was not clamped: {scroll}"
        );
        // The last section is on screen after scrolling to the end.
        assert!(screen.contains("scroll"), "{screen}");
    }

    fn render_buffer(app: &mut App, w: u16, h: u16) -> ratatui::buffer::Buffer {
        let mut images = Images::new(Picker::halfblocks(), None);
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw(f, app, &mut images)).unwrap();
        term.backend().buffer().clone()
    }

    #[test]
    fn every_theme_draws_every_view_without_panicking() {
        use crate::tui::theme::{ColorDepth, THEMES};
        for depth in [ColorDepth::TrueColor, ColorDepth::Ansi256] {
            for (i, theme) in THEMES.iter().enumerate() {
                let (mut app, _) = App::new(Some(session()), "x");
                app.apply_settings(
                    crate::config::Settings {
                        theme: Some(theme.name.into()),
                        ..Default::default()
                    },
                    depth,
                    None,
                );
                assert_eq!(app.theme_index, i);
                app.handle_event(Event::Timeline(Ok(posts(3).into())));
                for overlay in [
                    None,
                    Some(Overlay::Help { scroll: 0 }),
                    Some(Overlay::Themes {
                        selected: i,
                        previous: 0,
                    }),
                ] {
                    app.overlay = overlay;
                    for (w, h) in [(100, 30), (MIN_W, MIN_H), (20, 6), (1, 1)] {
                        render(&mut app, w, h);
                    }
                }
            }
        }
    }

    #[test]
    fn a_theme_with_a_background_paints_the_whole_screen() {
        let (mut app, _) = App::new(Some(session()), "x");
        app.apply_settings(
            crate::config::Settings {
                theme: Some("dracula".into()),
                ..Default::default()
            },
            crate::tui::theme::ColorDepth::TrueColor,
            None,
        );
        let buf = render_buffer(&mut app, 40, 10);
        // An empty cell far from any text still has the theme's background.
        assert_eq!(
            buf[(39, 5)].bg,
            ratatui::style::Color::Rgb(0x28, 0x2a, 0x36)
        );
    }

    #[test]
    fn monochrome_draws_no_color_at_all() {
        let (mut app, _) = App::new(Some(session()), "x");
        app.apply_settings(
            Default::default(),
            crate::tui::theme::ColorDepth::None,
            None,
        );
        app.handle_event(Event::Timeline(Ok(posts(3).into())));
        app.overlay = Some(Overlay::Help { scroll: 0 });
        let buf = render_buffer(&mut app, 100, 30);
        for cell in buf.content() {
            assert_eq!(cell.fg, ratatui::style::Color::Reset, "{cell:?}");
            assert_eq!(cell.bg, ratatui::style::Color::Reset, "{cell:?}");
        }
    }

    #[test]
    fn the_picker_shows_a_window_of_themes_even_on_a_tall_screen() {
        let (mut app, _) = App::new(Some(session()), "x");
        let nord = crate::tui::theme::index_of("nord").unwrap();
        app.overlay = Some(Overlay::Themes {
            selected: nord,
            previous: 0,
        });
        let screen = render(&mut app, 100, 60);
        assert!(screen.contains("▶ nord"), "{screen}");
        let shown = THEMES
            .iter()
            .filter(|t| screen.contains(&format!(" {} ", t.name)))
            .count();
        assert!(shown <= THEME_ROWS + 1, "{shown} themes drawn:\n{screen}");
        assert!(screen.contains('▲') && screen.contains('▼'), "{screen}");
        // Every theme is reached by scrolling.
        for (i, theme) in THEMES.iter().enumerate() {
            app.overlay = Some(Overlay::Themes {
                selected: i,
                previous: 0,
            });
            let screen = render(&mut app, 100, 60);
            assert!(
                screen.contains(&format!("▶ {}", theme.name)),
                "{}",
                theme.name
            );
        }
    }

    #[test]
    fn an_error_is_shown_in_the_middle_until_a_key() {
        let (mut app, _) = App::new(Some(session()), "x");
        app.handle_event(Event::Timeline(Ok(Vec::new().into())));
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('f'),
            crossterm::event::KeyModifiers::NONE,
        ));
        app.status = Some(crate::tui::app::Status {
            text: "something went wrong".into(),
            error: true,
            at: std::time::Instant::now(),
        });
        let screen = render(&mut app, 80, 24);
        let rows: Vec<&str> = screen.lines().collect();
        let row = rows
            .iter()
            .position(|l| l.contains("something went wrong"))
            .expect("shown");
        assert!(
            (8..16).contains(&row),
            "in the middle, not the bottom:\n{screen}"
        );
        assert!(!rows[23].contains("something went wrong"));
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('j'),
            crossterm::event::KeyModifiers::NONE,
        ));
        assert!(!render(&mut app, 80, 24).contains("something went wrong"));
    }

    #[test]
    fn the_picker_scrolls_to_keep_the_selection_on_a_short_screen() {
        let (mut app, _) = App::new(Some(session()), "x");
        let last = THEMES.len() - 1;
        app.overlay = Some(Overlay::Themes {
            selected: last,
            previous: 0,
        });
        let screen = render(&mut app, 80, 20);
        assert!(screen.contains("▶ monochrome"), "{screen}");
        assert!(
            screen.contains(&format!("{}/{}", last + 1, THEMES.len())),
            "{screen}"
        );
        assert!(
            !screen.contains("  bluesky "),
            "the top has scrolled away:\n{screen}"
        );
    }

    #[test]
    fn a_reply_shows_its_thread_above_it() {
        let (mut app, _) = App::new(Some(session()), "x");
        let mut reply = posts(1).remove(0);
        let parent: crate::api::types::RefPost = serde_json::from_value(json!({
            "$type": "app.bsky.feed.defs#postView", "uri": "at://parent", "cid": "c",
            "author": {"did": "d", "handle": "carol.test", "displayName": "Carol"},
            "record": {"text": "the question\nsecond line"}
        }))
        .unwrap();
        reply.context = Some(Box::new(crate::api::types::ReplyContext {
            root: Some(crate::api::types::RefPost::NotFound {
                uri: "at://root".into(),
            }),
            gap: true,
            parent,
        }));
        app.handle_event(Event::Timeline(Ok(vec![reply].into())));
        let screen = render(&mut app, 80, 24);
        let rows: Vec<&str> = screen.lines().collect();
        let at = |s: &str| {
            rows.iter()
                .position(|r| r.contains(s))
                .unwrap_or_else(|| panic!("{s}:\n{screen}"))
        };
        assert!(at("(post not found)") < at("┆ ⋮"));
        assert!(at("┆ ⋮") < at("Carol @carol.test: the question"));
        assert!(at("Carol @carol.test: the question") < at("post number 0"));
        assert!(!screen.contains("second line"), "one line per post above");
    }

    #[test]
    fn a_thread_indents_replies_and_shows_placeholders() {
        let (mut app, _) = App::new(Some(session()), "x");
        let node: crate::api::types::ThreadNode = serde_json::from_value(json!({
            "$type": "app.bsky.feed.defs#threadViewPost",
            "post": {"uri": "at://focus", "cid": "c", "author": {"did": "d", "handle": "a.test"}, "record": {"text": "the focus"}},
            "parent": {"$type": "app.bsky.feed.defs#notFoundPost", "uri": "at://gone", "notFound": true},
            "replies": [{"$type": "app.bsky.feed.defs#threadViewPost",
                "post": {"uri": "at://r1", "cid": "c", "author": {"did": "d", "handle": "b.test"}, "record": {"text": "a reply"}},
                "replies": []}]
        }))
        .unwrap();
        let (rows, focus) = crate::tui::thread::flatten(node);
        app.threads.push(crate::tui::app::ThreadView {
            uri: "at://focus".into(),
            list: List {
                items: rows,
                selected: focus,
                loaded: true,
                ..List::default()
            },
            error: None,
        });
        let screen = render(&mut app, 80, 24);
        assert!(screen.contains("Thread"), "{screen}");
        assert!(screen.contains("(post not found)"), "{screen}");
        let focus_line = screen.lines().find(|l| l.contains("the focus")).unwrap();
        let reply_line = screen.lines().find(|l| l.contains("a reply")).unwrap();
        let col = |l: &str, s: &str| l.find(s).unwrap();
        assert!(
            col(reply_line, "a reply") > col(focus_line, "the focus"),
            "the reply is indented:\n{screen}"
        );
        assert!(reply_line.contains('│'), "with a guide line:\n{screen}");
    }

    #[test]
    fn notifications_say_who_did_what_and_mark_the_unread() {
        let (mut app, _) = App::new(Some(session()), "x");
        let n = |reason: &str, read: bool| crate::tui::worker::NotifItem {
            n: serde_json::from_value(json!({
                "uri": format!("at://{reason}"), "reason": reason, "isRead": read,
                "author": {"did": "d", "handle": format!("{reason}.test"), "displayName": "Eve"},
                "indexedAt": "2026-09-22T00:00:00.000Z"
            }))
            .unwrap(),
            post: None,
            subject: (reason == "like").then(|| posts(1).remove(0)),
            fresh: !read,
        };
        app.tab = Tab::Notifications;
        app.unread = 1;
        app.notifications = List {
            items: vec![
                n("like", false),
                n("follow", true),
                n("starterpack-joined", true),
                n("brand-new", true),
            ],
            loaded: true,
            ..List::default()
        };
        let screen = render(&mut app, 100, 24);
        assert!(screen.contains("3 Notifications (1)"), "{screen}");
        assert!(
            screen.contains("● Eve @like.test liked your post"),
            "{screen}"
        );
        assert!(
            screen.contains("post number 0"),
            "the liked post is quoted:\n{screen}"
        );
        assert!(screen.contains("Eve @follow.test followed you"), "{screen}");
        assert!(!screen.contains("● Eve @follow.test"), "{screen}");
        assert!(
            screen.contains("joined through your starter pack"),
            "{screen}"
        );
        assert!(
            screen.contains("(brand-new)"),
            "an unknown reason is still shown:\n{screen}"
        );
    }

    #[test]
    fn scroll_moves_offset_only_as_far_as_needed() {
        let h = |_| 3;
        assert_eq!(scroll_offset(3, 0, 100, 7, h), 2);
        assert_eq!(scroll_offset(1, 2, 100, 7, h), 1, "up past the top");
        assert_eq!(scroll_offset(2, 1, 100, 7, h), 1, "already on screen");
        assert_eq!(scroll_offset(4, 4, 100, 7, h), 4);
        // A post taller than the screen is drawn from its top.
        assert_eq!(
            scroll_offset(2, 0, 100, 7, |i| if i == 2 { 20 } else { 3 }),
            2
        );
    }

    /// Room left below the last post is used for the posts above: after the
    /// terminal grows, or near the end of a list, the screen is filled from
    /// the bottom instead of leaving the first posts hidden.
    #[test]
    fn a_list_that_ends_on_screen_fills_it_from_above() {
        let h = |_| 3;
        // Five posts of 3 rows; the last three were on a 9-row screen.
        assert_eq!(scroll_offset(3, 2, 5, 9, h), 2, "the end just fills it");
        assert_eq!(scroll_offset(3, 2, 5, 15, h), 0, "grown: all five fit");
        assert_eq!(scroll_offset(4, 3, 5, 12, h), 1, "grown: four fit");
        // Not past what keeps the selection on screen, and not for a list
        // that goes on below the screen.
        assert_eq!(scroll_offset(1, 1, 50, 9, h), 1);
        assert_eq!(scroll_offset(0, 0, 5, 30, h), 0);
    }

    #[test]
    fn jumping_to_the_end_measures_only_what_fits() {
        let mut measured = Vec::new();
        let offset = scroll_offset(9_999, 0, 10_000, 10, |i| {
            measured.push(i);
            3
        });
        assert_eq!(offset, 9_997);
        // Only the posts near the end are measured (the caller caches the
        // heights, so asking for one again costs nothing).
        measured.sort_unstable();
        measured.dedup();
        assert_eq!(measured, [9_996, 9_997, 9_998, 9_999]);
    }

    /// The composer on the smallest screen the client draws in: the box has
    /// no room for the thumbnails, and the names of the pictures that will
    /// be sent must take their rows rather than fall off the bottom.
    #[test]
    fn a_short_composer_still_lists_every_picture_it_would_send() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _) = App::new(Some(session()), "x");
        app.handle_event(Event::Timeline(Ok(posts(1).into())));
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('n'),
        ));
        let Some(Overlay::Compose(c)) = &mut app.overlay else {
            panic!()
        };
        for i in 0..4 {
            c.media.push(crate::tui::app::Attached::new(
                dir.path()
                    .join(format!("写真👨\u{200d}👩\u{200d}👧{i}.png")),
            ));
        }
        // The cells a wide character covers are skipped, so the names read
        // as the terminal shows them.
        let screen = render_text_only(&mut app, MIN_W, MIN_H);
        for i in 1..=4 {
            assert!(
                screen.contains(&format!(" {i} 写真👨\u{200d}👩\u{200d}👧{}", i - 1)),
                "{i}: {screen}"
            );
        }
    }

    /// A reply loses two more rows to the post it answers, so not every
    /// name fits. The ones that do not are counted rather than dropped.
    #[test]
    fn a_composer_too_short_for_every_name_counts_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _) = App::new(Some(session()), "x");
        app.handle_event(Event::Timeline(Ok(posts(1).into())));
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('r'),
        ));
        let Some(Overlay::Compose(c)) = &mut app.overlay else {
            panic!()
        };
        for i in 0..4 {
            c.media.push(crate::tui::app::Attached::new(
                dir.path().join(format!("pic{i}.png")),
            ));
        }
        let screen = render(&mut app, MIN_W, MIN_H);
        assert!(screen.contains(" 1 pic0.png"), "{screen}");
        assert!(screen.contains("and 3 more"), "{screen}");
    }

    #[test]
    fn the_composer_lists_its_pictures_with_their_alt_text() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _) = App::new(Some(session()), "https://bsky.social");
        app.handle_event(Event::Timeline(Ok(Vec::new().into())));
        app.browse_from = Some(dir.path().to_path_buf());
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('n'),
            crossterm::event::KeyModifiers::NONE,
        ));
        let Some(Overlay::Compose(c)) = &mut app.overlay else {
            panic!()
        };
        c.media
            .push(crate::tui::app::Attached::new(dir.path().join("cat.png")));
        let screen = render(&mut app, 100, 40);
        assert!(
            screen.contains("1 cat.png  alt: (none; tab to describe it)"),
            "{screen}"
        );
        assert!(screen.contains("tab alt text"), "{screen}");
    }

    /// Folder and file names come from the user's disk and are often
    /// Japanese with emoji: the browser's path line, its list, the preview's
    /// caption, and the composer's attachment list never cut one apart, at
    /// any width.
    #[test]
    fn emoji_in_folder_and_file_names_are_never_cut_apart() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir
            .path()
            .join("家族👨\u{200d}👩\u{200d}👧\u{200d}👦の旅行🇯🇵");
        std::fs::create_dir(&folder).unwrap();
        std::fs::create_dir(folder.join("サブ👍🏽フォルダ")).unwrap();
        let name = "👍🏽いいね写真_with_a_rather_long_name_1️⃣.png";
        image::RgbImage::from_pixel(30, 20, image::Rgb([5, 5, 5]))
            .save(folder.join(name))
            .unwrap();
        for width in (20u16..=100).step_by(3) {
            let (mut app, _) = App::new(Some(session()), "https://bsky.social");
            app.handle_event(Event::Timeline(Ok(Vec::new().into())));
            app.browse_from = Some(folder.clone());
            for (c, m) in [
                ('n', crossterm::event::KeyModifiers::NONE),
                ('o', crossterm::event::KeyModifiers::CONTROL),
                ('j', crossterm::event::KeyModifiers::NONE),
                ('j', crossterm::event::KeyModifiers::NONE),
            ] {
                app.handle_key(crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Char(c),
                    m,
                ));
            }
            let rows = cells(&mut app, width, 30);
            for row in &rows {
                for c in row {
                    assert!(!is_fragment(c), "{width}: a cut cluster {c:?} in {row:?}");
                }
            }
            if width >= 98 {
                let screen: String = rows.iter().map(|r| r.concat() + "\n").collect();
                assert!(
                    screen.contains("家族👨\u{200d}👩\u{200d}👧\u{200d}👦の旅行🇯🇵"),
                    "{screen}"
                );
                assert!(screen.contains("サブ👍🏽フォルダ/"), "{screen}");
                assert!(screen.contains("👍🏽いいね写真"), "{screen}");
            }
            // Chosen, the picture is listed in the composer by its name.
            app.handle_key(crossterm::event::KeyEvent::from(
                crossterm::event::KeyCode::Enter,
            ));
            for row in cells(&mut app, width, 30) {
                for c in &row {
                    assert!(
                        !is_fragment(c),
                        "{width}: composer: a cut cluster {c:?} in {row:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_browser_shows_the_folder_its_entries_and_the_selection() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("trips")).unwrap();
        image::RgbImage::from_pixel(30, 20, image::Rgb([5, 5, 5]))
            .save(dir.path().join("snow.png"))
            .unwrap();
        let (mut app, _) = App::new(Some(session()), "https://bsky.social");
        app.handle_event(Event::Timeline(Ok(Vec::new().into())));
        app.browse_from = Some(dir.path().to_path_buf());
        for (c, m) in [
            ('n', crossterm::event::KeyModifiers::NONE),
            ('o', crossterm::event::KeyModifiers::CONTROL),
        ] {
            app.handle_key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char(c),
                m,
            ));
        }
        let screen = render(&mut app, 100, 30);
        assert!(
            screen.contains("Attach pictures (up to 4) or a video"),
            "{screen}"
        );
        assert!(screen.contains("trips/"), "{screen}");
        assert!(screen.contains("enter opens the folder"), "{screen}");
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('j'),
            crossterm::event::KeyModifiers::NONE,
        ));
        let screen = render(&mut app, 100, 30);
        assert!(screen.contains("30×20 ·"), "{screen}");
        assert!(screen.contains("space mark"), "{screen}");
    }

    #[rstest::rstest]
    #[case(0, "0 B")]
    #[case(1023, "1023 B")]
    #[case(2048, "2 KB")]
    #[case(3 * 1024 * 1024 / 2, "1.5 MB")]
    fn byte_counts_read_like_a_person_would_say_them(#[case] n: u64, #[case] want: &str) {
        assert_eq!(human_bytes(n), want);
    }

    #[test]
    fn list_avatars_use_the_small_bluesky_version() {
        let full = "https://cdn.bsky.app/img/avatar/plain/did:plc:x/bafy@jpeg";
        assert_eq!(
            small_avatar(full),
            "https://cdn.bsky.app/img/avatar_thumbnail/plain/did:plc:x/bafy@jpeg"
        );
        assert_eq!(
            small_avatar("http://127.0.0.1/img/a.png"),
            "http://127.0.0.1/img/a.png"
        );
    }

    #[test]
    fn hints_wrap_whole_onto_more_rows_when_narrow() {
        let t = THEMES[0];
        let hints: Vec<keys::Hint> = vec![
            ("?", "help"),
            ("j k", "move"),
            ("enter", "profile"),
            ("q", "quit"),
        ];
        let text = |lines: &[Line]| -> Vec<String> {
            lines
                .iter()
                .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
                .collect()
        };
        assert_eq!(
            text(&hint_lines(&hints, 80, &t)),
            [" ? help  j k move  enter profile  q quit"]
        );
        assert_eq!(
            text(&hint_lines(&hints, 20, &t)),
            [" ? help  j k move", " enter profile", " q quit"]
        );
    }

    /// The hints are one row, whatever the width: a narrow screen keeps the
    /// ones that fit, whole, starting with the help key and the list of
    /// what can be done here, which lead to all the others.
    #[test]
    fn the_hint_row_is_one_line_and_starts_with_the_keys_that_lead_on() {
        let (mut app, _) = App::new(Some(session()), "x");
        app.handle_event(Event::Timeline(Ok(posts(3).into())));
        for width in [MIN_W, 40, 50, 80, 120] {
            let screen = render(&mut app, width, 24);
            let rows: Vec<&str> = screen.lines().filter(|l| l.contains("? help")).collect();
            assert_eq!(rows.len(), 1, "{width}:\n{screen}");
            assert!(
                rows[0].trim_start().starts_with("? help"),
                "{width}: {rows:?}"
            );
            if width >= 40 {
                assert!(rows[0].contains(". actions"), "{width}: {rows:?}");
            }
        }
        // On a wide screen every hint of the view is on that one row.
        let screen = render(&mut app, 120, 24);
        for (key, what) in keys::hints(&app) {
            assert!(
                screen.contains(&format!("{key} {what}")),
                "{key} {what}:\n{screen}"
            );
        }
    }

    // A header is cut as one line: one ellipsis where it stops, even when
    // a wide character leaves a single column, and one whenever anything
    // after the cut is dropped.
    #[rstest::rstest]
    #[case(&["日本語", " @alice"], 4, "日…")]
    #[case(&["日本語", " @alice"], 5, "日本…")]
    #[case(&["Alice", " @alice"], 5, "Alic…")]
    #[case(&["Alice", " @alice"], 12, "Alice @alice")]
    #[case(&["👨‍👩‍👧👨‍👩‍👧", " @a"], 3, "👨‍👩‍👧…")]
    #[case(&["🇯🇵 name", " · 2026"], 8, "🇯🇵 name…")]
    fn a_cut_header_ends_in_one_ellipsis(
        #[case] spans: &[&str],
        #[case] width: usize,
        #[case] want: &str,
    ) {
        let line = Line::from(
            spans
                .iter()
                .map(|s| Span::raw(s.to_string()))
                .collect::<Vec<_>>(),
        );
        let cut = truncate_line(line, width);
        let text: String = cut.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, want);
        assert!(text.width() <= width, "{text:?} is wider than {width}");
    }

    #[test]
    fn a_long_path_keeps_its_end() {
        assert_eq!(truncate_start("/home/me/pics", 20), "/home/me/pics");
        assert_eq!(truncate_start("/home/me/pictures/trips", 10), "…res/trips");
        // Wide characters count as two columns.
        assert_eq!(truncate_start("/ホーム/写真", 6), "…/写真");
        assert_eq!(truncate_start("/ホーム/写真", 5), "…写真");
    }

    #[test]
    fn a_long_path_is_not_cut_inside_a_grapheme_cluster() {
        assert_eq!(truncate_start("/pics/👍🏽👍🏽", 3), "…👍🏽");
        assert_eq!(truncate_start("/pics/👨‍👩‍👧", 3), "…👨‍👩‍👧");
        assert_eq!(truncate_start("/pics/🇯🇵🇯🇵", 3), "…🇯🇵");
    }

    /// Timeline posts the size real ones are: a few lines of mixed text, a
    /// reply's context, pictures, and an avatar.
    fn heavy_posts(n: usize) -> Vec<Post> {
        let text = "今日は山に登りました 🏔️ The view from the top was worth every step, \
                    and the clouds rolled in just as we left 👨‍👩‍👧 #hiking https://example.com/trip \
                    また行きたい！ @bob.test ";
        (0..n)
            .map(|i| {
                serde_json::from_value(json!({
                    "uri": format!("at://did:plc:a/app.bsky.feed.post/{i}"), "cid": "c",
                    "author": {"did": "did:plc:a", "handle": "alice.test", "displayName": "Alice 🌸",
                               "avatar": "http://127.0.0.1:9/avatar.jpg"},
                    "record": {"text": text.repeat(1 + i % 3), "createdAt": "2026-09-22T00:00:00Z",
                               "reply": if i % 4 == 0 { json!({
                                   "root": {"uri": "at://did:plc:b/app.bsky.feed.post/r", "cid": "c"},
                                   "parent": {"uri": "at://did:plc:b/app.bsky.feed.post/r", "cid": "c"}
                               }) } else { json!(null) }},
                    "embed": if i % 2 == 0 { json!({"$type": "app.bsky.embed.images#view", "images": [
                        {"thumb": format!("http://127.0.0.1:9/{i}a.jpg"), "fullsize": "x", "alt": "", "aspectRatio": {"width": 4, "height": 3}},
                        {"thumb": format!("http://127.0.0.1:9/{i}b.jpg"), "fullsize": "x", "alt": "", "aspectRatio": {"width": 3, "height": 4}}
                    ]}) } else { json!(null) },
                    "likeCount": i, "repostCount": 1, "replyCount": 2,
                }))
                .unwrap()
            })
            .collect()
    }

    /// Time to draw one frame of the timeline while `j` walks down 200 posts,
    /// the cost a held key pays per step. Prints the median and the slowest.
    #[test]
    #[ignore = "measurement"]
    #[cfg(not(coverage))]
    fn frame_time() {
        let mut runs = Vec::new();
        for _ in 0..5 {
            let (mut app, _) = App::new(Some(session()), "x");
            app.handle_event(Event::Timeline(Ok(heavy_posts(200).into())));
            let mut images = Images::new(Picker::halfblocks(), None);
            let mut term = Terminal::new(TestBackend::new(120, 50)).unwrap();
            let mut times = Vec::new();
            for _ in 0..500 {
                app.handle_key(crossterm::event::KeyEvent::from(
                    crossterm::event::KeyCode::Char(if times.len() % 250 < 199 {
                        'j'
                    } else {
                        'k'
                    }),
                ));
                let start = std::time::Instant::now();
                term.draw(|f| draw(f, &mut app, &mut images)).unwrap();
                times.push(start.elapsed());
            }
            times.sort();
            runs.push((times[times.len() / 2], times[times.len() - 1]));
        }
        for (median, max) in runs {
            println!("frame: median {median:?}, max {max:?}");
        }
    }
}

/// Random keys and random, late, reordered answers from a stand-in worker,
/// thousands of steps per seed: the client must not panic, must keep every
/// selection inside its list, must never send more than one write for one
/// key, and must draw at any terminal size.
#[cfg(test)]
mod state_fuzz {
    use super::*;
    use crate::api::types::{Profile, ThreadNode};
    use crate::config::Session;
    use crate::error::Error;
    use crate::tui::worker::{Event, Feed, Job, MorePage, Page};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui_image::picker::Picker;
    use serde_json::json;

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            // xorshift64*: a failing seed can be replayed.
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
        fn chance(&mut self, percent: u64) -> bool {
            self.next() % 100 < percent
        }
    }

    const TEXTS: &[&str] = &[
        "hello",
        "👨‍👩‍👧‍👦 family 🇯🇵",
        "日本語の投稿 #タグ https://example.com",
        "e\u{301}t\u{e9} 1️⃣ ❤️ 👍🏽",
        "",
        "a very long line that keeps going and going so that it has to wrap over several rows of the screen",
    ];
    const AUTHORS: &[&str] = &["did:plc:me", "did:plc:alice", "did:plc:bob"];

    fn post(rng: &mut Rng, n: u64) -> Post {
        let author = AUTHORS[rng.below(AUTHORS.len())];
        let mut v = json!({
            "uri": format!("at://{author}/app.bsky.feed.post/p{n}"),
            "cid": format!("c{n}"),
            "author": {"did": author, "handle": format!("{}.test", &author[8..]), "displayName": TEXTS[rng.below(TEXTS.len())]},
            "record": {"text": TEXTS[rng.below(TEXTS.len())], "createdAt": "2026-09-22T00:00:00Z"},
            "likeCount": rng.below(5),
            "indexedAt": "2026-09-22T00:00:00Z",
        });
        if rng.chance(50) {
            v["author"]["viewer"] =
                json!({"following": format!("at://did:plc:me/app.bsky.graph.follow/{n}")});
        }
        if rng.chance(30) {
            v["viewer"] = json!({"like": format!("at://did:plc:me/app.bsky.feed.like/{n}")});
        }
        serde_json::from_value(v).unwrap()
    }

    fn page(rng: &mut Rng, next_id: &mut u64) -> Page<Post> {
        let items = (0..rng.below(8))
            .map(|_| {
                *next_id += 1;
                post(rng, *next_id)
            })
            .collect();
        Page {
            items,
            cursor: rng.chance(60).then(|| format!("c{}", rng.below(1000))),
        }
    }

    fn profile(did: &str) -> Profile {
        serde_json::from_value(
            json!({"did": did, "handle": "someone.test", "displayName": "Some 👍🏽 one"}),
        )
        .unwrap()
    }

    fn fail() -> Error {
        Error::api("the server said no")
    }

    /// What the worker would answer to `job`, sometimes with an error.
    fn answer(rng: &mut Rng, job: Job, next_id: &mut u64) -> Option<Event> {
        let ok = !rng.chance(15);
        Some(match job {
            Job::Login { .. } => Event::LoggedIn(Err(fail())),
            Job::PinnedFeeds => Event::PinnedFeeds(if ok {
                Ok(["discover", "science"]
                    .iter()
                    .take(rng.below(3))
                    .map(|n| crate::api::types::FeedInfo {
                        uri: format!("at://did:plc:f/app.bsky.feed.generator/{n}"),
                        name: n.to_string(),
                    })
                    .collect())
            } else {
                Err(fail())
            }),
            Job::CustomFeed(uri) => Event::CustomFeed {
                uri,
                result: if ok {
                    Ok(page(rng, next_id))
                } else {
                    Err(fail())
                },
            },
            Job::Timeline => Event::Timeline(if ok {
                Ok(page(rng, next_id))
            } else {
                Err(fail())
            }),
            Job::SearchPosts(query) => Event::SearchPosts {
                query,
                result: if ok {
                    Ok(page(rng, next_id))
                } else {
                    Err(fail())
                },
            },
            Job::SearchActors(query) => Event::SearchActors {
                query,
                result: if ok {
                    Ok(vec![profile("did:plc:alice"), profile("did:plc:bob")].into())
                } else {
                    Err(fail())
                },
            },
            Job::OpenProfile(a) => Event::Profile(if ok {
                Ok((profile(&a), page(rng, next_id)))
            } else {
                Err(fail())
            }),
            Job::Thread(uri) => {
                let node: ThreadNode = serde_json::from_value(json!({
                    "$type": "app.bsky.feed.defs#threadViewPost",
                    "post": {
                        "uri": uri.clone(), "cid": "c",
                        "author": {"did": "did:plc:bob", "handle": "bob.test"},
                        "record": {"text": TEXTS[rng.below(TEXTS.len())], "createdAt": "2026-09-22T00:00:00Z"},
                    },
                    "replies": [],
                }))
                .unwrap();
                Event::Thread {
                    uri,
                    result: if ok { Ok(node) } else { Err(fail()) },
                }
            }
            Job::Notifications => Event::Notifications {
                seen_at: "2026-09-22T00:00:00Z".into(),
                result: if ok {
                    Ok(Vec::new().into())
                } else {
                    Err(fail())
                },
            },
            Job::UpdateSeen(_) => Event::Seen(Ok(())),
            Job::More { feed, cursor } => {
                let result = if !ok {
                    Err(fail())
                } else {
                    Ok(match &feed {
                        Feed::SearchActors(_) => {
                            MorePage::Actors(vec![profile("did:plc:x")].into())
                        }
                        Feed::Notifications => MorePage::Notifications(Vec::new().into()),
                        _ => MorePage::Posts(page(rng, next_id)),
                    })
                };
                Event::More {
                    feed,
                    cursor,
                    result,
                }
            }
            Job::Like { subject } => Event::Liked {
                post_uri: subject.uri,
                result: if ok {
                    Ok("at://did:plc:me/app.bsky.feed.like/new".into())
                } else {
                    Err(fail())
                },
            },
            Job::Unlike { post_uri, .. } => Event::Unliked {
                post_uri,
                result: if ok { Ok(()) } else { Err(fail()) },
            },
            Job::Repost { subject } => Event::Reposted {
                post_uri: subject.uri,
                result: if ok {
                    Ok("at://did:plc:me/app.bsky.feed.repost/new".into())
                } else {
                    Err(fail())
                },
            },
            Job::Unrepost { post_uri, .. } => Event::Unreposted {
                post_uri,
                result: if ok { Ok(()) } else { Err(fail()) },
            },
            Job::Follow { did } => Event::Followed {
                did,
                result: if ok {
                    Ok("at://did:plc:me/app.bsky.graph.follow/new".into())
                } else {
                    Err(fail())
                },
            },
            Job::Unfollow { did, .. } => Event::Unfollowed {
                did,
                result: if ok { Ok(()) } else { Err(fail()) },
            },
            Job::Post { reply, .. } => Event::Posted {
                reply_to: reply.map(|r| r.parent.uri),
                result: if ok { Ok(()) } else { Err(fail()) },
            },
            Job::DeletePost { uri } => Event::PostDeleted {
                uri,
                result: if ok { Ok(()) } else { Err(fail()) },
            },
            Job::LoadProfileEditor => Event::ProfileEditor(Err(fail())),
            Job::SaveProfile { .. } => Event::ProfileSaved(if ok { Ok(()) } else { Err(fail()) }),
            Job::Download { .. } => Event::Downloaded(Err(fail())),
            Job::OpenLink { url, .. } => Event::Opened {
                url,
                result: Err(fail()),
            },
        })
    }

    fn is_write(job: &Job) -> bool {
        matches!(
            job,
            Job::Like { .. }
                | Job::Unlike { .. }
                | Job::Repost { .. }
                | Job::Unrepost { .. }
                | Job::Follow { .. }
                | Job::Unfollow { .. }
                | Job::Post { .. }
                | Job::DeletePost { .. }
                | Job::SaveProfile { .. }
        )
    }

    fn key(rng: &mut Rng) -> KeyEvent {
        const CHARS: &[char] = &[
            'j', 'k', 'g', 'G', 'l', 'b', 'f', 'r', 'n', 'v', 'o', '/', 't', 'T', '?', 'R', 'e',
            'd', 'D', '1', '2', '3', '4', ' ', 'a', 'y', 'x', '日', '👍', '[', ']',
        ];
        let codes = [
            KeyCode::Esc,
            KeyCode::Enter,
            KeyCode::Tab,
            KeyCode::BackTab,
            KeyCode::Backspace,
            KeyCode::Up,
            KeyCode::Down,
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::PageDown,
            KeyCode::PageUp,
            KeyCode::Home,
            KeyCode::End,
        ];
        match rng.below(10) {
            0..=5 => KeyEvent::from(KeyCode::Char(CHARS[rng.below(CHARS.len())])),
            6..=8 => KeyEvent::from(codes[rng.below(codes.len())]),
            _ => KeyEvent::new(
                KeyCode::Char(['s', 'o', 'x', 'u', 't'][rng.below(5)]),
                KeyModifiers::CONTROL,
            ),
        }
    }

    fn check_lists(app: &App, seed: u64, step: usize) {
        let bounded = |name: &str, len: usize, selected: usize| {
            assert!(
                len == 0 || selected < len,
                "seed {seed} step {step}: {name} selected {selected} of {len}"
            );
        };
        bounded("timeline", app.timeline.items.len(), app.timeline.selected);
        bounded(
            "search posts",
            app.search.posts.items.len(),
            app.search.posts.selected,
        );
        bounded(
            "search actors",
            app.search.actors.items.len(),
            app.search.actors.selected,
        );
        bounded(
            "profile posts",
            app.profile.posts.items.len(),
            app.profile.posts.selected,
        );
        bounded(
            "notifications",
            app.notifications.items.len(),
            app.notifications.selected,
        );
    }

    #[test]
    fn random_keys_and_late_answers_keep_the_client_sound() {
        let dir = tempfile::tempdir().unwrap();
        // A short run in every `cargo test`; BSKY_FUZZ_SEEDS=2000 (in a
        // release build) for a long one.
        let seeds: u64 = std::env::var("BSKY_FUZZ_SEEDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(12);
        for seed in 1..=seeds {
            let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
            let session = Session {
                service: "https://pds.test".into(),
                did: "did:plc:me".into(),
                handle: "me.test".into(),
                access_jwt: "a".into(),
                refresh_jwt: "r".into(),
            };
            let (mut app, jobs) = App::new(Some(session), "https://pds.test");
            app.browse_from = Some(dir.path().to_path_buf());
            let mut next_id = 0u64;
            let mut pending: Vec<Event> = Vec::new();
            for job in jobs {
                pending.extend(answer(&mut rng, job, &mut next_id));
            }
            let mut images = Images::new(Picker::halfblocks(), None);
            for step in 0..800 {
                if rng.chance(35) && !pending.is_empty() {
                    // Any waiting answer, not only the oldest: answers arrive
                    // late and out of order.
                    let ev = pending.swap_remove(rng.below(pending.len()));
                    for job in app.handle_event(ev) {
                        pending.extend(answer(&mut rng, job, &mut next_id));
                    }
                } else {
                    let k = key(&mut rng);
                    // What a like or repost must act on: the post selected
                    // when the key is pressed, whatever arrives later.
                    let target = app
                        .overlay
                        .is_none()
                        .then(|| app.selected_post().map(|p| p.uri))
                        .flatten();
                    let jobs = app.handle_key(k);
                    let writes = jobs.iter().filter(|j| is_write(j)).count();
                    assert!(
                        writes <= 1,
                        "seed {seed} step {step}: {k:?} sent {writes} writes: {jobs:?}"
                    );
                    for job in &jobs {
                        let acted_on = match job {
                            Job::Like { subject } | Job::Repost { subject } => Some(&subject.uri),
                            Job::Unlike { post_uri, .. } | Job::Unrepost { post_uri, .. } => {
                                Some(post_uri)
                            }
                            _ => None,
                        };
                        if let Some(uri) = acted_on {
                            assert_eq!(
                                Some(uri),
                                target.as_ref(),
                                "seed {seed} step {step}: {k:?} acted on another post than the selected one"
                            );
                        }
                    }
                    for job in jobs {
                        pending.extend(answer(&mut rng, job, &mut next_id));
                    }
                }
                if app.quit {
                    break;
                }
                check_lists(&app, seed, step);
                if step % 7 == 0 {
                    let (w, h) = (1 + rng.below(120) as u16, 1 + rng.below(50) as u16);
                    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
                    term.draw(|f| draw(f, &mut app, &mut images)).unwrap();
                }
            }
        }
    }
}
