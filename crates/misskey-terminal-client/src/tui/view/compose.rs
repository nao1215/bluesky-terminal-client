//! The composer, its attachments, the file browser, and the reaction
//! picker, drawn as bsky draws its composer and browser.

use super::*;
use crate::tui::app::Compose;
use crate::tui::input::TextInput;
use crate::tui::theme::MisskeyStyles;
use ratatui::layout::Position;

/// Size of a picture's thumbnail in the composer, in cells, as bsky's.
const THUMB: (u16, u16) = (14, 5);

pub(super) fn draw_single_input(frame: &mut Frame, area: Rect, input: &TextInput, focused: bool) {
    let width = usize::from(area.width.max(1));
    let layout = input.layout(width);
    // The visual line holding the cursor, so long input scrolls.
    let row = layout.cursor.0.min(layout.lines.len().saturating_sub(1));
    let line = layout.lines.get(row).cloned().unwrap_or_default();
    frame.render_widget(Paragraph::new(line), area);
    if focused {
        frame.set_cursor_position(Position::new(area.x + layout.cursor.1 as u16, area.y));
    }
}

pub(super) fn draw_multi_input(frame: &mut Frame, area: Rect, input: &TextInput, focused: bool) {
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

pub(super) fn draw_compose(
    frame: &mut Frame,
    area: Rect,
    c: &Compose,
    images: &mut Images,
    t: &Theme,
) {
    let (title, target) = match (&c.reply, &c.quote) {
        (Some(r), _) => (i18n::tf("Reply to {}", &[&r.who]), Some(r)),
        (None, Some(q)) => (i18n::tf("Quote {}", &[&q.who]), Some(q)),
        (None, None) => (i18n::t("New note").to_string(), None),
    };
    let n = c.media.len() as u16;
    let pics_h = if n > 0 { THUMB.1 + n } else { 0 };
    let cw_h = u16::from(c.cw.is_some());
    let inner = popup(frame, area, 72, 14 + pics_h + cw_h, &title, t);
    let quote_h = if target.is_some() { 2 } else { 0 };
    let pics_h = pics_h.min(inner.height.saturating_sub(quote_h + cw_h + 2));
    let [quote, cw_row, text, pics, foot] = Layout::vertical([
        Constraint::Length(quote_h),
        Constraint::Length(cw_h),
        Constraint::Min(1),
        Constraint::Length(pics_h),
        Constraint::Length(1),
    ])
    .areas(inner);
    if let Some(tg) = target {
        let mark = if c.reply.is_some() { "↩" } else { "❝" };
        frame.render_widget(
            Paragraph::new(truncate(
                &format!("{mark} {}: {}", tg.who, tg.excerpt),
                usize::from(quote.width),
            ))
            .style(t.dim()),
            quote,
        );
    }
    let typing = !c.sending && c.browser.is_none();
    if let Some(cw) = &c.cw {
        let label = " CW: ";
        frame.render_widget(Paragraph::new(label).style(t.warn()), cw_row);
        let field = Rect {
            x: cw_row.x + 5,
            width: cw_row.width.saturating_sub(6),
            ..cw_row
        };
        draw_single_input(frame, field, cw, typing && c.cw_focused());
    }
    let text_area = Rect {
        x: text.x + 1,
        width: text.width.saturating_sub(2),
        ..text
    };
    draw_multi_input(frame, text_area, &c.input, typing && c.focus == 0);
    if n > 0 {
        draw_attachments(frame, pics, c, images, typing, t);
    }
    let count = c.input.text().trim().chars().count();
    let action = if c.sending {
        i18n::t("posting…").to_string()
    } else {
        let who = match c.visibility {
            crate::cli::Visibility::Public => i18n::t("public"),
            crate::cli::Visibility::Home => i18n::t("home"),
            crate::cli::Visibility::Followers => i18n::t("followers"),
        };
        format!(
            "{}  ·  {who}",
            i18n::tf("{} characters", &[&count.to_string()])
        )
    };
    frame.render_widget(
        truncate_line(
            Line::from(vec![Span::styled(format!(" {action}"), t.dim())]),
            usize::from(foot.width),
        ),
        foot,
    );
}

/// The composer's files: thumbnails in a row, then each one's number,
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
    let room = area.height >= THUMB.1 + n;
    let thumbs = c.browser.is_none() && room;
    let top = if room { THUMB.1 } else { 0 };
    let rows = area.height.saturating_sub(top);
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
            if a.is_video() {
                frame.render_widget(
                    Paragraph::new("▶ video")
                        .alignment(ratatui::layout::Alignment::Center)
                        .block(Block::bordered().border_style(t.dim())),
                    r,
                );
            } else {
                images.draw_file(frame, r, &a.path);
            }
        }
        let row = Rect {
            y: area.y + top + i16,
            height: 1,
            ..area
        };
        if row.y >= area.bottom() {
            break;
        }
        let focused = typing && c.focused_media() == Some(i);
        let name = a
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let label = format!(" {} {}  {} ", i + 1, truncate(&name, 20), i18n::t("alt:"));
        let label_w = cells(&label) as u16;
        let style = if focused { t.accent().bold() } else { t.dim() };
        frame.render_widget(Paragraph::new(label).style(style), row);
        let field = Rect {
            x: row.x + label_w.min(row.width),
            width: row.width.saturating_sub(label_w + 1),
            ..row
        };
        if a.alt.is_empty() && !focused {
            frame.render_widget(
                Paragraph::new(i18n::t("(none; tab to describe it)")).style(t.dim()),
                field,
            );
        } else {
            draw_single_input(frame, field, &a.alt, focused);
        }
    }
    if n > shown {
        frame.render_widget(
            Paragraph::new(format!(
                " {}",
                i18n::tf("and {} more", &[&(n - shown).to_string()])
            ))
            .style(t.dim()),
            Rect {
                y: area.y + top + shown,
                height: 1,
                ..area
            },
        );
    }
}

/// The reactions `e` offers: what is typed, then the choices, each with
/// its picture when it is a custom emoji the server has.
#[expect(
    clippy::too_many_arguments,
    reason = "the picker reads the choices, the typing and the server's emoji, and draws pictures"
)]
pub(super) fn draw_reactions(
    frame: &mut Frame,
    area: Rect,
    choices: &[String],
    selected: usize,
    query: &str,
    emojis: &std::collections::HashMap<String, String>,
    images: &mut Images,
    t: &Theme,
) {
    let rows = choices.len().clamp(1, 12) as u16;
    let inner = popup(frame, area, 50, rows + 4, n!("React"), t);
    let [search, list, foot] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .areas(inner);
    let typed = if query.is_empty() {
        Line::styled(
            format!(" {}", i18n::t("type to search the server's emoji")),
            t.dim(),
        )
    } else {
        Line::from(vec![
            Span::styled(format!(" {} ", i18n::t("search:")), t.accent().bold()),
            Span::raw(format!("{query}▏")),
        ])
    };
    frame.render_widget(typed, search);
    if choices.is_empty() {
        frame.render_widget(
            Paragraph::new(format!(" {}", i18n::t("no emoji has that name"))).style(t.dim()),
            list,
        );
    }
    for (row, (i, c)) in choices
        .iter()
        .enumerate()
        .skip(first_shown(selected, list.height))
        .take(usize::from(list.height))
        .enumerate()
    {
        let y = list.y + row as u16;
        let marker = if i == selected { "▶ " } else { "  " };
        let style = if i == selected {
            t.base().bold()
        } else {
            t.base()
        };
        frame.render_widget(
            Line::from(vec![
                Span::styled(marker, t.accent().bold()),
                Span::raw("   "),
                Span::styled(
                    truncate(c, usize::from(list.width.saturating_sub(6))),
                    style,
                ),
            ]),
            Rect {
                y,
                height: 1,
                ..list
            },
        );
        let name = c.trim_matches(':');
        if let Some(url) = emojis.get(name) {
            images.draw(
                frame,
                Rect {
                    x: list.x + 2,
                    y,
                    width: 2,
                    height: 1,
                },
                url,
            );
        }
    }
    frame.render_widget(
        Paragraph::new(vec![
            Line::raw(""),
            Line::styled(
                format!(
                    " {}",
                    i18n::t("↑ ↓ choose  enter react  1-9 0 default ones  esc close")
                ),
                t.dim(),
            ),
        ]),
        foot,
    );
}
