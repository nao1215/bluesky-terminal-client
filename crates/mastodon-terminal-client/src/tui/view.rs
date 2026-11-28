//! Drawing the client, as bsky and msky draw theirs: the tab row, the list
//! or thread shown, the keys that work there, and the status row; the help,
//! the actions, the theme picker and the composer over them.
//!
//! Everything a server wrote reaches the screen through
//! [`terminal_kit::text::drawable`] (in [`wrap`], [`truncate`] and
//! [`truncate_line`]), so an escape sequence in a post is drawn as text.

use ratatui::Frame;
use terminal_kit::media::images::Images;
mod accounts;
mod browse;
mod chat;
mod deck;
mod edit_profile;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use terminal_kit::text::{cells, cluster_width, drawable, format_time, graphemes, truncate, wrap};

use crate::api::types::{Account, Media, Notification, Status};
use crate::i18n::{n, t, tf};
use crate::tui::app::{App, Compose, Field, Overlay, Tab, visibility_label};
use crate::tui::keys;
use crate::tui::list::{Keyed, List, Row};
use crate::tui::theme::{self, THEMES, Theme};

/// The marker column left of a list's items.
const MARK_W: u16 = 2;
/// How far in a reply is drawn per level of its thread.
const INDENT: u16 = 2;
/// The most rows the hints take.
const MAX_HINT_ROWS: u16 = 2;
const ERROR_W: u16 = 76;
const LIST_W: u16 = 52;
const HELP_W: u16 = 64;
const COMPOSE_W: u16 = 72;
/// The smallest terminal the client draws in.
pub(crate) const MIN_W: u16 = 24;
pub(crate) const MIN_H: u16 = 8;

#[cfg(test)]
pub(crate) fn draw(frame: &mut Frame, app: &mut App) {
    draw_with_images(frame, app, &mut Images::none());
}

pub(crate) fn draw_with_images(frame: &mut Frame, app: &mut App, images: &mut Images) {
    let area = frame.area();
    let th = app.theme;
    images.begin_frame(area.as_size(), th.dim());
    frame.render_widget(Block::new().style(th.base()), area);
    if area.width < MIN_W || area.height < MIN_H {
        draw_too_small(frame, area, &th);
        return;
    }
    let hint_lines = hint_lines(&keys::hints(app), area.width, &th);
    let hint_h = (hint_lines.len() as u16).clamp(1, MAX_HINT_ROWS);
    let [top, body, hint_row, status_row] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(hint_h),
        Constraint::Length(1),
    ])
    .areas(area);
    draw_tabs(frame, top, app);
    let opened = app.opened.clone();
    let ctx = Ctx {
        opened: &opened,
        t: th,
    };
    if let Some(thread) = app.threads.last_mut() {
        draw_thread(frame, body, &mut thread.rows, &ctx, images);
    } else if app.tab == Tab::Chat {
        chat::draw(frame, body, app, images, &ctx);
    } else if app.in_deck() {
        deck::draw(frame, body, app, images, &ctx);
    } else if matches!(app.tab, Tab::Search | Tab::Profile) {
        browse::draw_browse(frame, body, app, images, &ctx);
    } else {
        match app.tab.kind() {
            Some(kind) => {
                let i = Tab::ALL.iter().position(|t| *t == app.tab).unwrap_or(0);
                let login = app.tab.needs_login() && app.session.is_none();
                let host = app.host().to_string();
                let list = &mut app.timelines[i];
                if login {
                    draw_needs_login(frame, body, &host, &th);
                } else {
                    let empty = match kind {
                        crate::cli::TimelineKind::Home => {
                            n!("No posts from the accounts you follow yet. Press R to refresh.")
                        }
                        _ => n!("No posts yet. Press R to refresh."),
                    };
                    draw_statuses(frame, body, list, empty, &ctx, images);
                }
            }
            None if app.session.is_none() => {
                let host = app.host().to_string();
                draw_needs_login(frame, body, &host, &th);
            }
            None => draw_notifications(frame, body, &mut app.notifications, &ctx, images),
        }
    }
    frame.render_widget(Paragraph::new(hint_lines), hint_row);
    draw_status(frame, status_row, app, images);
    let counted = match &app.overlay {
        Some(Overlay::Compose(c)) => Some(app.counted(c)),
        _ => None,
    };
    let actions: Vec<String> = match &app.overlay {
        Some(Overlay::Actions { .. }) => keys::actions(app)
            .iter()
            .map(|(k, d)| format!("{k:<6} {d}"))
            .collect(),
        _ => Vec::new(),
    };
    let column_choices = app.column_choices();
    let settings_rows = app.settings_rows();
    match &mut app.overlay {
        Some(Overlay::Login {
            input,
            url,
            waiting,
        }) => accounts::login(frame, body, input, url.as_deref(), *waiting, &th),
        Some(Overlay::Logout { host }) => {
            let inner = popup(frame, body, 64, 6, t("Log out"), &th);
            frame.render_widget(
                wrapped(
                    &tf("Remove the saved login for {}? y confirms", &[host]),
                    inner.width,
                ),
                inner,
            );
        }
        Some(Overlay::AddColumn { selected, query }) => {
            deck::picker(frame, area, *selected, query, &column_choices, &th)
        }
        Some(Overlay::CloseColumn(_)) => {
            let inner = popup(frame, area, 64, 5, t("Close column?"), &th);
            frame.render_widget(
                Paragraph::new(t("y confirms; any other key cancels")),
                inner,
            );
        }
        Some(Overlay::EditProfile(e)) => edit_profile::draw(frame, area, e, images, &th),
        Some(Overlay::Confirm(c)) => {
            let title = match c {
                crate::tui::app::Confirmation::Delete(_) => t("Delete this post?"),
                crate::tui::app::Confirmation::Relation {
                    action: crate::tui::worker::RelationAction::Mute,
                    ..
                } => t("Toggle mute for this account?"),
                _ => t("Toggle block for this account?"),
            };
            let inner = popup(frame, area, 64, 5, title, &th);
            frame.render_widget(
                wrapped(t("y confirms; any other key cancels"), inner.width),
                inner,
            );
        }
        Some(Overlay::Report(r)) => draw_report(frame, area, r, &th),
        Some(Overlay::Help { scroll }) => draw_help(frame, area, scroll, &th),
        Some(Overlay::Actions { selected }) => {
            draw_choices(frame, body, t("Actions"), &actions, *selected, &th);
        }
        Some(Overlay::Themes { selected, .. }) => {
            let rows: Vec<String> = THEMES.iter().map(|t| t.name.to_string()).collect();
            draw_choices(frame, body, t("Theme"), &rows, *selected, &th);
        }
        Some(Overlay::Compose(c)) => {
            draw_compose(frame, area, c, counted.unwrap_or((0, 0)), &th);
            if let Some(b) = &mut c.browser {
                b.draw(frame, area, images, &th, t);
            }
        }
        Some(Overlay::Viewer {
            files,
            index,
            generation,
        }) => browse::draw_viewer(frame, body, files, *index, *generation, images, &th),
        Some(Overlay::SettingText { row, input }) => {
            let inner = popup(
                frame,
                body,
                72,
                7,
                t(if *row == 2 {
                    n!("Download folder")
                } else {
                    n!("Browser")
                }),
                &th,
            );
            let layout = input.layout(usize::from(inner.width.max(1)));
            frame.render_widget(Paragraph::new(layout.lines.join("\n")), inner);
            frame.set_cursor_position((
                inner.x + (layout.cursor.1 as u16).min(inner.width.saturating_sub(1)),
                inner.y,
            ));
        }
        Some(Overlay::Settings { selected }) => {
            let rows = settings_rows;
            draw_choices(frame, body, t("Settings"), &rows, *selected, &th);
        }
        Some(Overlay::Languages { selected }) => draw_choices(
            frame,
            body,
            t("Language"),
            &["English".into(), "日本語".into()],
            *selected,
            &th,
        ),
        Some(Overlay::Accounts { selected }) => {
            let rows: Vec<_> = app
                .accounts
                .iter()
                .map(|a| {
                    format!(
                        "{} @{}@{}",
                        if app.session.as_ref().is_some_and(|s| s.service == a.service) {
                            "*"
                        } else {
                            " "
                        },
                        crate::text::one_line(&a.username),
                        a.host()
                    )
                })
                .collect();
            let rows = if rows.is_empty() {
                vec![t("No saved accounts. Press a to log in.").into()]
            } else {
                rows
            };
            draw_choices(frame, body, t("Accounts"), &rows, *selected, &th);
        }
        None => {}
    }
    if let Some(m) = app.message.as_ref().filter(|m| m.error) {
        draw_error(frame, area, &m.text, &th);
    }
}

/// What every status is drawn with.
struct Ctx<'a> {
    /// The statuses whose warning `w` opened.
    opened: &'a std::collections::HashSet<String>,
    t: Theme,
}

/// The screen for a terminal too small for the client, as bsky's says.
fn draw_too_small(frame: &mut Frame, area: Rect, th: &Theme) {
    let w = usize::from(area.width).max(1);
    let mut lines: Vec<Line> = wrap(t("Terminal too small"), w)
        .into_iter()
        .map(|l| Line::styled(l, th.accent().bold()).centered())
        .collect();
    let size = tf(
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

/// An error in a box in the middle of the screen, over everything. It goes
/// with the next key, or by itself after a while.
fn draw_error(frame: &mut Frame, area: Rect, text: &str, th: &Theme) {
    let w = ERROR_W.min(area.width.saturating_sub(4)).max(10);
    let inner_w = usize::from(w.saturating_sub(4)).max(1);
    let mut lines: Vec<Line> = wrap(text, inner_w)
        .into_iter()
        .map(|l| Line::styled(l, th.error()))
        .collect();
    lines.push(Line::default());
    lines.push(Line::styled(t("any key closes this"), th.dim()));
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
        .title(format!(" {} ", t("Error")))
        .border_style(th.error())
        .style(th.base());
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
    let th = app.theme;
    let mut spans = Vec::new();
    for (i, &tab) in Tab::ALL.iter().enumerate() {
        let count = if tab == Tab::Notifications && app.unread > 0 {
            format!(" ({})", app.unread)
        } else {
            String::new()
        };
        let label = format!(" {} {}{} ", i + 1, t(tab.title()), count);
        let style = if tab == app.tab {
            th.selected()
        } else {
            th.dim()
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }
    // Who the client reads as, at the right where it fits whole: the
    // account in use, or the server read as a visitor.
    let used: usize = spans.iter().map(|s| cells(&s.content)).sum();
    let name = match app.session.as_ref().filter(|s| !s.username.is_empty()) {
        Some(s) => format!("@{}@{} ", s.username, app.host()),
        None if app.session.is_some() => format!("{} ", app.host()),
        None => format!("{} ({}) ", app.host(), t("visitor")),
    };
    let name = drawable(&name).into_owned();
    let room = usize::from(area.width).saturating_sub(used);
    let name_w = cells(&name);
    if name_w <= room {
        spans.push(Span::raw(" ".repeat(room - name_w)));
        spans.push(Span::styled(name, th.accent()));
    }
    frame.render_widget(Line::from(spans), area);
}

/// The keys that work in the current view, packed into as many rows of
/// `width` as they need; a hint is never split across rows.
fn hint_lines(hints: &[keys::Hint], width: u16, th: &Theme) -> Vec<Line<'static>> {
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
        row.push(Span::styled(*key, th.accent().bold()));
        row.push(Span::styled(format!(" {what}"), th.dim()));
        used += w;
    }
    lines.push(row);
    lines
        .into_iter()
        .map(|spans| truncate_line(Line::from(spans), width))
        .collect()
}

/// The last message on the left; activity on the right, as bsky's.
fn draw_status(frame: &mut Frame, area: Rect, app: &App, images: &Images) {
    let th = app.theme;
    let right = if app.pending() > 0 {
        format!("{} ", t("loading…"))
    } else if images.shows() {
        format!(
            "{} ",
            terminal_kit::terminal::protocol_name(images.protocol_type())
        )
    } else {
        format!("{} ", t("text"))
    };
    let w = cells(&right) as u16;
    let relationship = app
        .account_target()
        .and_then(|a| app.relationships.get(&a.id))
        .map(App::relationship_label);
    let message = app
        .message
        .as_ref()
        .filter(|m| !m.error)
        .map(|m| m.text.as_str());
    let text = match (relationship, message) {
        (Some(r), Some(m)) => Some(format!("{r} · {m}")),
        (Some(r), None) => Some(r),
        (None, m) => m.map(str::to_owned),
    };
    if let Some(text) = text {
        let room = usize::from(area.width.saturating_sub(w + 2));
        frame.render_widget(
            Line::styled(format!(" {}", truncate(&text, room)), th.ok()),
            area,
        );
    }
    if w < area.width {
        frame.render_widget(
            Paragraph::new(right).style(th.dim()),
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
pub(crate) fn scroll_offset(
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
fn empty_message<T>(frame: &mut Frame, area: Rect, list: &List<T>, empty: &str, th: &Theme) {
    let p = match &list.error {
        Some(e) => wrapped(&format!(" {e}  {}", t("(R to retry)")), area.width).style(th.error()),
        None => wrapped(&format!(" {}", t(empty)), area.width).style(th.dim()),
    };
    frame.render_widget(p, area);
}

/// A tab the server shows only to an account, opened as a visitor.
fn draw_needs_login(frame: &mut Frame, area: Rect, host: &str, th: &Theme) {
    let text = tf(
        "{} shows this only to accounts that are logged in; run `mstdn login --host {}`",
        &[host, host],
    );
    frame.render_widget(
        wrapped(&format!(" {text}"), area.width).style(th.dim()),
        area,
    );
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

/// `@name` for an account of the server read, `@name@host` for one of
/// another.
pub(crate) fn acct(a: &Account) -> String {
    format!("@{}", a.acct)
}

/// The name an account shows, else its user name.
pub(crate) fn display_name(a: &Account) -> String {
    let name = a.display_name.trim();
    if name.is_empty() {
        a.username.clone()
    } else {
        name.to_string()
    }
}

fn emoji_text(
    text: &str,
    emojis: &[crate::api::types::Emoji],
    pictures: bool,
    urls: &mut Vec<String>,
) -> String {
    let text = drawable(text);
    if !pictures {
        return text.into_owned();
    }
    terminal_kit::emoji::with_emoji_cells(
        &text,
        &mut |name| {
            emojis.iter().find(|e| e.shortcode == name).and_then(|e| {
                let url = if e.static_url.is_empty() {
                    &e.url
                } else {
                    &e.static_url
                };
                (!url.is_empty()).then(|| url.clone())
            })
        },
        urls,
    )
}

/// The text of a status, as it reads.
fn text_of(s: &Status) -> String {
    crate::html::to_text(&s.content)
}

/// The first line of what `s` says, for a line that sums it up: its warning
/// in place of a text it hides.
fn first_line(s: &Status, opened: bool) -> String {
    if !s.spoiler_text.is_empty() && !opened {
        return tf("⚠ CW: {}", &[&s.spoiler_text]);
    }
    let text = text_of(s);
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    if line.is_empty() && !s.media_attachments.is_empty() {
        return files_summary(&s.media_attachments);
    }
    line.to_string()
}

/// How many pictures, videos and other files `files` are.
fn files_summary(files: &[Media]) -> String {
    let count = |k: &[&str]| {
        files
            .iter()
            .filter(|f| k.contains(&f.kind.as_str()))
            .count()
    };
    let (pics, videos, sounds) = (
        count(&["image"]),
        count(&["video", "gifv"]),
        count(&["audio"]),
    );
    let other = files.len() - pics - videos - sounds;
    let plural = |n: usize, one: &str, many: &str| {
        if n == 1 {
            t(one).to_string()
        } else {
            tf(many, &[&n.to_string()])
        }
    };
    let mut parts = Vec::new();
    if pics > 0 {
        parts.push(format!(
            "▣ {}",
            plural(pics, n!("1 picture"), n!("{} pictures"))
        ));
    }
    if videos > 0 {
        parts.push(format!(
            "▶ {}",
            plural(videos, n!("1 video"), n!("{} videos"))
        ));
    }
    if sounds > 0 {
        parts.push(format!(
            "♪ {}",
            plural(sounds, n!("1 sound"), n!("{} sounds"))
        ));
    }
    if other > 0 {
        parts.push(plural(other, n!("1 file"), n!("{} files")));
    }
    parts.join(" · ")
}

/// The lines a status is drawn in, at `width` cells: who boosted it, its
/// header, its warning or its text, its files and its counts.
fn status_lines(item: &Status, width: u16, ctx: &Ctx) -> Vec<Line<'static>> {
    status_lines_nested(item, width, ctx, 0, false, &mut Vec::new())
}

fn status_lines_nested(
    item: &Status,
    width: u16,
    ctx: &Ctx,
    depth: usize,
    pictures: bool,
    urls: &mut Vec<String>,
) -> Vec<Line<'static>> {
    let th = &ctx.t;
    let width = usize::from(width.max(1));
    let s = item.shown();
    let mut lines = Vec::new();
    if item.reblog.is_some() {
        lines.push(Line::styled(
            truncate(
                &tf(
                    "⟳ boosted by {} {}",
                    &[
                        &emoji_text(
                            &display_name(&item.account),
                            &item.account.emojis,
                            pictures,
                            urls,
                        ),
                        &acct(&item.account),
                    ],
                ),
                width,
            ),
            th.repost(),
        ));
    }
    let mut header = vec![
        Span::styled(
            emoji_text(&display_name(&s.account), &s.account.emojis, pictures, urls),
            Style::new().bold(),
        ),
        Span::styled(format!(" {}", acct(&s.account)), th.dim()),
        Span::styled(format!(" · {}", format_time(&s.created_at)), th.dim()),
    ];
    if s.in_reply_to_id.is_some() {
        header.push(Span::styled(format!(" ↩ {}", t("reply")), th.dim()));
    }
    if let Some(v) = match s.visibility.as_str() {
        "unlisted" => Some(n!("quiet public")),
        "private" => Some(n!("followers only")),
        "direct" => Some(n!("mentioned only")),
        _ => None,
    } {
        header.push(Span::styled(format!(" · {}", t(v)), th.dim()));
    }
    lines.push(truncate_line(Line::from(header), width));
    let shown = s.spoiler_text.is_empty() || ctx.opened.contains(&s.id);
    if !s.spoiler_text.is_empty() {
        lines.extend(
            wrap(&tf("⚠ CW: {}", &[&s.spoiler_text]), width)
                .into_iter()
                .map(|l| Line::styled(l, theme::warn(th))),
        );
    }
    let text = text_of(s);
    if shown {
        let text = emoji_text(&text, &s.emojis, pictures, urls);
        lines.extend(wrap(&text, width).into_iter().map(Line::from));
    } else {
        let chars = graphemes(&text).count().to_string();
        let hidden = if s.media_attachments.is_empty() {
            tf("w shows it ({} characters)", &[&chars])
        } else {
            tf(
                "w shows it ({} characters, {} files)",
                &[&chars, &s.media_attachments.len().to_string()],
            )
        };
        lines.push(Line::styled(truncate(&hidden, width), th.dim()));
    }
    if shown && !s.media_attachments.is_empty() {
        let mut spans = Vec::new();
        if s.sensitive {
            spans.push(Span::styled(
                format!("⚠ {} ", t("sensitive")),
                theme::warn(th),
            ));
        }
        spans.push(Span::styled(files_summary(&s.media_attachments), th.dim()));
        lines.push(truncate_line(Line::from(spans), width));
        for m in &s.media_attachments {
            if let Some(alt) = m.description.as_deref().filter(|d| !d.trim().is_empty()) {
                lines.push(Line::styled(
                    truncate(&format!("  ALT {alt}"), width),
                    th.dim(),
                ));
            }
        }
    }
    if shown && let Some(card) = &s.card {
        for line in [
            card.title.as_str(),
            card.description.as_str(),
            card.url.as_str(),
        ] {
            if !line.is_empty() {
                lines.extend(
                    wrap(&crate::text::drawable(line), width.saturating_sub(2).max(1))
                        .into_iter()
                        .take(3)
                        .map(|line| Line::styled(format!("│ {line}"), th.dim())),
                );
            }
        }
    }
    if shown && let Some(p) = &s.poll {
        for o in &p.options {
            let votes = o.votes_count.map(|v| format!(" ({v})")).unwrap_or_default();
            lines.push(Line::styled(
                truncate(&format!("  ○ {}{votes}", o.title), width),
                th.dim(),
            ));
        }
    }
    if shown && let Some((state, quoted)) = s.quote_view() {
        lines.push(Line::styled(truncate(t("Quoted post"), width), th.repost()));
        if state == "accepted"
            && depth < 2
            && width > 4
            && let Some(quoted) = quoted
        {
            for mut line in
                status_lines_nested(quoted, (width - 2) as u16, ctx, depth + 1, pictures, urls)
            {
                line.spans.insert(0, Span::styled("│ ", th.repost()));
                lines.push(line);
            }
        } else {
            lines.push(Line::styled(
                truncate(t("quote unavailable or awaiting approval"), width),
                th.dim(),
            ));
        }
    }
    let (star, star_style) = if s.favourited == Some(true) {
        ("★", th.like())
    } else {
        ("☆", th.dim())
    };
    let boost_style = if s.reblogged == Some(true) {
        th.repost()
    } else {
        th.dim()
    };
    let stats = vec![
        Span::styled(format!("{star} {}", s.favourites_count), star_style),
        Span::raw("   "),
        Span::styled(format!("⟳ {}", s.reblogs_count), boost_style),
        Span::styled(format!("   ↩ {}", s.replies_count), th.dim()),
    ];
    lines.push(truncate_line(Line::from(stats), width));
    lines
}

/// The content column of a list row, right of the marker, `indent` cells
/// further in.
fn content_rect(area: Rect, indent: u16) -> Rect {
    let left = (MARK_W + 1 + indent).min(area.width);
    Rect {
        x: area.x + left,
        width: area.width.saturating_sub(left),
        ..area
    }
}

fn draw_marker(frame: &mut Frame, area: Rect, height: u16, selected: bool, th: &Theme) {
    if !selected {
        return;
    }
    let h = height.saturating_sub(1).min(area.height).max(1);
    let bar = vec![Line::from("▌"); usize::from(h)];
    frame.render_widget(
        Paragraph::new(bar).style(th.accent()),
        Rect {
            width: 1,
            height: h,
            ..area
        },
    );
}

/// An item of a list of statuses: a timeline's status, or a thread's row,
/// drawn in by its depth.
trait StatusRow {
    fn status(&self) -> &Status;
    fn indent(&self) -> u16 {
        0
    }
    fn focus(&self) -> bool {
        false
    }
}

impl StatusRow for Status {
    fn status(&self) -> &Status {
        self
    }
}

impl StatusRow for Row {
    fn status(&self) -> &Status {
        &self.status
    }
    fn indent(&self) -> u16 {
        self.depth as u16 * INDENT
    }
    fn focus(&self) -> bool {
        self.focus
    }
}

fn draw_statuses<T: StatusRow + Keyed>(
    frame: &mut Frame,
    area: Rect,
    list: &mut List<T>,
    empty: &str,
    ctx: &Ctx,
    images: &mut Images,
) {
    let th = &ctx.t;
    if !list.loaded && list.items.is_empty() {
        frame.render_widget(
            Paragraph::new(format!(" {}", t("loading…"))).style(th.dim()),
            area,
        );
        return;
    }
    if list.items.is_empty() {
        empty_message(frame, area, list, empty, th);
        return;
    }
    let mut emoji_urls = Vec::new();
    let lines: Vec<Vec<Line<'static>>> = list
        .items
        .iter()
        .map(|item| {
            let w = content_rect(area, item.indent()).width;
            let mut urls = Vec::new();
            let lines = if images.shows() {
                status_lines_nested(item.status(), w, ctx, 0, true, &mut urls)
            } else {
                status_lines(item.status(), w, ctx)
            };
            emoji_urls.push(urls);
            lines
        })
        .collect();
    let pictures: Vec<_> = list
        .items
        .iter()
        .map(|item| browse::thumbnails(item.status().shown(), ctx, images.shows()))
        .collect();
    let height = |i: usize| {
        lines.get(i).map_or(1, |l| {
            l.len() as u16 + 1 + if pictures[i].is_empty() { 0 } else { 8 }
        })
    };
    list.offset = scroll_offset(
        list.selected,
        list.offset,
        list.items.len(),
        area.height,
        height,
    );
    let mut y = area.y;
    for (i, item) in list.items.iter().enumerate().skip(list.offset) {
        if y >= area.bottom() {
            break;
        }
        let h = height(i).min(area.bottom() - y);
        let row = Rect {
            y,
            height: h,
            ..area
        };
        draw_marker(frame, row, height(i), i == list.selected, th);
        let content = content_rect(row, item.indent());
        let mut drawn = lines[i].clone();
        if item.focus() {
            for l in &mut drawn {
                l.style = l.style.patch(Style::new().bold());
            }
        }
        let picture_line = lines[i].len().saturating_sub(1);
        if !pictures[i].is_empty() {
            drawn.splice(picture_line..picture_line, vec![Line::default(); 8]);
        }
        frame.render_widget(Paragraph::new(drawn), content);
        terminal_kit::emoji::draw_emoji_cells(frame, content, &emoji_urls[i], images);
        let pic_y = content.y.saturating_add(picture_line as u16);
        if !pictures[i].is_empty() && pic_y + 8 <= area.bottom() {
            let count = pictures[i].len() as u16;
            let width = (content.width.saturating_sub(count.saturating_sub(1)) / count).min(36);
            for (n, url) in pictures[i].iter().enumerate() {
                images.draw(
                    frame,
                    Rect::new(content.x + n as u16 * (width + 1), pic_y, width, 8),
                    url,
                );
            }
        }
        y += height(i);
    }
    if let Some(e) = &list.error {
        let r = Rect {
            y: area.bottom().saturating_sub(1),
            height: 1,
            ..area
        };
        frame.render_widget(Clear, r);
        frame.render_widget(
            Line::styled(format!(" {e}  {}", t("(R to retry)")), th.error()),
            r,
        );
    }
}

fn draw_thread(
    frame: &mut Frame,
    area: Rect,
    rows: &mut List<Row>,
    ctx: &Ctx,
    images: &mut Images,
) {
    let th = &ctx.t;
    let [title, body] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
    let mut head = vec![Span::styled(
        format!(" {}", t("Thread")),
        th.accent().bold(),
    )];
    if !rows.loaded {
        head.push(Span::styled(format!("  {}", t("loading…")), th.dim()));
    }
    frame.render_widget(Line::from(head), title);
    draw_statuses(
        frame,
        body,
        rows,
        n!("No posts yet. Press R to refresh."),
        ctx,
        images,
    );
}

/// What a notification says happened.
fn happened(n: &Notification) -> String {
    let what = match n.kind.as_str() {
        "mention" => n!("mentioned you"),
        "status" => n!("posted"),
        "reblog" => n!("boosted your post"),
        "follow" => n!("followed you"),
        "follow_request" => n!("asked to follow you"),
        "favourite" => n!("favourited your post"),
        "poll" => n!("a poll has ended"),
        "update" => n!("edited a post"),
        "quote" => n!("quoted your post"),
        "admin.sign_up" => n!("signed up"),
        "admin.report" => n!("filed a report"),
        other => return format!("({other})"),
    };
    t(what).to_string()
}

/// A notification's two lines.
fn notification_lines(
    n: &Notification,
    width: u16,
    ctx: &Ctx,
    pictures: bool,
    urls: &mut Vec<String>,
) -> Vec<Line<'static>> {
    let th = &ctx.t;
    let width = usize::from(width.max(1));
    let mut head = Vec::new();
    if let Some(a) = &n.account {
        head.push(Span::styled(
            emoji_text(&display_name(a), &a.emojis, pictures, urls),
            Style::new().bold(),
        ));
        head.push(Span::styled(format!(" {}", acct(a)), th.dim()));
        head.push(Span::raw(" "));
    }
    head.push(Span::raw(happened(n)));
    head.push(Span::styled(
        format!(" · {}", format_time(&n.created_at)),
        th.dim(),
    ));
    let second = match &n.status {
        Some(s) => {
            let s = s.shown();
            let line = truncate(
                &emoji_text(
                    &first_line(s, ctx.opened.contains(&s.id)),
                    &s.emojis,
                    pictures,
                    urls,
                ),
                width,
            );
            // A mention is the post itself; a favourite or a boost is about
            // your own, drawn dim.
            if matches!(n.kind.as_str(), "mention" | "status" | "quote") {
                Line::from(line)
            } else {
                Line::styled(line, th.dim())
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
    ctx: &Ctx,
    images: &mut Images,
) {
    let th = &ctx.t;
    if !list.loaded {
        frame.render_widget(
            Paragraph::new(format!(" {}", t("loading…"))).style(th.dim()),
            area,
        );
        return;
    }
    if list.items.is_empty() {
        empty_message(frame, area, list, n!("No notifications yet."), th);
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
    let content = content_rect(area, 0);
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
        draw_marker(frame, row, H, i == list.selected, th);
        let mut urls = Vec::new();
        frame.render_widget(
            Paragraph::new(notification_lines(
                item,
                content.width,
                ctx,
                images.shows(),
                &mut urls,
            )),
            Rect {
                y,
                height: 2,
                ..content
            },
        );
        terminal_kit::emoji::draw_emoji_cells(
            frame,
            Rect {
                y,
                height: 2,
                ..content
            },
            &urls,
            images,
        );
        y += H;
    }
}

/// A box of `w` x `h` centered in `area`, cleared, as bsky's.
fn popup(frame: &mut Frame, area: Rect, w: u16, h: u16, title: &str, th: &Theme) -> Rect {
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
        .title(format!(" {title} "))
        .border_style(th.accent())
        .style(th.base());
    let inner = block.inner(r);
    frame.render_widget(block, r);
    inner
}

/// A list to choose from, in a box, the selection on screen.
fn draw_choices(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    rows: &[String],
    selected: usize,
    th: &Theme,
) {
    let h = (rows.len() as u16 + 2)
        .min(area.height.saturating_sub(2))
        .max(3);
    let inner = popup(frame, area, LIST_W, h, title, th);
    let shown = usize::from(inner.height.max(1));
    let first = (selected + 1).saturating_sub(shown);
    let lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .skip(first)
        .take(shown)
        .map(|(i, r)| {
            let text = truncate(&format!(" {r}"), usize::from(inner.width));
            if i == selected {
                Line::styled(text, th.selected())
            } else {
                Line::raw(text)
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_help(frame: &mut Frame, area: Rect, scroll: &mut u16, th: &Theme) {
    let mut lines: Vec<Line> = Vec::new();
    for (title, keys) in keys::help() {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        lines.push(Line::styled(title, th.accent().bold()));
        for (k, d) in keys {
            lines.push(Line::from(vec![
                Span::styled(format!("  {k:<14}"), th.accent()),
                Span::raw(d),
            ]));
        }
    }
    let inner = popup(
        frame,
        area,
        HELP_W,
        area.height.saturating_sub(2),
        t("Keys"),
        th,
    );
    let max = (lines.len() as u16).saturating_sub(inner.height);
    *scroll = (*scroll).min(max);
    let lines: Vec<Line> = lines
        .into_iter()
        .skip(usize::from(*scroll))
        .map(|l| truncate_line(l, usize::from(inner.width)))
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_compose(
    frame: &mut Frame,
    area: Rect,
    c: &Compose,
    (count, max): (usize, usize),
    th: &Theme,
) {
    let w = COMPOSE_W.min(area.width);
    let inner_w = usize::from(w.saturating_sub(2)).max(1);
    let text = c.text.layout(inner_w);
    let warning = c
        .warning
        .as_ref()
        .map(|w| w.layout(inner_w.saturating_sub(4).max(1)));
    let mut lines: Vec<Line> = Vec::new();
    if let Some((_, to)) = &c.reply {
        lines.push(Line::styled(
            truncate(&tf("replying to {}", &[to]), inner_w),
            th.dim(),
        ));
    }
    if c.quote.is_some() {
        lines.push(Line::styled(t("Quote post"), th.repost()));
    }
    if let Some(policy) = &c.quote_policy {
        let label = match policy.as_str() {
            "public" => t("anyone"),
            "followers" => t("followers only"),
            _ => t("only me"),
        };
        lines.push(Line::styled(tf("Quote approval: {}", &[label]), th.dim()));
    }
    let mut cursor = None;
    if let Some(l) = &warning {
        let at = lines.len();
        for (i, row) in l.lines.iter().enumerate() {
            let head = if i == 0 { "CW: " } else { "    " };
            lines.push(Line::from(vec![
                Span::styled(head, theme::warn(th)),
                Span::raw(row.clone()),
            ]));
        }
        if c.focus == Field::Warning {
            cursor = Some((at + l.cursor.0, l.cursor.1 + 4));
        }
    }
    let at = lines.len();
    lines.extend(text.lines.iter().map(|l| Line::raw(l.clone())));
    if c.focus == Field::Text {
        cursor = Some((at + text.cursor.0, text.cursor.1));
    }
    for (i, file) in c.media.iter().enumerate() {
        lines.push(Line::styled(
            truncate(&file.path.display().to_string(), inner_w),
            th.accent(),
        ));
        let alt = file.alt.layout(inner_w.saturating_sub(5).max(1));
        let at = lines.len();
        for row in &alt.lines {
            lines.push(Line::raw(format!("{}: {row}", t("Alt"))));
        }
        if c.focus == Field::Alt(i) {
            cursor = Some((at + alt.cursor.0, alt.cursor.1 + cells(t("Alt")) + 2));
        }
    }
    lines.push(Line::default());
    let count_style = if count > max { th.error() } else { th.dim() };
    let mut foot = vec![
        Span::styled(format!("{count}/{max}"), count_style),
        Span::styled(
            format!(" · {}", t(visibility_label(c.visibility))),
            th.dim(),
        ),
    ];
    if c.sending.is_some() {
        foot.push(Span::styled(format!(" · {}", t("posting…")), th.ok()));
    }
    lines.push(Line::from(foot));
    let h = (lines.len() as u16 + 2).min(area.height);
    let title = if c.reply.is_some() {
        t("Reply")
    } else {
        t("New post")
    };
    let inner = popup(frame, area, w, h.max(6), title, th);
    // The cursor's line is kept on screen.
    let skip = cursor
        .map(|(row, _)| (row + 1).saturating_sub(usize::from(inner.height.saturating_sub(2))))
        .unwrap_or(0);
    let shown: Vec<Line> = lines.into_iter().skip(skip).collect();
    frame.render_widget(Paragraph::new(shown), inner);
    if let Some((row, col)) = cursor
        && c.sending.is_none()
        && c.browser.is_none()
    {
        let row = row.saturating_sub(skip) as u16;
        if row < inner.height {
            frame.set_cursor_position((
                inner.x + (col as u16).min(inner.width.saturating_sub(1)),
                inner.y + row,
            ));
        }
    }
}

fn draw_report(frame: &mut Frame, area: Rect, report: &crate::tui::app::Report, th: &Theme) {
    let inner = popup(
        frame,
        area,
        72,
        area.height.saturating_sub(4).min(14),
        t("Report"),
        th,
    );
    let target = match &report.status {
        Some(id) => format!("{} · {id}", report.shown),
        None => report.shown.clone(),
    };
    let [head, comment, foot] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .areas(inner);
    frame.render_widget(wrapped(&target, head.width), head);
    let layout = report.comment.layout(usize::from(comment.width.max(1)));
    let skip = (layout.cursor.0 + 1).saturating_sub(usize::from(comment.height));
    frame.render_widget(
        Paragraph::new(
            layout
                .lines
                .into_iter()
                .skip(skip)
                .map(Line::from)
                .collect::<Vec<_>>(),
        ),
        comment,
    );
    let hint = if report.sending.is_some() {
        t("sending report…")
    } else if report.confirming {
        t("y confirms; any other key cancels")
    } else {
        t("tab comment · enter confirm · esc close")
    };
    frame.render_widget(wrapped(hint, foot.width).style(th.accent()), foot);
    if report.writing && !report.confirming && report.sending.is_none() && comment.height > 0 {
        frame.set_cursor_position((
            comment.x + (layout.cursor.1 as u16).min(comment.width.saturating_sub(1)),
            comment.y + (layout.cursor.0 - skip) as u16,
        ));
    }
}

#[cfg(test)]
mod tests;
