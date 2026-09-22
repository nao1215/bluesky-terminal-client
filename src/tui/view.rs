//! Drawing. Every function here reads the [`App`] and writes to a frame;
//! the only state it changes is each list's scroll offset.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect, Size};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::api::types::{Embed, Post, Profile};
use crate::api::{MAX_POST_GRAPHEMES, grapheme_len};
use crate::terminal::protocol_name;
use crate::tui::app::{App, Compose, EditProfile, List, LoginForm, Overlay, SearchMode, Tab};
use crate::tui::images::Images;
use crate::tui::input::TextInput;
use crate::tui::text::{format_time, truncate, wrap};

const ACCENT: Color = Color::Cyan;
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

/// Draw the whole UI.
pub fn draw(frame: &mut Frame, app: &mut App, images: &mut Images) {
    let area = frame.area();
    images.begin_frame(Size::new(area.width, area.height));
    if let Some(form) = &app.login {
        draw_login(frame, area, form);
        return;
    }
    let [top, body, bottom] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(area);
    draw_tabs(frame, top, app);
    match app.tab {
        Tab::Timeline => {
            let empty = "No posts from accounts you follow yet. Press R to refresh.";
            draw_posts(frame, body, &mut app.timeline, images, empty);
        }
        Tab::Search => draw_search(frame, body, app, images),
        Tab::Profile => draw_profile(frame, body, app, images),
    }
    draw_status(frame, bottom, app, images);
    match &app.overlay {
        Some(Overlay::Compose(c)) => draw_compose(frame, area, c),
        Some(Overlay::EditProfile(e)) => draw_edit_profile(frame, area, e),
        Some(Overlay::Help) => draw_help(frame, area),
        None => {}
    }
}

fn draw_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let mut spans = vec![
        Span::styled(" bs ", Style::new().black().on_cyan().bold()),
        Span::raw(" "),
    ];
    for (i, tab) in Tab::ALL.iter().enumerate() {
        let label = format!(" {} {} ", i + 1, tab.title());
        let style = if *tab == app.tab {
            Style::new()
                .fg(ACCENT)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::new().dark_gray()
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }
    frame.render_widget(Line::from(spans), area);
    if let Some(s) = &app.session {
        let who = format!("@{} ", s.handle);
        let w = who.width() as u16;
        if w < area.width {
            let r = Rect {
                x: area.right() - w,
                width: w,
                ..area
            };
            frame.render_widget(Paragraph::new(who).dark_gray(), r);
        }
    }
}

fn draw_status(frame: &mut Frame, area: Rect, app: &App, images: &Images) {
    let right = if app.pending > 0 || images.loading() {
        "loading… ".to_string()
    } else {
        format!("{} ", protocol_name(images.protocol_type()))
    };
    let left = match &app.status {
        Some(s) if s.error => Line::from(Span::styled(
            format!(" {}", s.text),
            Style::new().red().bold(),
        )),
        Some(s) => Line::from(Span::styled(format!(" {}", s.text), Style::new().green())),
        None => Line::from(Span::styled(
            format!(" {}", hints(app)),
            Style::new().dark_gray(),
        )),
    };
    frame.render_widget(left, area);
    let w = right.width() as u16;
    if w < area.width {
        frame.render_widget(
            Paragraph::new(right).dark_gray(),
            Rect {
                x: area.right() - w,
                width: w,
                ..area
            },
        );
    }
}

fn hints(app: &App) -> &'static str {
    match app.tab {
        Tab::Timeline => {
            "j/k move  l like  r reply  n post  f unfollow  enter profile  R refresh  ? help  q quit"
        }
        Tab::Search if app.search.editing => "enter search  ctrl+t posts/accounts  esc done",
        Tab::Search if app.search.mode == SearchMode::Accounts => {
            "/ edit query  t posts/accounts  f follow/unfollow  enter profile  ? help"
        }
        Tab::Search => "/ edit query  t posts/accounts  l like  r reply  enter profile  ? help",
        Tab::Profile if app.profile.actor.is_some() => {
            "f follow/unfollow  l like  r reply  esc my profile  ? help"
        }
        Tab::Profile => "e edit profile  l like  r reply  R reload  ? help",
    }
}

/// Keep `list.selected` on screen given each item's height.
fn scroll<T>(list: &mut List<T>, heights: &[u16], viewport: u16) {
    if list.selected < list.offset {
        list.offset = list.selected;
    }
    while list.offset < list.selected {
        let used: u32 = heights[list.offset..=list.selected]
            .iter()
            .map(|h| u32::from(*h))
            .sum();
        if used <= u32::from(viewport) {
            break;
        }
        list.offset += 1;
    }
}

/// Everything a post draws besides images, precomputed for a width.
struct PostLines {
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
    fn new(post: &Post, width: u16, cell: (u16, u16)) -> Self {
        let width = usize::from(width.max(1));
        let record = post.record();
        let time = format_time(record.created_at.as_deref().unwrap_or(&post.indexed_at));
        let mut header = vec![
            Span::styled(post.author.name().to_string(), Style::new().bold()),
            Span::styled(
                format!(" @{}", post.author.handle),
                Style::new().dark_gray(),
            ),
            Span::styled(format!(" · {time}"), Style::new().dark_gray()),
        ];
        if record.reply.is_some() {
            header.push(Span::styled(" ↩ reply", Style::new().dark_gray()));
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
            body.extend(embed_lines(embed, width));
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
        let heart_style = if liked {
            Style::new().red()
        } else {
            Style::new().dark_gray()
        };
        let stats = Line::from(vec![
            Span::styled(format!("{heart} {}", post.like_count), heart_style),
            Span::styled(
                format!("   ⟳ {}   ↩ {}", post.repost_count, post.reply_count),
                Style::new().dark_gray(),
            ),
        ]);
        Self {
            header,
            body,
            images,
            image_rows: rows,
            stats,
        }
    }

    fn height(&self) -> u16 {
        let content = 2 + self.body.len() as u16 + self.image_rows;
        content.max(AVATAR.1) + 1
    }
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

fn embed_lines(embed: &Embed, width: usize) -> Vec<Line<'static>> {
    let dim = Style::new().dark_gray();
    match embed {
        Embed::External { external } => {
            let title = if external.title.is_empty() {
                &external.uri
            } else {
                &external.title
            };
            vec![
                Line::styled(
                    truncate(&format!("🔗 {title}"), width),
                    Style::new().fg(ACCENT),
                ),
                Line::styled(truncate(&external.uri, width), dim),
            ]
        }
        Embed::Record { record } => quote_line(record, width).into_iter().collect(),
        Embed::Video { .. } => vec![Line::styled("▶ video", dim)],
        Embed::Images { .. } | Embed::RecordWithMedia { .. } | Embed::Other => Vec::new(),
    }
}

fn quote_line(record: &serde_json::Value, width: usize) -> Option<Line<'static>> {
    let handle = record.pointer("/author/handle")?.as_str()?;
    let text = record
        .pointer("/value/text")
        .and_then(|t| t.as_str())
        .unwrap_or("");
    let first = text.lines().next().unwrap_or("");
    Some(Line::styled(
        truncate(&format!("❝ @{handle}: {first}"), width),
        Style::new().dark_gray(),
    ))
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

fn draw_marker(frame: &mut Frame, area: Rect, height: u16, selected: bool) {
    if !selected {
        return;
    }
    let h = height.saturating_sub(1).min(area.height).max(1);
    let bar = vec![Line::from("▌"); usize::from(h)];
    frame.render_widget(
        Paragraph::new(bar).fg(ACCENT),
        Rect {
            width: 1,
            height: h,
            ..area
        },
    );
}

fn draw_posts(
    frame: &mut Frame,
    area: Rect,
    list: &mut List<Post>,
    images: &mut Images,
    empty: &str,
) {
    if !list.loaded {
        frame.render_widget(Paragraph::new(" loading…").dark_gray(), area);
        return;
    }
    if list.items.is_empty() {
        frame.render_widget(Paragraph::new(format!(" {empty}")).dark_gray(), area);
        return;
    }
    let content = content_rect(area);
    let lines: Vec<PostLines> = list
        .items
        .iter()
        .map(|p| PostLines::new(p, content.width, images.cell_size()))
        .collect();
    let heights: Vec<u16> = lines.iter().map(PostLines::height).collect();
    scroll(list, &heights, area.height);

    let mut y = area.y;
    for (i, (post, pl)) in list.items.iter().zip(&lines).enumerate().skip(list.offset) {
        if y >= area.bottom() {
            break;
        }
        let h = heights[i];
        let visible = (area.bottom() - y).min(h);
        let row = Rect {
            y,
            height: visible,
            ..area
        };
        let whole = visible == h;
        draw_marker(frame, row, h, i == list.selected);

        if whole || visible >= AVATAR.1 {
            let avatar = Rect {
                x: area.x + MARK_W,
                y,
                width: AVATAR.0,
                height: AVATAR.1,
            };
            if let Some(url) = &post.author.avatar
                && whole
            {
                images.draw(frame, avatar, url);
            }
        }
        let c = Rect {
            y,
            height: visible,
            ..content
        };
        let mut text: Vec<Line> = vec![pl.header.clone()];
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

fn account_lines(p: &Profile, width: u16) -> Vec<Line<'static>> {
    let width = usize::from(width.max(1));
    let mut head = vec![
        Span::styled(p.name().to_string(), Style::new().bold()),
        Span::styled(format!(" @{}", p.handle), Style::new().dark_gray()),
    ];
    if p.following_uri().is_some() {
        head.push(Span::styled("  ✓ following", Style::new().fg(ACCENT)));
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
        Line::styled(truncate(&desc, width), Style::new().dark_gray()),
    ]
}

fn draw_accounts(frame: &mut Frame, area: Rect, list: &mut List<Profile>, images: &mut Images) {
    if !list.loaded {
        frame.render_widget(Paragraph::new(" searching…").dark_gray(), area);
        return;
    }
    if list.items.is_empty() {
        frame.render_widget(Paragraph::new(" No accounts found.").dark_gray(), area);
        return;
    }
    const H: u16 = 3;
    let heights = vec![H; list.items.len()];
    scroll(list, &heights, area.height);
    let content = content_rect(area);
    let mut y = area.y;
    for (i, p) in list.items.iter().enumerate().skip(list.offset) {
        if y + H > area.bottom() {
            break;
        }
        draw_marker(
            frame,
            Rect {
                y,
                height: H,
                ..area
            },
            H,
            i == list.selected,
        );
        if let Some(url) = &p.avatar {
            images.draw(
                frame,
                Rect {
                    x: area.x + MARK_W,
                    y,
                    width: AVATAR.0,
                    height: AVATAR.1,
                },
                url,
            );
        }
        frame.render_widget(
            Paragraph::new(account_lines(p, content.width)),
            Rect {
                y,
                height: 2,
                ..content
            },
        );
        y += H;
    }
}

fn draw_search(frame: &mut Frame, area: Rect, app: &mut App, images: &mut Images) {
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
            Span::styled(
                format!(" {label} "),
                Style::new().fg(ACCENT).add_modifier(Modifier::REVERSED),
            )
        } else {
            Span::styled(format!(" {label} "), Style::new().dark_gray())
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
    frame.render_widget(Paragraph::new(prompt).fg(ACCENT), input);
    if app.search.input.is_empty() && !app.search.editing {
        frame.render_widget(Paragraph::new("press / to type").dark_gray(), field);
    } else {
        draw_single_input(frame, field, &app.search.input, app.search.editing);
    }
    let _ = gap;
    match mode {
        SearchMode::Posts => {
            if app.search.posts.loaded || !app.search.posts.items.is_empty() || app.pending > 0 {
                draw_posts(
                    frame,
                    results,
                    &mut app.search.posts,
                    images,
                    "No posts found.",
                );
            }
        }
        SearchMode::Accounts => {
            if app.search.actors.loaded || app.pending > 0 {
                draw_accounts(frame, results, &mut app.search.actors, images);
            }
        }
    }
}

fn draw_profile(frame: &mut Frame, area: Rect, app: &mut App, images: &mut Images) {
    let Some(p) = app.profile.profile.clone() else {
        let msg = match &app.profile.error {
            Some(e) => {
                Paragraph::new(format!(" could not load the profile: {e}  (R to retry)")).red()
            }
            None => Paragraph::new(" loading…").dark_gray(),
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
        Line::styled(format!("@{}", p.handle), Style::new().dark_gray()),
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
            Span::styled("✓ following", Style::new().fg(ACCENT).bold())
        } else {
            Span::styled("not following", Style::new().dark_gray())
        }];
        if viewer.followed_by.is_some() {
            rel.push(Span::styled("  · follows you", Style::new().dark_gray()));
        }
        rel.push(Span::styled("  (f to toggle)", Style::new().dark_gray()));
        lines.push(Line::from(rel));
    } else {
        lines.push(Line::styled(
            "this is you (e to edit)",
            Style::new().dark_gray(),
        ));
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
    draw_posts(frame, feed, &mut app.profile.posts, images, "No posts yet.");
}

/// A box of `w` x `h` centered in `area`, cleared.
fn popup(frame: &mut Frame, area: Rect, w: u16, h: u16, title: &str) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    let r = Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    };
    frame.render_widget(Clear, r);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(ACCENT))
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

fn draw_login(frame: &mut Frame, area: Rect, form: &LoginForm) {
    let inner = popup(frame, area, 64, 16, "Log in to Bluesky");
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
            Line::styled(
                " Settings → Privacy and security → App passwords",
                Style::new().dark_gray(),
            ),
        ]),
        rows[0],
    );
    for (i, label) in LoginForm::LABELS.iter().enumerate() {
        let focused = form.focus == i && !form.pending;
        let style = if focused {
            Style::new().fg(ACCENT).bold()
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
            Paragraph::new("›").fg(ACCENT),
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
        Line::styled(" logging in…", Style::new().dark_gray())
    } else if let Some(e) = &form.error {
        Line::styled(format!(" {e}"), Style::new().red().bold())
    } else {
        Line::raw("")
    };
    frame.render_widget(
        Paragraph::new(msg).wrap(ratatui::widgets::Wrap { trim: true }),
        rows[9],
    );
    frame.render_widget(
        Paragraph::new(" enter next/submit  tab switch field  esc quit").dark_gray(),
        rows[10],
    );
}

fn draw_compose(frame: &mut Frame, area: Rect, c: &Compose) {
    let title = match &c.reply {
        Some((_, handle, _)) => format!("Reply to @{handle}"),
        None => "New post".to_string(),
    };
    let inner = popup(frame, area, 72, 14, &title);
    let [quote, text, foot] = Layout::vertical([
        Constraint::Length(if c.reply.is_some() { 2 } else { 0 }),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(inner);
    if let Some((_, _, excerpt)) = &c.reply {
        frame.render_widget(
            Paragraph::new(truncate(&format!("❝ {excerpt}"), usize::from(quote.width))).dark_gray(),
            quote,
        );
    }
    let text_area = Rect {
        x: text.x + 1,
        width: text.width.saturating_sub(2),
        ..text
    };
    draw_multi_input(frame, text_area, &c.input, !c.sending);
    let n = grapheme_len(c.input.text().trim_end());
    let count_style = if n > MAX_POST_GRAPHEMES {
        Style::new().red().bold()
    } else {
        Style::new().dark_gray()
    };
    let action = if c.sending {
        "sending…"
    } else {
        "ctrl+s send  esc cancel"
    };
    frame.render_widget(
        Line::from(vec![
            Span::styled(format!(" {n}/{MAX_POST_GRAPHEMES}  "), count_style),
            Span::styled(action, Style::new().dark_gray()),
        ]),
        foot,
    );
}

fn draw_edit_profile(frame: &mut Frame, area: Rect, e: &EditProfile) {
    let inner = popup(frame, area, 72, 16, "Edit profile");
    if e.loading {
        frame.render_widget(Paragraph::new(" loading…").dark_gray(), inner);
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
            Style::new().fg(ACCENT).bold()
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
        " tab next field  ctrl+s save  esc cancel"
    };
    frame.render_widget(
        Paragraph::new(action).dark_gray(),
        Rect { height: 1, ..foot },
    );
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let rows: &[(&str, &str)] = &[
        ("1 2 3 tab", "switch Timeline, Search, Profile"),
        ("j k ↑ ↓", "move the selection (g/G top/bottom)"),
        ("n", "new post"),
        ("r", "reply to the selected post"),
        ("l", "like / remove like"),
        ("f", "follow / unfollow the selected account"),
        ("enter", "open the selected account's profile"),
        ("/", "search (ctrl+t or t: posts or accounts)"),
        ("e", "edit your profile (Profile tab)"),
        ("R F5", "refresh the current view"),
        ("ctrl+s", "send the post / save the profile"),
        ("esc", "close a window, back to your profile"),
        ("q ctrl+c", "quit"),
    ];
    let inner = popup(frame, area, 60, rows.len() as u16 + 4, "Keys");
    let mut lines: Vec<Line> = rows
        .iter()
        .map(|(k, d)| {
            Line::from(vec![
                Span::styled(format!(" {k:<12}"), Style::new().fg(ACCENT).bold()),
                Span::raw(*d),
            ])
        })
        .collect();
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        " press any key to close",
        Style::new().dark_gray(),
    ));
    frame.render_widget(Paragraph::new(lines), inner);
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
        let mut images = Images::new(Picker::halfblocks());
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
        app.handle_event(Event::Timeline(Ok(posts(20))));
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
        app.handle_event(Event::Timeline(Ok(vec![])));
        assert!(render(&mut app, 80, 10).contains("No posts from accounts you follow"));
    }

    #[test]
    fn tiny_terminals_do_not_panic() {
        let (mut app, _) = App::new(Some(session()), "x");
        app.handle_event(Event::Timeline(Ok(posts(3))));
        for (w, h) in [(1, 1), (5, 3), (10, 2), (20, 5)] {
            render(&mut app, w, h);
        }
        app.overlay = Some(Overlay::Help);
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
    fn scroll_moves_offset_only_as_far_as_needed() {
        let mut list: List<u8> = List {
            items: vec![0; 5],
            selected: 3,
            offset: 0,
            loaded: true,
        };
        scroll(&mut list, &[3, 3, 3, 3, 3], 7);
        assert_eq!(list.offset, 2);
        list.selected = 1;
        scroll(&mut list, &[3, 3, 3, 3, 3], 7);
        assert_eq!(list.offset, 1);
    }
}
