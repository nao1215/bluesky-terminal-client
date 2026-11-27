//! The Search tab and the Profile tab, drawn as bsky's: the search box over
//! its results, and an account's header over its notes.

use super::*;
use crate::tui::app::{Profile, Search, SearchMode};

/// Rows an account takes in a list of accounts.
const ACCOUNT_H: u16 = 3;

/// An account's two lines: who, and the start of what it says.
fn account_lines(
    u: &User,
    width: u16,
    pictures: bool,
    urls: &mut Vec<String>,
    ctx: &Ctx,
) -> Vec<Line<'static>> {
    let t = &ctx.t;
    let width = usize::from(width.max(1));
    let mut head = vec![
        Span::styled(name_cells(u, pictures, ctx, urls), Style::new().bold()),
        Span::styled(format!(" {}", acct(u)), t.dim()),
    ];
    if u.is_following {
        head.push(Span::styled(
            format!("  {}", i18n::t("✓ following")),
            t.accent(),
        ));
    } else if u.has_pending_follow_request_from_you {
        head.push(Span::styled(
            format!("  {}", i18n::t("… requested")),
            t.accent(),
        ));
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
            i18n::t("searching…")
        } else {
            i18n::t("type a query and press enter")
        };
        frame.render_widget(Paragraph::new(format!(" {what}")).style(t.dim()), area);
        return;
    }
    if list.items.is_empty() {
        empty_message(frame, area, list, n!("No accounts found."), t);
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
        let mut urls = Vec::new();
        let lines = account_lines(u, content.width, images.shows(), &mut urls, ctx);
        let text = Rect {
            y,
            height: 2,
            ..content
        };
        frame.render_widget(Paragraph::new(lines), text);
        draw_emoji_cells(frame, text, &urls, images);
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
        SearchMode::Notes => n!("Search notes:"),
        SearchMode::Accounts => n!("Search accounts:"),
    };
    let style = if s.editing {
        t.accent().bold()
    } else {
        t.dim()
    };
    let label = format!(" {} ", i18n::t(label));
    frame.render_widget(Paragraph::new(label.as_str()).style(style), bar);
    let w = cells(&label) as u16;
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
                    Paragraph::new(format!(" {}", i18n::t("type a query and press enter")))
                        .style(t.dim()),
                    list,
                );
                return;
            }
            draw_notes(
                frame,
                list,
                &mut s.notes,
                images,
                n!("No notes found."),
                ctx,
            );
        }
        SearchMode::Accounts => draw_accounts(frame, list, &mut s.users, images, ctx),
    }
}

/// The account's header lines, as bsky's: the name, the address, the
/// counts, how you are related (or, on your own, the buttons for the keys
/// that edit it), and what it says about itself as MFM draws it.
fn header_lines(
    u: &User,
    width: u16,
    me: bool,
    pictures: bool,
    urls: &mut Vec<String>,
    ctx: &Ctx,
) -> Vec<Line<'static>> {
    let t = &ctx.t;
    let width_us = usize::from(width.max(1));
    let mut lines = vec![truncate_line(
        Line::styled(name_cells(u, pictures, ctx, urls), Style::new().bold()),
        width_us,
    )];
    let mut who = vec![Span::styled(acct(u), t.dim())];
    if u.is_locked {
        who.push(Span::styled(
            format!("  {}", i18n::t("🔒 approves followers")),
            t.dim(),
        ));
    }
    lines.push(truncate_line(Line::from(who), width_us));
    let count = |n: Option<u64>, what: &str, last: bool| {
        [
            Span::styled(n.unwrap_or(0).to_string(), Style::new().bold()),
            Span::raw(format!(
                " {}{}",
                i18n::t(what),
                if last { "" } else { "  " }
            )),
        ]
    };
    let mut counts = Vec::new();
    counts.extend(count(u.followers_count, n!("followers"), false));
    counts.extend(count(u.following_count, n!("following"), false));
    counts.extend(count(u.notes_count, n!("notes"), true));
    lines.push(truncate_line(Line::from(counts), width_us));
    if me {
        // Buttons that say the keys exist: msky does not read the mouse, so
        // they are pressed with the key they name, as bsky's.
        let button = |key: &'static str, what: &'static str| {
            [
                Span::styled("[ ", t.accent()),
                Span::styled(key, t.accent().bold()),
                Span::styled(format!(" {} ]", i18n::t(what)), t.accent()),
            ]
        };
        let mut row = vec![Span::styled(
            format!("{}  ", i18n::t("this is you")),
            t.dim(),
        )];
        row.extend(button("e", n!("Edit profile")));
        row.push(Span::raw("  "));
        row.extend(button("s", n!("Settings")));
        lines.push(truncate_line(Line::from(row), width_us));
    } else {
        let mut rel = vec![if u.is_following {
            Span::styled(i18n::t("✓ following"), t.accent().bold())
        } else if u.has_pending_follow_request_from_you {
            Span::styled(i18n::t("… requested"), t.accent())
        } else {
            Span::styled(i18n::t("not following"), t.dim())
        }];
        if u.is_followed {
            rel.push(Span::styled(
                format!("  · {}", i18n::t("follows you")),
                t.dim(),
            ));
        }
        if u.is_muted {
            rel.push(Span::styled(format!("  · {}", i18n::t("muted")), t.error()));
        }
        if u.is_blocking {
            rel.push(Span::styled(
                format!("  · {}", i18n::t("blocked")),
                t.error(),
            ));
        }
        rel.push(Span::styled(
            format!("  {}", i18n::t("(f to toggle)")),
            t.dim(),
        ));
        lines.push(truncate_line(Line::from(rel), width_us));
    }
    if let Some(d) = u.description.as_deref().filter(|d| !d.trim().is_empty()) {
        let mut picture = |e: &str| pictures.then(|| user_emoji(u, e, ctx)).flatten();
        let mut found = Vec::new();
        let desc = mfm_lines_urls(d, width_us, &mut picture, &mut found);
        // Only the pictures of the lines shown are drawn.
        let shown: Vec<Line<'static>> = desc.into_iter().take(4).collect();
        let cells_shown: usize = shown
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.matches('\u{E000}').count())
            .sum();
        urls.extend(found.into_iter().take(cells_shown));
        lines.extend(shown);
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
        let msg = match &p.error {
            Some(e) => {
                wrapped(&format!(" {e}  {}", i18n::t("(R to retry)")), area.width).style(t.error())
            }
            None => wrapped(&format!(" {}", i18n::t("loading…")), area.width).style(t.dim()),
        };
        frame.render_widget(msg, area);
        return;
    };
    // The avatar is drawn big beside the header, as bsky's.
    let big = if images.shows() { BIG_AVATAR } else { (0, 0) };
    let text_x = if images.shows() {
        MARK_W + big.0 + 2
    } else {
        MARK_W + 1
    };
    let text_w = area.width.saturating_sub(text_x);
    let me = p.own(ctx.me);
    let mut urls = Vec::new();
    let lines = header_lines(&u, text_w, me, images.shows(), &mut urls, ctx);
    let h = (lines.len() as u16).max(big.1) + 1;
    let [head, list] = Layout::vertical([Constraint::Length(h), Constraint::Min(1)]).areas(area);
    if images.shows()
        && let Some(url) = u.avatar_url.as_deref().filter(|u| !u.is_empty())
    {
        let a = Rect {
            x: head.x + MARK_W,
            y: head.y,
            width: BIG_AVATAR.0,
            height: BIG_AVATAR.1,
        };
        images.draw(frame, a.intersection(head), url);
    }
    let text = Rect {
        x: head.x + text_x.min(head.width),
        y: head.y,
        width: text_w,
        height: head.height.saturating_sub(1),
    };
    frame.render_widget(Paragraph::new(lines), text);
    draw_emoji_cells(frame, text, &urls, images);
    draw_notes(frame, list, &mut p.notes, images, n!("No notes yet."), ctx);
}
