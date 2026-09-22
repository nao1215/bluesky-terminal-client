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
use crate::api::{MAX_POST_GRAPHEMES, grapheme_len};
use crate::media;
use crate::terminal::protocol_name;
use crate::tui::app::{
    App, Compose, EditProfile, List, LoginForm, Overlay, SearchMode, Tab, ThreadView,
};
use crate::tui::files::{Browser, EntryKind};
use crate::tui::images::Images;
use crate::tui::input::TextInput;
use crate::tui::keys;
use crate::tui::player::State;
use crate::tui::text::{format_time, truncate, wrap};
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
/// Most rows the key hints take on a narrow screen.
const MAX_HINT_ROWS: u16 = 3;
/// Themes the picker shows at once; the rest scroll.
const THEME_ROWS: usize = 10;
/// Widest the error box gets, in cells.
const ERROR_W: u16 = 76;
/// Size of a picture's thumbnail in the composer, in cells.
const THUMB: (u16, u16) = (14, 5);

/// Draw the whole UI.
pub fn draw(frame: &mut Frame, app: &mut App, images: &mut Images) {
    let area = frame.area();
    let t = app.theme;
    images.begin_frame(Size::new(area.width, area.height), t.dim());
    // Every cell starts in the theme's colors, so a theme with its own
    // background covers the whole terminal, not only the cells with text.
    frame.render_widget(Block::new().style(t.base()), area);
    if let Some(form) = &app.login {
        draw_login(frame, area, form, &t);
        return;
    }
    // The key hints wrap onto more rows on a narrow screen rather than
    // being cut off.
    let hint_lines = hint_lines(&keys::hints(app), area.width, &t);
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
        Some(Overlay::Help { scroll }) => draw_help(frame, area, scroll, &t),
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
        Media::Image { url, alt, aspect } => (Some(url.as_str()), alt, *aspect),
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
        Media::Image { .. } => {
            if let Some(url) = url {
                images.draw(frame, r, url);
            }
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
        Tab::Timeline => {
            let empty = "No posts from accounts you follow yet. Press R to refresh.";
            draw_posts(frame, body, &mut app.timeline, images, empty, None, &t);
        }
        Tab::Search => draw_search(frame, body, app, images),
        Tab::Profile => draw_profile(frame, body, app, images),
        Tab::Notifications => draw_notifications(frame, body, app, images),
    }
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
    let mut spans = vec![Span::styled(" bs ", t.badge()), Span::raw(" ")];
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
        format!("{} ", protocol_name(images.protocol_type()))
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
    viewport: u16,
    mut height: impl FnMut(usize) -> u16,
) -> usize {
    if selected <= offset {
        return selected;
    }
    let viewport = u32::from(viewport);
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
    fn new(post: &Post, width: u16, cell: (u16, u16), me: Option<&str>, t: &Theme) -> Self {
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
        if let Some(embed) = &post.embed {
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
            stats: Line::default(),
        }
    }

    fn height(&self) -> u16 {
        let ctx = self.context.len() as u16;
        let content = ctx + 2 + self.body.len() as u16 + self.image_rows;
        content.max(ctx + AVATAR.1) + 1
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

fn truncate_line(line: Line<'static>, width: usize) -> Line<'static> {
    let mut left = width;
    let mut spans = Vec::new();
    for span in line.spans {
        if left == 0 {
            break;
        }
        let text = truncate(&span.content, left);
        left = left.saturating_sub(text.width());
        spans.push(Span::styled(text, span.style));
    }
    Line::from(spans)
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
        Embed::Record { record } => quote_line(record, width, t).into_iter().collect(),
        Embed::Video { .. } => vec![Line::styled("▶ video", dim)],
        Embed::Images { .. } | Embed::RecordWithMedia { .. } | Embed::Other => Vec::new(),
    }
}

fn quote_line(record: &serde_json::Value, width: usize, t: &Theme) -> Option<Line<'static>> {
    let handle = record.pointer("/author/handle")?.as_str()?;
    let text = record
        .pointer("/value/text")
        .and_then(|t| t.as_str())
        .unwrap_or("");
    let first = text.lines().next().unwrap_or("");
    Some(Line::styled(
        truncate(&format!("❝ @{handle}: {first}"), width),
        t.dim(),
    ))
}

/// Width left for a row's text once its indentation is taken off.
fn content_width<T: PostRow>(content: Rect, row: &T) -> u16 {
    content.width.saturating_sub(row.indent() * 2)
}

/// The content column of a list row, right of the marker and avatar.
fn content_rect(area: Rect) -> Rect {
    let left = MARK_W + AVATAR.0 + 1;
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
    cell: (u16, u16),
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
    let content = content_rect(area);
    let cell = images.cell_size();
    // Lay out only what this frame can need: the posts above the selection
    // that fit with it on screen, then as many as fill the screen. A long,
    // paged list costs no more than a short one.
    let mut lines: HashMap<usize, PostLines> = HashMap::new();
    let selected = list.selected.min(list.items.len() - 1);
    let items = &list.items;
    list.offset = scroll_offset(selected, list.offset, area.height, |i| {
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
    list.offset = scroll_offset(list.selected, list.offset, area.height, |_| H);
    let content = content_rect(area);
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
    let text_x = MARK_W + BIG_AVATAR.0 + 2;
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
        lines.push(Line::styled("this is you (e to edit)", t.dim()));
    }
    lines.extend(desc.into_iter().map(Line::from));
    let head_h = (lines.len() as u16).max(BIG_AVATAR.1) + 1;
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
    let inner = popup(frame, area, 64, 16, "Log in to Bluesky", t);
    let rows = Layout::vertical([
        Constraint::Length(2),
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
            Line::from(" Use an app password, not your account password."),
            Line::styled(" Settings → Privacy and security → App passwords", t.dim()),
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
    let title = match &c.reply {
        Some((_, handle, _)) => format!("Reply to @{handle}"),
        None => "New post".to_string(),
    };
    let n = c.media.len() as u16;
    // A row of thumbnails, then a line per picture for its alt text.
    let pics_h = if n > 0 { THUMB.1 + n } else { 0 };
    let inner = popup(frame, area, 72, 14 + pics_h, &title, t);
    let [quote, text, pics, foot] = Layout::vertical([
        Constraint::Length(if c.reply.is_some() { 2 } else { 0 }),
        Constraint::Min(1),
        Constraint::Length(pics_h),
        Constraint::Length(1),
    ])
    .areas(inner);
    if let Some((_, _, excerpt)) = &c.reply {
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
    let len = grapheme_len(c.input.text().trim_end());
    let count_style = if len > MAX_POST_GRAPHEMES {
        t.error()
    } else {
        t.dim()
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
                Span::styled(format!(" {len}/{MAX_POST_GRAPHEMES}  "), count_style),
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
    // No thumbnails behind the browser (they would only cost encodes), nor
    // when the terminal is too low to give them their rows.
    let thumbs = c.browser.is_none() && area.height >= THUMB.1 + c.media.len() as u16;
    for (i, a) in c.media.iter().enumerate() {
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
            y: area.y + THUMB.1 + i16,
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
}

/// A picture drawn in its box. A video has no frame to show (bs does not
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
    let mut out: Vec<char> = Vec::new();
    let mut used = 1;
    for ch in s.chars().rev() {
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + w > width {
            break;
        }
        used += w;
        out.push(ch);
    }
    out.push('…');
    out.into_iter().rev().collect()
}

/// The picture browser: the folder on top, its folders and pictures on the
/// left, the selected picture previewed on the right.
fn draw_browser(frame: &mut Frame, area: Rect, b: &mut Browser, images: &mut Images, t: &Theme) {
    let title = match (b.videos, b.room) {
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
        b.list.offset = scroll_offset(selected, b.list.offset, rows.height, |_| 1);
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
        Some(_) => frame.render_widget(
            Paragraph::new("enter opens the folder").style(t.dim()),
            preview,
        ),
        None => frame.render_widget(
            Paragraph::new("no folders, pictures, or videos here").style(t.dim()),
            preview,
        ),
    }
    let foot_line = match &b.note {
        Some(n) => Line::styled(format!(" {n}"), t.error()),
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

fn draw_help(frame: &mut Frame, area: Rect, scroll: &mut u16, t: &Theme) {
    let mut lines: Vec<Line> = Vec::new();
    for (i, section) in keys::HELP.iter().enumerate() {
        if i > 0 {
            lines.push(Line::raw(""));
        }
        lines.push(Line::styled(
            format!(" {}", section.title),
            Style::new().bold(),
        ));
        for (k, d) in section.keys {
            lines.push(Line::from(vec![
                Span::styled(format!("   {k:<15}"), t.accent().bold()),
                Span::raw(*d),
            ]));
        }
    }
    let inner = popup(frame, area, 64, lines.len() as u16 + 3, "Keys", t);
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
        assert!(screen.contains("App password"));
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

    #[test]
    fn empty_timeline_says_why() {
        let (mut app, _) = App::new(Some(session()), "x");
        app.handle_event(Event::Timeline(Ok(vec![].into())));
        assert!(render(&mut app, 80, 10).contains("No posts from accounts you follow"));
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
        assert!(scroll < 100, "scroll was not clamped: {scroll}");
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
                    for (w, h) in [(100, 30), (20, 6), (1, 1)] {
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
        assert_eq!(scroll_offset(3, 0, 7, h), 2);
        assert_eq!(scroll_offset(1, 2, 7, h), 1, "up past the top");
        assert_eq!(scroll_offset(2, 1, 7, h), 1, "already on screen");
        assert_eq!(scroll_offset(4, 4, 7, h), 4);
        // A post taller than the screen is drawn from its top.
        assert_eq!(scroll_offset(2, 0, 7, |i| if i == 2 { 20 } else { 3 }), 2);
    }

    #[test]
    fn jumping_to_the_end_measures_only_what_fits() {
        let mut measured = Vec::new();
        let offset = scroll_offset(9_999, 0, 10, |i| {
            measured.push(i);
            3
        });
        assert_eq!(offset, 9_997);
        assert_eq!(measured, [9_999, 9_998, 9_997, 9_996]);
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

    #[test]
    fn a_narrow_screen_shows_every_hint() {
        let (mut app, _) = App::new(Some(session()), "x");
        app.handle_event(Event::Timeline(Ok(Vec::new().into())));
        let screen = render(&mut app, 50, 24);
        for (key, what) in keys::hints(&app) {
            assert!(
                screen.contains(&format!("{key} {what}")),
                "{key} {what}:\n{screen}"
            );
        }
    }

    #[test]
    fn a_long_path_keeps_its_end() {
        assert_eq!(truncate_start("/home/me/pics", 20), "/home/me/pics");
        assert_eq!(truncate_start("/home/me/pictures/trips", 10), "…res/trips");
        // Wide characters count as two columns.
        assert_eq!(truncate_start("/ホーム/写真", 6), "…/写真");
        assert_eq!(truncate_start("/ホーム/写真", 5), "…写真");
    }
}
