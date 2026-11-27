//! Drawing, in bsky's layout: the tabs on the top row, the list, the key
//! hints, and the status row. Every function here reads the [`App`] and
//! writes to a frame; the only state it changes is each list's scroll
//! offset.

use std::collections::HashMap;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect, Size};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::api::types::{DriveFile, Note, Notification, User};
use crate::cli::TimelineKind;
use crate::terminal::protocol_name;
use crate::tui::app::{Account, App, List, LoginForm, Overlay, Tab, ThreadRow, ThreadView};
use crate::tui::images::Images;
use crate::tui::keys;
use crate::tui::text::{cells, cluster_width, drawable, format_time, graphemes, truncate, wrap};
use crate::tui::theme::Theme;

mod browse;
mod chat;
mod compose;
#[cfg(test)]
mod tests;

use browse::{draw_profile, draw_search};
use compose::{draw_browser, draw_compose, draw_reactions};

/// Width of the selection marker column.
const MARK_W: u16 = 2;
/// Avatar size in the lists, in cells.
const AVATAR: (u16, u16) = (4, 2);
/// Rows for a note's pictures when the server did not measure them.
const IMAGE_ROWS: u16 = 8;
/// Bounds on the rows a note's pictures take.
const IMAGE_ROWS_MIN: u16 = 3;
const IMAGE_ROWS_MAX: u16 = 12;
/// Widest a single picture may be, in cells.
const IMAGE_MAX_W: u16 = 36;
/// Pictures of a note shown in the list; the viewer shows them all.
const IMAGES_SHOWN: usize = 4;
/// Notes below the screen whose pictures are fetched and encoded ahead.
const PREFETCH: usize = 4;
/// Notes further on whose pictures are only downloaded ahead.
const WARM: usize = 20;
/// The key hints take a second row rather than lose a key off their end.
const MAX_HINT_ROWS: u16 = 2;
/// Widest the error box, the lists over the screen, and the help get.
const ERROR_W: u16 = 76;
const LIST_W: u16 = 52;
const HELP_W: u16 = 64;
const HELP_KEYS: usize = 18;
/// Smallest terminal the client draws in, as bsky's.
const MIN_W: u16 = 24;
const MIN_H: u16 = 8;
/// Reactions listed beside a note's counts, the most given first.
const REACTIONS_SHOWN: usize = 3;

/// Draw the whole UI.
pub fn draw(frame: &mut Frame, app: &mut App, images: &mut Images) {
    let area = frame.area();
    let t = app.theme;
    images.begin_frame(Size::new(area.width, area.height), t.dim());
    frame.render_widget(Block::new().style(t.base()), area);
    if area.width < MIN_W || area.height < MIN_H {
        draw_too_small(frame, area, &t);
        return;
    }
    let hint_lines = hint_lines(&keys::hints(app), area.width, &t);
    let actions = keys::actions(app);
    let hint_h = (hint_lines.len() as u16).clamp(1, MAX_HINT_ROWS);
    let [top, body, hint_row, status_row] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(hint_h),
        Constraint::Length(1),
    ])
    .areas(area);
    draw_tabs(frame, top, app);
    let ctx = Ctx {
        opened: &app.opened,
        emojis: &app.emojis,
        t,
    };
    if let Some(th) = app.threads.last_mut() {
        draw_thread(frame, body, th, images, &ctx);
    } else {
        match app.tab {
            Tab::Timeline(kind) => {
                draw_notes(
                    frame,
                    body,
                    &mut app.timelines[kind.index()],
                    images,
                    empty_timeline(kind),
                    &ctx,
                );
            }
            Tab::Notifications => {
                draw_notifications(frame, body, &mut app.notifications, images, &ctx);
            }
            Tab::Search => draw_search(frame, body, &mut app.search, images, &ctx),
            Tab::Profile => draw_profile(frame, body, &mut app.profile, images, &ctx),
            Tab::Chat => {
                let me = app.session.as_ref().map(|s| s.user_id.clone());
                chat::draw_chat(frame, body, &mut app.chat, me.as_deref(), images, &ctx);
            }
        }
    }
    frame.render_widget(Paragraph::new(hint_lines), hint_row);
    draw_status(frame, status_row, app, images);
    match &app.overlay {
        Some(Overlay::Help { scroll }) => {
            let mut scroll = *scroll;
            draw_help(frame, area, &mut scroll, app.pictures, &t);
            app.overlay = Some(Overlay::Help { scroll });
        }
        Some(Overlay::Actions { selected }) => {
            let rows: Vec<String> = actions.iter().map(|(k, d)| format!("{k:<6} {d}")).collect();
            draw_choices(frame, area, "Actions", &rows, *selected, None, &t);
        }
        Some(Overlay::Accounts { selected }) => {
            let me = app.session.as_ref().map(|s| s.host().to_string());
            draw_account_list(frame, area, &app.accounts, *selected, me.as_deref(), &t);
        }
        Some(Overlay::Login(f)) => draw_login(frame, area, f, &t),
        Some(Overlay::Reactions { selected, query }) => {
            let choices = app.reaction_choices(query);
            draw_reactions(
                frame,
                area,
                &choices,
                *selected,
                query,
                &app.emojis,
                images,
                &t,
            );
        }
        Some(Overlay::Compose(_)) => {}
        Some(Overlay::Viewer {
            files,
            index,
            replay,
        }) => {
            let body = Rect {
                height: area.height.saturating_sub(hint_h + 1),
                ..area
            };
            draw_viewer(frame, body, files, *index, *replay, images, &t);
        }
        None => {}
    }
    // A video plays only while the viewer shows it.
    let video_shown = matches!(
        &app.overlay,
        Some(Overlay::Viewer { files, index, .. })
            if files.get(*index).is_some_and(|f| f.2.mime.starts_with("video/"))
    );
    if !video_shown {
        images.stop_video();
    }
    if let Some(Overlay::Compose(c)) = &mut app.overlay {
        draw_compose(frame, area, c, images, &t);
        if let Some(b) = &mut c.browser {
            draw_browser(frame, area, b, images, &t);
        }
    }
    if let Some(s) = app.status.as_ref().filter(|s| s.error) {
        draw_error(frame, area, &s.text, &t);
    }
}

/// What every note is drawn with.
struct Ctx<'a> {
    /// The notes whose warning `w` opened.
    opened: &'a std::collections::HashSet<String>,
    /// The server's custom emoji, name to picture address.
    emojis: &'a std::collections::HashMap<String, String>,
    t: Theme,
}

fn empty_timeline(kind: TimelineKind) -> &'static str {
    match kind {
        TimelineKind::Home => "No notes from the accounts you follow yet. Press R to refresh.",
        _ => "No notes yet. Press R to refresh.",
    }
}

/// The screen for a terminal too small for the client, as bsky's says.
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

/// An error in a box in the middle of the screen, over everything. It goes
/// with the next key, or by itself after a while.
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

fn draw_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let t = app.theme;
    let mut spans = Vec::new();
    for (i, &tab) in app.tabs.iter().enumerate() {
        let label = format!(" {} {} ", i + 1, tab.title());
        let style = if tab == app.tab {
            t.selected()
        } else {
            t.dim()
        };
        spans.push(Span::styled(label, style));
    }
    // Who the client reads as, at the right where it fits whole: the
    // account in use, or the server read as a visitor.
    let used: usize = spans.iter().map(|s| cells(&s.content)).sum();
    let name = match &app.session {
        Some(s) => format!("@{}@{} ", s.username, s.host()),
        None => format!("{} (visitor) ", app.host()),
    };
    let name = drawable(&name).into_owned();
    let room = usize::from(area.width).saturating_sub(used);
    let name_w = cells(&name);
    if name_w < room {
        spans.push(Span::raw(" ".repeat(room - name_w)));
        spans.push(Span::styled(name, t.accent()));
    }
    frame.render_widget(Line::from(spans), area);
}

/// The keys that work in the current view, packed into as many rows of
/// `width` as they need; a hint is never split across rows.
fn hint_lines(hints: &[keys::Hint], width: u16, t: &Theme) -> Vec<Line<'static>> {
    let width = usize::from(width.max(1));
    let mut lines: Vec<Vec<Span<'static>>> = Vec::new();
    let mut row: Vec<Span<'static>> = vec![Span::raw(" ")];
    let mut used = 1;
    for (key, what) in hints {
        let w = cells(key) + 1 + cells(what);
        let first = row.len() == 1;
        if !first && used + 2 + w > width {
            lines.push(std::mem::replace(&mut row, vec![Span::raw(" ")]));
            used = 1;
        } else if !first {
            row.push(Span::raw("  "));
            used += 2;
        }
        row.push(Span::styled(*key, t.accent().bold()));
        row.push(Span::styled(format!(" {what}"), t.dim()));
        used += w;
    }
    lines.push(row);
    lines
        .into_iter()
        .map(|spans| truncate_line(Line::from(spans), width))
        .collect()
}

/// The last message on the left; activity or the picture protocol on the
/// right, as bsky's.
fn draw_status(frame: &mut Frame, area: Rect, app: &App, images: &Images) {
    let t = app.theme;
    let right = if app.pending > 0 || images.loading() {
        "loading… ".to_string()
    } else if images.shows() {
        format!("{} ", protocol_name(images.protocol_type()))
    } else {
        "text ".to_string()
    };
    let w = cells(&right) as u16;
    if let Some(s) = app.status.as_ref().filter(|s| !s.error) {
        let room = usize::from(area.width.saturating_sub(w + 2));
        frame.render_widget(
            Line::from(Span::styled(
                format!(" {}", truncate(&s.text, room)),
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

/// The first item to draw so that `selected` is on screen, as bsky picks
/// it: `offset` when it already is, otherwise the lowest start that still
/// fits the selection; room left at the end goes to the items above.
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

/// `text` wrapped to `width` cells by the client's own rule.
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
        Some(e) if list.retry => {
            wrapped(&format!(" {e}  (R to retry)"), area.width).style(t.error())
        }
        Some(e) => wrapped(&format!(" {e}"), area.width).style(t.dim()),
        None => wrapped(&format!(" {empty}"), area.width).style(t.dim()),
    };
    frame.render_widget(p, area);
}

/// `line` cut to `width` columns as one piece: whole grapheme clusters up
/// to the cut and one ellipsis there, in the style of the span it cuts.
fn truncate_line(line: Line<'static>, width: usize) -> Line<'static> {
    let spans: Vec<(String, Style)> = line
        .spans
        .into_iter()
        .map(|s| (drawable(&s.content).into_owned(), s.style))
        .collect();
    if spans.iter().map(|(t, _)| cells(t)).sum::<usize>() <= width {
        return Line::from(
            spans
                .into_iter()
                .map(|(t, s)| Span::styled(t, s))
                .collect::<Vec<_>>(),
        );
    }
    let mut left = width.saturating_sub(1);
    let mut out = Vec::new();
    for (text, style) in spans {
        let mut kept = String::new();
        for (_, g) in graphemes(&text) {
            let gw = cluster_width(g);
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

/// `@username` or `@username@host`, as text only.
fn acct(u: &User) -> String {
    let s = match u.host.as_deref().filter(|h| !h.is_empty()) {
        Some(h) => format!("@{}@{h}", u.username),
        None => format!("@{}", u.username),
    };
    crate::tui::text::one_line(&drawable(&s))
}

/// The display name, or the username when there is none.
fn display_name(u: &User) -> String {
    let name = u
        .name
        .as_deref()
        .map(|n| crate::tui::text::one_line(&drawable(n)))
        .filter(|n| !n.is_empty());
    name.unwrap_or_else(|| crate::tui::text::one_line(&drawable(&u.username)))
}

/// The first line of what `n` says, its warning in place of a text it
/// hides.
fn first_line(n: &Note, opened: bool) -> String {
    match (&n.cw, opened) {
        (Some(cw), false) => format!("⚠ CW: {}", crate::tui::text::one_line(&drawable(cw))),
        _ => n
            .text
            .as_deref()
            .map(|t| {
                drawable(&crate::mfm::plain_text(t))
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string()
            })
            .unwrap_or_default(),
    }
}

/// A note's text as lines of `width`: its MFM drawn as plain text
/// ([`crate::mfm::plain`]), the bold runs bold.
fn mfm_lines(text: &str, width: usize) -> Vec<Line<'static>> {
    mfm_lines_urls(text, width, &mut |_| None, &mut Vec::new())
}

/// The two cells a custom emoji's picture is drawn over: what is laid out
/// in its place, and found again on screen to draw it.
pub(super) const EMOJI_CELLS: &str = "\u{E000}\u{E001}";

/// `text` with each `:name:` that `picture` has a picture for put as
/// [`EMOJI_CELLS`], the pictures' addresses pushed in order.
fn with_emoji_cells(
    text: &str,
    picture: &mut dyn FnMut(&str) -> Option<String>,
    urls: &mut Vec<String>,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(':') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let end = after.find(|c: char| {
            !(c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-' | '@' | '.'))
        });
        match end {
            Some(e) if e > 0 && after[e..].starts_with(':') => {
                let name = &after[..e];
                match picture(name) {
                    Some(url) => {
                        urls.push(url);
                        out.push_str(EMOJI_CELLS);
                        rest = &after[e + 1..];
                    }
                    None => {
                        out.push(':');
                        rest = after;
                    }
                }
            }
            _ => {
                out.push(':');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// [`mfm_lines`] with the custom emoji `picture` knows laid out as
/// [`EMOJI_CELLS`], their addresses pushed to `urls` in order.
fn mfm_lines_urls(
    text: &str,
    width: usize,
    picture: &mut dyn FnMut(&str) -> Option<String>,
    urls: &mut Vec<String>,
) -> Vec<Line<'static>> {
    let runs = crate::mfm::plain(text);
    // The text as wrap lays it out, each run made drawable first, and
    // where each bold run is in it.
    let mut plain = String::new();
    let mut bold: Vec<(usize, usize)> = Vec::new();
    for r in &runs {
        let t = with_emoji_cells(&drawable(&r.text), picture, urls);
        if r.bold {
            bold.push((plain.len(), plain.len() + t.len()));
        }
        plain.push_str(&t);
    }
    let lines = crate::tui::text::wrap_cached(&plain, width);
    if bold.is_empty() {
        return lines.into_iter().map(Line::from).collect();
    }
    // Each line is a piece of the text, in order: where it is says which
    // of its characters are bold.
    let mut at = 0;
    lines
        .into_iter()
        .map(|line| {
            let Some(start) = plain[at..].find(line.as_str()).map(|p| at + p) else {
                return Line::from(line);
            };
            at = start + line.len();
            let mut spans: Vec<Span<'static>> = Vec::new();
            let mut cut = 0;
            for &(b0, b1) in &bold {
                let (s0, s1) = (b0.max(start), b1.min(at));
                if s0 >= s1 {
                    continue;
                }
                let (s0, s1) = (s0 - start, s1 - start);
                if s0 > cut {
                    spans.push(Span::raw(line[cut..s0].to_string()));
                }
                spans.push(Span::styled(line[s0..s1].to_string(), Style::new().bold()));
                cut = s1;
            }
            if cut < line.len() {
                spans.push(Span::raw(line[cut..].to_string()));
            }
            Line::from(spans)
        })
        .collect()
}

/// The picture of custom emoji `name` in the text of `n`: one of its own
/// server's, else one of the server read (for a note of this server).
fn emoji_of(n: &Note, name: &str, ctx: &Ctx) -> Option<String> {
    if let Some(u) = n.emojis.get(name) {
        return Some(u.clone());
    }
    if n.user.host.as_deref().is_none_or(str::is_empty) {
        return ctx.emojis.get(name).cloned();
    }
    None
}

/// The picture of reaction `key`: `:name@.:` is the server read's own,
/// `:name@host:` another server's, which the note names.
fn reaction_picture(n: &Note, key: &str, ctx: &Ctx) -> Option<String> {
    let inner = key.strip_prefix(':')?.strip_suffix(':')?;
    match inner.split_once('@') {
        Some((name, ".")) => ctx.emojis.get(name).cloned(),
        Some(_) => n.reaction_emojis.get(inner).cloned(),
        None => ctx.emojis.get(inner).cloned(),
    }
}

/// Draw `urls` over the [`EMOJI_CELLS`] of `area`, in order, row by row.
fn draw_emoji_cells(frame: &mut Frame, area: Rect, urls: &[String], images: &mut Images) {
    if urls.is_empty() {
        return;
    }
    let area = area.intersection(frame.area());
    let mut found = Vec::new();
    {
        let buf = frame.buffer_mut();
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if buf[(x, y)].symbol() == "\u{E000}" {
                    found.push((x, y));
                }
            }
        }
    }
    for ((x, y), url) in found.into_iter().zip(urls) {
        // Blank under the picture: one still loading draws a mark in the
        // first cell only.
        {
            let buf = frame.buffer_mut();
            buf[(x, y)].set_symbol(" ");
            if x + 1 < area.right() {
                buf[(x + 1, y)].set_symbol(" ");
            }
        }
        images.draw(
            frame,
            Rect {
                x,
                y,
                width: 2,
                height: 1,
            },
            url,
        );
    }
}

/// A reaction as it reads: a custom emoji of the server read, `:name@.:`,
/// as `:name:`; one of another server keeps its host. Custom emoji are
/// shown by name: drawing them is for a later version.
fn reaction_label(key: &str) -> String {
    match key.strip_suffix("@.:") {
        Some(name) => format!("{name}:"),
        None => key.to_string(),
    }
}

/// Everything a note draws besides pictures, laid out for a width.
struct NoteLines {
    /// Who renoted it, and the note it answers, one dim line each.
    context: Vec<Line<'static>>,
    header: Line<'static>,
    body: Vec<Line<'static>>,
    /// The pictures drawn under the text: their addresses and shapes.
    images: Vec<(String, Option<(u32, u32)>)>,
    image_rows: u16,
    /// Whether an avatar is drawn beside it.
    avatar: Option<String>,
    /// The custom emoji drawn as pictures, in the order their cells come.
    emoji: Vec<String>,
    stats: Line<'static>,
}

/// Width in cells of each picture's box when `n` share `width` columns.
fn image_box_width(width: u16, n: u16) -> u16 {
    if n == 0 || width < n {
        return 0;
    }
    ((width - (n - 1)) / n).min(IMAGE_MAX_W)
}

/// Rows a picture of shape `w:h` needs at `box_w` cells, as bsky's.
fn image_rows(aspect: Option<(u32, u32)>, box_w: u16, cell: (u16, u16)) -> u16 {
    let Some((w, h)) = aspect else {
        return IMAGE_ROWS;
    };
    let (cw, ch) = (u64::from(cell.0.max(1)), u64::from(cell.1.max(1)));
    let px_h = u64::from(box_w) * cw * u64::from(h) / u64::from(w);
    let rows = px_h.div_ceil(ch);
    rows.clamp(u64::from(IMAGE_ROWS_MIN), u64::from(IMAGE_ROWS_MAX)) as u16
}

/// The picture to show of `f` in a list: the server's thumbnail when it
/// made one.
fn list_picture(f: &DriveFile) -> Option<String> {
    f.thumbnail_url
        .clone()
        .or_else(|| f.url.clone())
        .filter(|u| !u.is_empty())
}

/// What a note's files are, in words: `▣ 2 pictures · ▶ 1 video · 1 file`,
/// with `⚠ sensitive` when the uploader marked any of them so. The
/// pictures are drawn all the same; the mark says what they are.
fn files_line(files: &[DriveFile], pictures_drawn: bool, t: &Theme) -> Option<Line<'static>> {
    if files.is_empty() {
        return None;
    }
    let count = |p: &str| files.iter().filter(|f| f.mime.starts_with(p)).count();
    let (pics, videos, sounds) = (count("image/"), count("video/"), count("audio/"));
    let other = files.len() - pics - videos - sounds;
    let plural = |n: usize, one: &str, many: &str| {
        if n == 1 {
            format!("1 {one}")
        } else {
            format!("{n} {many}")
        }
    };
    let mut parts = Vec::new();
    if pics > 0 && !pictures_drawn {
        parts.push(format!("▣ {}", plural(pics, "picture", "pictures")));
    }
    if videos > 0 {
        parts.push(format!("▶ {}", plural(videos, "video", "videos")));
    }
    if sounds > 0 {
        parts.push(format!("♪ {}", plural(sounds, "sound", "sounds")));
    }
    if other > 0 {
        parts.push(plural(other, "file", "files"));
    }
    let sensitive = files.iter().filter(|f| f.is_sensitive).count();
    let mut spans = Vec::new();
    if sensitive > 0 {
        spans.push(Span::styled(
            format!(
                "⚠ {} ",
                plural(sensitive, "sensitive file", "sensitive files")
            ),
            t.warn(),
        ));
    }
    if !parts.is_empty() {
        spans.push(Span::styled(parts.join(" · "), t.dim()));
    }
    (!spans.is_empty()).then(|| Line::from(spans))
}

impl NoteLines {
    /// `n` laid out at `width`; `cell` is the terminal's cell size in
    /// pixels, or `None` when it shows no pictures.
    fn new(n: &Note, width: u16, cell: Option<(u16, u16)>, ctx: &Ctx) -> Self {
        let t = &ctx.t;
        let width = usize::from(width.max(1));
        let shown = n.shown();
        let mut context = Vec::new();
        if n.is_pure_renote() {
            context.push(Line::styled(
                truncate(
                    &format!("⟳ renoted by {} {}", display_name(&n.user), acct(&n.user)),
                    width,
                ),
                t.renote(),
            ));
        }
        if let Some(parent) = &shown.reply {
            let opened = ctx.opened.contains(&parent.id);
            context.push(Line::styled(
                truncate(
                    &format!(
                        "┆ {} {}: {}",
                        display_name(&parent.user),
                        acct(&parent.user),
                        first_line(parent, opened || parent.cw.is_none())
                    ),
                    width,
                ),
                t.dim(),
            ));
        }
        let mut header = vec![
            Span::styled(display_name(&shown.user), Style::new().bold()),
            Span::styled(format!(" {}", acct(&shown.user)), t.dim()),
            Span::styled(format!(" · {}", format_time(&shown.created_at)), t.dim()),
        ];
        if shown.reply_id.is_some() {
            header.push(Span::styled(" ↩ reply", t.dim()));
        }
        let header = truncate_line(Line::from(header), width);

        let mut body: Vec<Line<'static>> = Vec::new();
        let mut emoji: Vec<String> = Vec::new();
        let text_shown = shown.cw.is_none() || ctx.opened.contains(&shown.id);
        if let Some(cw) = &shown.cw {
            let cw = if cw.trim().is_empty() {
                "(no reason given)"
            } else {
                cw
            };
            body.extend(
                crate::tui::text::wrap_cached(&format!("⚠ CW: {cw}"), width)
                    .into_iter()
                    .map(|l| Line::styled(l, t.warn())),
            );
        }
        if text_shown {
            if let Some(text) = &shown.text {
                if cell.is_some() {
                    let mut picture = |name: &str| emoji_of(shown, name, ctx);
                    body.extend(mfm_lines_urls(text, width, &mut picture, &mut emoji));
                } else {
                    body.extend(mfm_lines(text, width));
                }
            }
        } else {
            let chars = shown.text.as_deref().map_or(0, |s| graphemes(s).count());
            let mut hidden = format!("w shows it ({chars} characters");
            if !shown.files.is_empty() {
                hidden.push_str(&format!(", {} files", shown.files.len()));
            }
            hidden.push(')');
            body.push(Line::styled(truncate(&hidden, width), t.dim()));
        }
        // A quote: who is quoted and the start of what they wrote.
        if !shown.is_pure_renote() && shown.renote_id.is_some() {
            let line = match &shown.renote {
                Some(q) => format!(
                    "❝ {}: {}",
                    acct(&q.user),
                    first_line(q, ctx.opened.contains(&q.id))
                ),
                None => "❝ (the quoted note is not shown)".to_string(),
            };
            body.push(Line::styled(truncate(&line, width), t.dim()));
        }
        let mut images = Vec::new();
        let mut rows = 0;
        // The files of a note whose text is hidden stay with it.
        let pictures: Vec<&DriveFile> = if text_shown {
            shown.files.iter().filter(|f| f.is_picture()).collect()
        } else {
            Vec::new()
        };
        if let Some(cell) = cell
            && !pictures.is_empty()
        {
            let drawn: Vec<&DriveFile> = pictures.iter().copied().take(IMAGES_SHOWN).collect();
            let box_w = image_box_width(width as u16, drawn.len() as u16);
            rows = drawn
                .iter()
                .map(|f| image_rows(f.aspect(), box_w, cell))
                .max()
                .unwrap_or(0);
            images = drawn
                .iter()
                .filter_map(|f| list_picture(f).map(|u| (u, f.aspect())))
                .collect();
            if pictures.len() > IMAGES_SHOWN {
                body.push(Line::styled(
                    format!(
                        "+{} more: space shows them all",
                        pictures.len() - IMAGES_SHOWN
                    ),
                    t.dim(),
                ));
            }
        }
        if text_shown && let Some(l) = files_line(&shown.files, cell.is_some(), t) {
            body.push(truncate_line(l, width));
        }
        let mut stats = vec![
            Span::styled(format!("♡ {}", shown.reaction_total()), t.dim()),
            Span::raw("   "),
            Span::styled(format!("⟳ {}", shown.renote_count), t.dim()),
            Span::styled(format!("   ↩ {}", shown.replies_count), t.dim()),
        ];
        let mut reactions: Vec<(&String, &u64)> = shown.reactions.iter().collect();
        reactions.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        for (k, v) in reactions.into_iter().take(REACTIONS_SHOWN) {
            match cell.and_then(|_| reaction_picture(shown, k, ctx)) {
                Some(url) => {
                    emoji.push(url);
                    stats.push(Span::raw(format!("   {EMOJI_CELLS} {v}")));
                }
                None => stats.push(Span::raw(format!("   {} {v}", reaction_label(k)))),
            }
        }
        let stats = truncate_line(Line::from(stats), width);
        Self {
            context,
            header,
            body,
            images,
            image_rows: rows,
            avatar: cell
                .and_then(|_| shown.user.avatar_url.clone())
                .filter(|u| !u.is_empty()),
            emoji,
            stats,
        }
    }

    fn height(&self) -> u16 {
        let ctx = self.context.len() as u16;
        let content = ctx + 2 + self.body.len() as u16 + self.image_rows;
        let avatar = if self.avatar.is_some() {
            ctx + AVATAR.1
        } else {
            0
        };
        content.max(avatar) + 1
    }
}

/// Something drawn as one entry of a list of notes: a timeline's note, or
/// a row of a thread, indented by its depth.
trait NoteRow {
    fn note(&self) -> &Note;
    fn indent(&self) -> u16 {
        0
    }
}

impl NoteRow for Note {
    fn note(&self) -> &Note {
        self
    }
}

impl NoteRow for ThreadRow {
    fn note(&self) -> &Note {
        &self.note
    }
    fn indent(&self) -> u16 {
        self.depth
    }
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

/// `lines` from the top of `area`, one per row, each cut at its width.
fn draw_lines<'a>(frame: &mut Frame, area: Rect, lines: impl IntoIterator<Item = &'a Line<'a>>) {
    let area = area.intersection(frame.area());
    let buf = frame.buffer_mut();
    for (y, line) in (area.top()..area.bottom()).zip(lines) {
        buf.set_line(area.x, y, line, area.width);
    }
}

fn draw_image_row(
    frame: &mut Frame,
    area: Rect,
    urls: &[(String, Option<(u32, u32)>)],
    images: &mut Images,
) {
    let each = image_box_width(area.width, urls.len() as u16);
    if each == 0 {
        return;
    }
    for (i, (url, _)) in urls.iter().enumerate() {
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

fn draw_notes<T: NoteRow>(
    frame: &mut Frame,
    area: Rect,
    list: &mut List<T>,
    images: &mut Images,
    empty: &str,
    ctx: &Ctx,
) {
    let t = &ctx.t;
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
    let width_of = |item: &T| content.width.saturating_sub(item.indent() * 2);
    let mut lines: HashMap<usize, NoteLines> = HashMap::new();
    let selected = list.selected.min(list.items.len() - 1);
    let items = &list.items;
    list.offset = scroll_offset(selected, list.offset, list.items.len(), area.height, |i| {
        lines
            .entry(i)
            .or_insert_with(|| NoteLines::new(items[i].note(), width_of(&items[i]), cell, ctx))
            .height()
    });
    let mut y = area.y;
    let mut below = list.offset;
    for (i, item) in list.items.iter().enumerate().skip(list.offset) {
        if y >= area.bottom() {
            break;
        }
        let nl = lines
            .entry(i)
            .or_insert_with(|| NoteLines::new(item.note(), width_of(item), cell, ctx));
        let h = nl.height();
        let visible = (area.bottom() - y).min(h);
        let row = Rect {
            y,
            height: visible,
            ..area
        };
        let whole = visible == h;
        below = if whole { i + 1 } else { i };
        draw_marker(frame, row, h, i == list.selected, t);
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
        if let Some(url) = &nl.avatar
            && whole
        {
            let avatar = Rect {
                x: area.x + MARK_W + indent,
                y: y + nl.context.len() as u16,
                width: AVATAR.0,
                height: AVATAR.1,
            };
            images.draw(frame, avatar, url);
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
        let text = nl.context.iter().chain([&nl.header]).chain(&nl.body);
        let body_rows = (nl.context.len() + 1 + nl.body.len()) as u16;
        draw_lines(frame, c, text);
        let mut next = y + body_rows;
        if !nl.images.is_empty() {
            let img_area = Rect {
                y: next,
                height: nl.image_rows,
                ..content
            };
            if whole {
                draw_image_row(frame, img_area, &nl.images, images);
            }
            next += nl.image_rows;
        }
        if next < area.bottom() {
            frame.render_widget(
                nl.stats.clone(),
                Rect {
                    y: next,
                    height: 1,
                    ..content
                },
            );
        }
        // The custom emoji of the text and the counts, over their cells.
        draw_emoji_cells(
            frame,
            Rect {
                y,
                height: visible,
                ..content
            },
            &nl.emoji,
            images,
        );
        y += h;
    }
    // The pictures of the notes below the screen are ready when scrolled to.
    for (i, item) in list.items.iter().enumerate().skip(below).take(PREFETCH) {
        let nl = lines
            .entry(i)
            .or_insert_with(|| NoteLines::new(item.note(), width_of(item), cell, ctx));
        if let Some(url) = &nl.avatar {
            images.prefetch(url, AVATAR.0, AVATAR.1);
        }
        let each = image_box_width(width_of(item), nl.images.len() as u16);
        for (url, _) in &nl.images {
            images.prefetch(url, each, nl.image_rows);
        }
    }
    for item in list.items.iter().skip(below + PREFETCH).take(WARM) {
        let n = item.note().shown();
        if let Some(url) = &n.user.avatar_url {
            images.warm(url);
        }
        for f in n.files.iter().filter(|f| f.is_picture()).take(IMAGES_SHOWN) {
            if let Some(url) = list_picture(f) {
                images.warm(&url);
            }
        }
    }
}

/// A thread over the current tab: a title row, then its notes, the opened
/// one among them where the selection starts.
fn draw_thread(frame: &mut Frame, area: Rect, th: &mut ThreadView, images: &mut Images, ctx: &Ctx) {
    let t = &ctx.t;
    let [title, list] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
    frame.render_widget(
        Line::from(vec![
            Span::styled(" Thread ", t.selected()),
            Span::styled("  esc back", t.dim()),
        ]),
        title,
    );
    if th.list.items.is_empty()
        && let Some(e) = &th.list.error
    {
        frame.render_widget(
            wrapped(
                &format!(" could not load the thread: {e}  (R to retry)"),
                list.width,
            )
            .style(t.error()),
            list,
        );
        return;
    }
    draw_notes(
        frame,
        list,
        &mut th.list,
        images,
        "The thread is empty.",
        ctx,
    );
}

/// What a notification says happened, as `msky notifications` says it.
fn happened(n: &Notification) -> String {
    match n.kind.as_str() {
        "follow" => "followed you".into(),
        "mention" => "mentioned you".into(),
        "reply" => "replied to you".into(),
        "renote" => "renoted your note".into(),
        "quote" => "quoted your note".into(),
        "reaction" => format!(
            "reacted {} to your note",
            reaction_label(n.reaction.as_deref().unwrap_or("?"))
        ),
        "pollEnded" => "a poll has ended".into(),
        "receiveFollowRequest" => "asked to follow you".into(),
        "followRequestAccepted" => "accepted your follow request".into(),
        "achievementEarned" => format!(
            "achievement earned: {}",
            n.achievement.as_deref().unwrap_or("")
        ),
        "app" => [n.header.as_deref(), n.body.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(": "),
        other => format!("({other})"),
    }
}

fn notification_lines(n: &Notification, width: u16, ctx: &Ctx) -> Vec<Line<'static>> {
    let t = &ctx.t;
    let width = usize::from(width.max(1));
    let mut head = Vec::new();
    if let Some(u) = &n.user {
        head.push(Span::styled(display_name(u), Style::new().bold()));
        head.push(Span::styled(format!(" {}", acct(u)), t.dim()));
        head.push(Span::raw(" "));
    }
    head.push(Span::raw(happened(n)));
    head.push(Span::styled(
        format!(" · {}", format_time(&n.created_at)),
        t.dim(),
    ));
    let second = match &n.note {
        Some(note) => {
            let s = note.shown();
            let line = first_line(s, s.cw.is_none() || ctx.opened.contains(&s.id));
            // A reply or a mention is the note itself; a reaction or a
            // renote is about your own, drawn dim.
            if matches!(n.kind.as_str(), "reply" | "mention" | "quote") {
                Line::from(truncate(&line, width))
            } else {
                Line::styled(truncate(&line, width), t.dim())
            }
        }
        None => Line::default(),
    };
    vec![truncate_line(Line::from(head), width), second]
}

fn draw_notifications(
    frame: &mut Frame,
    area: Rect,
    list: &mut List<Notification>,
    images: &mut Images,
    ctx: &Ctx,
) {
    let t = &ctx.t;
    if !list.loaded {
        frame.render_widget(Paragraph::new(" loading…").style(t.dim()), area);
        return;
    }
    if list.items.is_empty() {
        empty_message(frame, area, list, "No notifications yet.", t);
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
        if let Some(url) = item.user.as_ref().and_then(|u| u.avatar_url.as_deref()) {
            let r = Rect {
                x: area.x + MARK_W,
                y,
                width: AVATAR.0,
                height: AVATAR.1,
            };
            images.draw(frame, r, url);
        }
        frame.render_widget(
            Paragraph::new(notification_lines(item, content.width, ctx)),
            Rect {
                y,
                height: 2,
                ..content
            },
        );
        y += H;
    }
    let shown = usize::from(area.height / H);
    for item in list.items.iter().skip(list.offset + shown).take(WARM) {
        if let Some(url) = item.user.as_ref().and_then(|u| u.avatar_url.as_deref()) {
            images.warm(url);
        }
    }
}

/// A box of `w` x `h` centered in `area`, cleared, as bsky's.
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
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .style(t.base())
        .border_style(t.accent())
        .title(format!(" {title} "));
    let inner = block.inner(r);
    frame.render_widget(block, r);
    inner
}

/// The first of a list's entries to draw in `rows` rows so the selected
/// one is among them.
fn first_shown(selected: usize, rows: u16) -> usize {
    (selected + 1).saturating_sub(usize::from(rows.max(1)))
}

/// A list to choose from over the screen; `current` is marked.
fn draw_choices(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    rows: &[String],
    selected: usize,
    current: Option<usize>,
    t: &Theme,
) {
    let inner = popup(frame, area, LIST_W, rows.len().max(1) as u16 + 2, title, t);
    let width = usize::from(inner.width);
    let lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .skip(first_shown(selected, inner.height))
        .map(|(i, r)| {
            let marker = if i == selected { "▶ " } else { "  " };
            let mark = if current == Some(i) { "  (shown)" } else { "" };
            let style = if i == selected {
                t.base().bold()
            } else {
                t.base()
            };
            Line::from(vec![
                Span::styled(marker, t.accent().bold()),
                Span::styled(truncate(r, width.saturating_sub(2 + cells(mark))), style),
                Span::styled(mark, t.accent()),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_account_list(
    frame: &mut Frame,
    area: Rect,
    accounts: &[Account],
    selected: usize,
    me: Option<&str>,
    t: &Theme,
) {
    let inner = popup(
        frame,
        area,
        LIST_W,
        accounts.len().max(1) as u16 + 4,
        "Accounts",
        t,
    );
    let width = usize::from(inner.width);
    let rows = inner.height.saturating_sub(2);
    let mut lines: Vec<Line> = accounts
        .iter()
        .enumerate()
        .skip(first_shown(selected, rows))
        .take(usize::from(rows.max(1)))
        .map(|(i, a)| {
            let marker = if i == selected { "▶ " } else { "  " };
            let used = if me == Some(a.host.as_str()) {
                "  in use"
            } else {
                ""
            };
            let name = truncate(
                &format!("@{}@{}", a.username, a.host),
                width.saturating_sub(cells(marker) + cells(used)),
            );
            let style = if i == selected {
                t.base().bold()
            } else {
                t.base()
            };
            Line::from(vec![
                Span::styled(marker, t.accent().bold()),
                Span::styled(name, style),
                Span::styled(used, t.accent()),
            ])
        })
        .collect();
    if accounts.is_empty() {
        lines.push(Line::styled("  no account yet: run `msky login`", t.dim()));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        truncate(" enter use  x log out  a log in another", width),
        t.dim(),
    ));
    frame.render_widget(Paragraph::new(lines), inner);
}

/// The login form: the servers to choose from, or the host being typed,
/// or the address to allow msky at while the login waits for it.
fn draw_login(frame: &mut Frame, area: Rect, f: &LoginForm, t: &Theme) {
    let w = HELP_W.min(area.width);
    let h = area.height.saturating_sub(4).clamp(8, 24);
    let inner = popup(frame, area, w, h, "Log in", t);
    let width = usize::from(inner.width.max(1));
    let mut lines: Vec<Line> = Vec::new();
    if let Some(wait) = &f.waiting {
        let host = crate::api::authority(&wait.origin);
        lines.extend(
            wrap(
                &format!("Open this address, signed in on {host}, and allow msky to read your account, post notes and follow:"),
                width,
            )
            .into_iter()
            .map(Line::from),
        );
        lines.push(Line::raw(""));
        // The address whole, broken anywhere, so it can be copied.
        let chars: Vec<char> = wait.url.chars().collect();
        for chunk in chars.chunks(width) {
            lines.push(Line::styled(chunk.iter().collect::<String>(), t.accent()));
        }
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            truncate("waiting for it…  o opens it again, esc calls it off", width),
            t.dim(),
        ));
        frame.render_widget(Paragraph::new(lines), inner);
        return;
    }
    lines.push(Line::styled(
        truncate(
            " The server to log in on (MiAuth: msky never sees the password)",
            width,
        ),
        t.dim(),
    ));
    let foot: u16 = if f.error.is_some() { 4 } else { 2 };
    let rows = inner.height.saturating_sub(1 + foot);
    if let Some(typed) = &f.typed {
        lines.push(Line::from(vec![
            Span::styled(" host: ", t.accent().bold()),
            Span::raw(truncate(&format!("{typed}▏"), width.saturating_sub(7))),
        ]));
    } else {
        for (i, s) in f
            .servers
            .iter()
            .enumerate()
            .skip(first_shown(f.selected, rows))
            .take(usize::from(rows.max(1)))
        {
            let marker = if i == f.selected { "▶ " } else { "  " };
            let users = s
                .users
                .map(|n| format!("  {n} accounts"))
                .unwrap_or_default();
            let style = if i == f.selected {
                t.base().bold()
            } else {
                t.base()
            };
            lines.push(truncate_line(
                Line::from(vec![
                    Span::styled(marker, t.accent().bold()),
                    Span::styled(s.host.clone(), style),
                    Span::styled(format!("  {}", s.name), t.base()),
                    Span::styled(users, t.dim()),
                ]),
                width,
            ));
        }
    }
    let body_h = 1 + rows;
    while (lines.len() as u16) < body_h {
        lines.push(Line::raw(""));
    }
    lines.truncate(usize::from(body_h));
    if let Some(e) = &f.error {
        lines.extend(
            wrap(e, width)
                .into_iter()
                .take(2)
                .map(|l| Line::styled(l, t.error())),
        );
    }
    let note = if f.loading {
        " reading Misskey Hub's list…"
    } else {
        " / another server  enter log in  esc back"
    };
    lines.push(Line::raw(""));
    lines.push(Line::styled(truncate(note, width), t.dim()));
    frame.render_widget(Paragraph::new(lines), inner);
}

/// The largest box of cells a `w` x `h` picture fits in within `area`, at
/// its own shape, centered, as bsky's.
fn fit(area: Rect, (w, h): (u32, u32), (cw, ch): (u16, u16)) -> Rect {
    let (cw, ch) = (f64::from(cw.max(1)), f64::from(ch.max(1)));
    let (aw, ah) = (f64::from(area.width) * cw, f64::from(area.height) * ch);
    let scale = (aw / f64::from(w.max(1))).min(ah / f64::from(h.max(1)));
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

/// A byte count the way a person reads it.
fn human_bytes(n: u64) -> String {
    match n {
        n if n >= 1024 * 1024 => format!("{:.1} MB", n as f64 / (1024.0 * 1024.0)),
        n if n >= 1024 => format!("{} KB", n / 1024),
        n => format!("{n} B"),
    }
}

/// A note's files full screen, the one at `index` shown at its own shape,
/// centered, with its number, name and alt text under it. A file that is
/// not a picture is named, with the keys that save or open it.
fn draw_viewer(
    frame: &mut Frame,
    area: Rect,
    files: &[crate::tui::app::Attached],
    index: usize,
    replay: u32,
    images: &mut Images,
    t: &Theme,
) {
    frame.render_widget(Clear, area);
    frame.render_widget(Block::new().style(t.base()), area);
    let Some((_, _, f)) = files.get(index) else {
        return;
    };
    let [pic, caption] = Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(area);
    let mut head = vec![Span::styled(
        format!(" {}/{}", index + 1, files.len()),
        t.accent().bold(),
    )];
    head.push(Span::raw(format!(
        "  {}  {} · {}",
        crate::tui::text::one_line(&drawable(&f.name)),
        crate::tui::text::one_line(&drawable(&f.mime)),
        human_bytes(f.size)
    )));
    if f.is_sensitive {
        head.push(Span::styled("  ⚠ sensitive", t.warn()));
    }
    let url = f.url.as_deref().unwrap_or("");
    if f.is_picture() && !url.is_empty() {
        let shape = images.dims(url).or(f.aspect());
        let r = match shape {
            Some(d) => fit(pic, d, images.cell_size()),
            None => pic,
        };
        // The thumbnail at once, the full size as soon as it is here.
        let thumb = f.thumbnail_url.as_deref().unwrap_or("");
        images.draw_first(frame, r, &[url, thumb]);
    } else if f.mime.starts_with("video/") && !url.is_empty() {
        let r = match f.aspect() {
            Some(d) => fit(pic, d, images.cell_size()),
            None => pic,
        };
        let state = images.draw_video(frame, r, url, replay);
        if !images.video_shown()
            && let Some(thumb) = f.thumbnail_url.as_deref()
        {
            images.draw(frame, r, thumb);
        }
        use crate::tui::player::State;
        let (text, style) = match state {
            State::Loading => ("  loading the video…".to_string(), t.dim()),
            State::Playing => ("  ▶ playing (no sound)".to_string(), t.accent()),
            State::Ended => ("  ■ ended  r plays it again".to_string(), t.dim()),
            State::Warning(why) => (format!("  ⚠ {why}; d saves it, o opens it"), t.error()),
        };
        head.push(Span::styled(text, style));
    } else {
        let what = if f.mime.starts_with("video/") {
            "▶ a video: d saves it, o opens it in the browser"
        } else {
            "a file: d saves it, o opens it in the browser"
        };
        let lines = vec![Line::styled(what, t.dim()).centered()];
        frame.render_widget(
            Paragraph::new(lines),
            Rect {
                y: pic.y + pic.height / 2,
                height: 1,
                ..pic
            },
        );
    }
    let alt = match f.comment.as_deref().filter(|c| !c.trim().is_empty()) {
        Some(c) => Line::from(format!(
            " {}",
            truncate(
                &crate::tui::text::one_line(&drawable(c)),
                usize::from(caption.width.saturating_sub(2))
            )
        )),
        None => Line::styled(" (no alt text)", t.dim()),
    };
    frame.render_widget(
        Paragraph::new(vec![
            truncate_line(Line::from(head), usize::from(caption.width)),
            alt,
        ]),
        caption,
    );
}

fn draw_help(frame: &mut Frame, area: Rect, scroll: &mut u16, pictures: bool, t: &Theme) {
    let inner_w = usize::from(HELP_W.min(area.width).saturating_sub(2));
    let beside = inner_w >= HELP_KEYS + 20;
    let mut lines: Vec<Line> = Vec::new();
    for (i, (title, keys)) in keys::help(pictures).into_iter().enumerate() {
        if i > 0 {
            lines.push(Line::raw(""));
        }
        lines.push(Line::styled(format!(" {title}"), Style::new().bold()));
        for (k, d) in keys {
            if beside {
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
    let max = (lines.len() as u16).saturating_sub(body.height);
    *scroll = (*scroll).min(max);
    frame.render_widget(Paragraph::new(lines).scroll((*scroll, 0)), body);
    let more = if *scroll < max { "  j/pgdn more" } else { "" };
    frame.render_widget(
        Paragraph::new(format!(" esc/q/? close{more}")).style(t.dim()),
        foot,
    );
}
