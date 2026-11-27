//! The Search tab and the Profile tab, drawn as bsky's: the search box over
//! its results, and an account's header over its notes.

use super::*;
use crate::tui::app::{Profile, Search, SearchMode};

/// Rows an account takes in a list of accounts.
const ACCOUNT_H: u16 = 3;

/// An account's two lines: who, and the start of what it says.
fn account_lines(u: &User, width: u16, ctx: &Ctx) -> Vec<Line<'static>> {
    let t = &ctx.t;
    let width = usize::from(width.max(1));
    let mut head = vec![
        Span::styled(display_name(u), Style::new().bold()),
        Span::styled(format!(" {}", acct(u)), t.dim()),
    ];
    if u.is_following {
        head.push(Span::styled("  ✓ following", t.accent()));
    } else if u.has_pending_follow_request_from_you {
        head.push(Span::styled("  … requested", t.accent()));
    }
    let desc = u
        .description
        .as_deref()
        .map(first_text_line)
        .unwrap_or_default();
    vec![
        truncate_line(Line::from(head), width),
        Line::styled(truncate(&desc, width), t.dim()),
    ]
}

/// The first line of `text` as MFM draws it.
fn first_text_line(text: &str) -> String {
    drawable(&crate::mfm::plain_text(text))
        .lines()
        .next()
        .unwrap_or("")
        .to_string()
}

fn draw_accounts(
    frame: &mut Frame,
    area: Rect,
    list: &mut List<User>,
    images: &mut Images,
    ctx: &Ctx,
) {
    let t = &ctx.t;
    if !list.loaded {
        let what = if list.loading {
            " searching…"
        } else {
            " type a query and press enter"
        };
        frame.render_widget(Paragraph::new(what).style(t.dim()), area);
        return;
    }
    if list.items.is_empty() {
        empty_message(frame, area, list, "No accounts found.", t);
        return;
    }
    list.offset = scroll_offset(
        list.selected,
        list.offset,
        list.items.len(),
        area.height,
        |_| ACCOUNT_H,
    );
    let content = content_rect(area, images.shows());
    let mut y = area.y;
    for (i, u) in list.items.iter().enumerate().skip(list.offset) {
        if y + ACCOUNT_H > area.bottom() {
            break;
        }
        let row = Rect {
            y,
            height: ACCOUNT_H,
            ..area
        };
        draw_marker(frame, row, ACCOUNT_H, i == list.selected, t);
        if let Some(url) = u.avatar_url.as_deref() {
            let r = Rect {
                x: area.x + MARK_W,
                y,
                width: AVATAR.0,
                height: AVATAR.1,
            };
            images.draw(frame, r, url);
        }
        frame.render_widget(
            Paragraph::new(account_lines(u, content.width, ctx)),
            Rect {
                y,
                height: 2,
                ..content
            },
        );
        y += ACCOUNT_H;
    }
}

pub(super) fn draw_search(
    frame: &mut Frame,
    area: Rect,
    s: &mut Search,
    images: &mut Images,
    ctx: &Ctx,
) {
    let t = &ctx.t;
    let [bar, list] = Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(area);
    let label = match s.mode {
        SearchMode::Notes => " Search notes: ",
        SearchMode::Accounts => " Search accounts: ",
    };
    let style = if s.editing {
        t.accent().bold()
    } else {
        t.dim()
    };
    frame.render_widget(Paragraph::new(label).style(style), bar);
    let w = cells(label) as u16;
    let field = Rect {
        x: bar.x + w.min(bar.width),
        width: bar.width.saturating_sub(w + 1),
        height: 1,
        ..bar
    };
    compose::draw_single_input(frame, field, &s.input, s.editing);
    match s.mode {
        SearchMode::Notes => {
            if !s.notes.loaded && !s.notes.loading {
                frame.render_widget(
                    Paragraph::new(" type a query and press enter").style(t.dim()),
                    list,
                );
                return;
            }
            draw_notes(frame, list, &mut s.notes, images, "No notes found.", ctx);
        }
        SearchMode::Accounts => draw_accounts(frame, list, &mut s.users, images, ctx),
    }
}

/// The account's header lines: who, how you are related, its counts, and
/// what it says about itself as MFM draws it.
fn header_lines(u: &User, width: u16, me: bool, ctx: &Ctx) -> Vec<Line<'static>> {
    let t = &ctx.t;
    let width_us = usize::from(width.max(1));
    let mut head = vec![
        Span::styled(display_name(u), Style::new().bold()),
        Span::styled(format!(" {}", acct(u)), t.dim()),
    ];
    if !me {
        let rel = match (
            u.is_following,
            u.has_pending_follow_request_from_you,
            u.is_followed,
        ) {
            (true, _, true) => "  ✓ following each other",
            (true, _, false) => "  ✓ following",
            (false, true, _) => "  … requested",
            (false, false, true) => "  follows you",
            _ => "  not following",
        };
        head.push(Span::styled(rel, t.accent()));
    }
    if u.is_locked {
        head.push(Span::styled("  🔒 approves followers", t.dim()));
    }
    let mut lines = vec![truncate_line(Line::from(head), width_us)];
    lines.push(Line::styled(
        format!(
            "{} notes  {} following  {} followers",
            u.notes_count.unwrap_or(0),
            u.following_count.unwrap_or(0),
            u.followers_count.unwrap_or(0)
        ),
        t.dim(),
    ));
    if let Some(d) = u.description.as_deref().filter(|d| !d.trim().is_empty()) {
        lines.extend(mfm_lines(d, width_us).into_iter().take(3));
    }
    lines
}

pub(super) fn draw_profile(
    frame: &mut Frame,
    area: Rect,
    p: &mut Profile,
    images: &mut Images,
    ctx: &Ctx,
) {
    let t = &ctx.t;
    let Some(u) = p.user.clone() else {
        let text = match &p.error {
            Some(e) => {
                frame.render_widget(wrapped(&format!(" {e}"), area.width).style(t.error()), area);
                return;
            }
            None => " loading…",
        };
        frame.render_widget(Paragraph::new(text).style(t.dim()), area);
        return;
    };
    let content = content_rect(area, images.shows());
    let me = p.who == Some(crate::tui::worker::Who::Me);
    let lines = header_lines(&u, content.width, me, ctx);
    let h = (lines.len() as u16).max(AVATAR.1) + 1;
    let [head, list] = Layout::vertical([Constraint::Length(h), Constraint::Min(1)]).areas(area);
    if let Some(url) = u.avatar_url.as_deref() {
        images.draw(
            frame,
            Rect {
                x: area.x + MARK_W,
                y: head.y,
                width: AVATAR.0,
                height: AVATAR.1,
            },
            url,
        );
    }
    frame.render_widget(
        Paragraph::new(lines),
        Rect {
            y: head.y,
            height: head.height.saturating_sub(1),
            ..content
        },
    );
    draw_notes(frame, list, &mut p.notes, images, "No notes yet.", ctx);
}
