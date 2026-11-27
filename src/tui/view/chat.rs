//! The Chat tab, drawn as bsky's: the conversations, each with whom and
//! its last message, and one conversation, its newest messages at the
//! bottom over the line a message is written on.

use super::*;
use crate::api::types::ChatMessage;
use crate::tui::app::{ChatPane, partner, partner_user};

/// The picture of custom emoji `name` in message `m`, as a note's: one
/// the sender's server sent with them, else the server read's own for one
/// of its accounts.
fn chat_emoji(m: &ChatMessage, name: &str, ctx: &Ctx) -> Option<String> {
    match &m.from_user {
        Some(u) => user_emoji(u, name, ctx),
        None => ctx.emojis.get(name).cloned(),
    }
}

/// A message's text, MFM drawn as plain text, or what it carries; with
/// `pictures`, the custom emoji it has pictures for laid out as
/// [`EMOJI_CELLS`], their addresses pushed to `urls` in order.
fn message_text(m: &ChatMessage, pictures: bool, ctx: &Ctx, urls: &mut Vec<String>) -> String {
    match (&m.text, &m.file) {
        (Some(t), _) if !t.trim().is_empty() => {
            let text = drawable(&crate::mfm::plain_text(t)).into_owned();
            if pictures {
                with_emoji_cells(&text, &mut |e| chat_emoji(m, e, ctx), urls)
            } else {
                text
            }
        }
        (_, Some(f)) => format!("▣ {}", crate::tui::text::one_line(&f.name)),
        _ => String::new(),
    }
}

/// Who sent `m`, as the conversation names them: you, or their display
/// name, as bsky's.
fn sender(
    m: &ChatMessage,
    me: Option<&str>,
    pictures: bool,
    ctx: &Ctx,
    urls: &mut Vec<String>,
) -> String {
    if Some(m.from_user_id.as_str()) == me {
        return i18n::t("you").to_string();
    }
    m.from_user.as_ref().map_or_else(
        || m.from_user_id.clone(),
        |u| name_cells(u, pictures, ctx, urls),
    )
}

/// Whom a conversation is with, as bsky's names them: the display name and
/// the address of the account, or `title` for a room.
fn chat_title(
    user: Option<&User>,
    title: &str,
    pictures: bool,
    ctx: &Ctx,
    urls: &mut Vec<String>,
) -> String {
    match user {
        Some(u) => format!("{} {}", name_cells(u, pictures, ctx, urls), acct(u)),
        None => drawable(title).into_owned(),
    }
}

pub(super) fn draw_chat(
    frame: &mut Frame,
    area: Rect,
    chat: &mut ChatPane,
    me: Option<&str>,
    images: &mut Images,
    ctx: &Ctx,
) {
    let t = &ctx.t;
    let Some(open) = &mut chat.open else {
        let list = &mut chat.convos;
        if !list.loaded {
            frame.render_widget(
                Paragraph::new(format!(" {}", i18n::t("loading…"))).style(t.dim()),
                area,
            );
            return;
        }
        if list.items.is_empty() {
            empty_message(
                frame,
                area,
                list,
                n!("No conversations yet. m on someone's profile starts one."),
                t,
            );
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
        let width = usize::from(content.width.max(1));
        let mut y = area.y;
        for (i, m) in list.items.iter().enumerate().skip(list.offset) {
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
                t,
            );
            let other = if Some(m.from_user_id.as_str()) == me {
                m.to_user.as_ref()
            } else {
                m.from_user.as_ref()
            };
            if m.to_room.is_none()
                && let Some(url) = other.and_then(|u| u.avatar_url.as_deref())
            {
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
            let (_, title) = partner(m, me);
            let user = partner_user(m, me);
            let pictures = images.shows();
            let mut urls = Vec::new();
            let unread = m.is_read == Some(false) && Some(m.from_user_id.as_str()) != me;
            let mut head = vec![Span::styled(
                chat_title(user, &title, pictures, ctx, &mut urls),
                Style::new().bold(),
            )];
            if unread {
                head.push(Span::styled(" ●", t.accent()));
            }
            head.push(Span::styled(
                format!(" · {}", format_time(&m.created_at)),
                t.dim(),
            ));
            // As bsky's: the account a conversation is with is in its
            // title, so only you and a room's members are named before
            // the message.
            let who = if user.is_some() && Some(m.from_user_id.as_str()) != me {
                None
            } else {
                Some(sender(m, me, pictures, ctx, &mut urls))
            };
            let last = message_text(m, pictures, ctx, &mut urls);
            let last = last.lines().next().unwrap_or("");
            let last = match who {
                Some(who) => format!("{who}: {last}"),
                None => last.to_string(),
            };
            let rows = Rect {
                y,
                height: 2,
                ..content
            };
            frame.render_widget(
                Paragraph::new(vec![
                    truncate_line(Line::from(head), width),
                    Line::styled(truncate(&last, width), t.dim()),
                ]),
                rows,
            );
            draw_emoji_cells(frame, rows, &urls, images);
            y += H;
        }
        return;
    };
    let [title, body, input] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .areas(area);
    let pictures = images.shows();
    let mut title_urls = Vec::new();
    let heading = chat_title(
        open.user.as_ref(),
        &open.title,
        pictures,
        ctx,
        &mut title_urls,
    );
    frame.render_widget(
        Paragraph::new(format!(
            " {}",
            truncate(&heading, usize::from(title.width.saturating_sub(2)))
        ))
        .style(t.accent().bold()),
        title,
    );
    draw_emoji_cells(frame, title, &title_urls, images);
    let width = usize::from(body.width.saturating_sub(2).max(1));
    // The messages from the bottom up, the one at `selected` (counted from
    // the newest) at the bottom: k reads further back. The pictures of
    // their custom emoji in the order they are on screen.
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut urls: Vec<String> = Vec::new();
    let mut y_left = usize::from(body.height);
    let mut all = true;
    for m in open.messages.items.iter().skip(open.messages.selected) {
        let mine = Some(m.from_user_id.as_str()) == me;
        let mut block_urls = Vec::new();
        let mut block = vec![Line::from(vec![
            Span::styled(
                sender(m, me, pictures, ctx, &mut block_urls),
                if mine {
                    t.accent().bold()
                } else {
                    Style::new().bold()
                },
            ),
            Span::styled(format!(" · {}", format_time(&m.created_at)), t.dim()),
        ])];
        block.extend(
            wrap(&message_text(m, pictures, ctx, &mut block_urls), width)
                .into_iter()
                .map(|l| Line::raw(format!(" {l}"))),
        );
        if block.len() > y_left {
            all = false;
            break;
        }
        y_left -= block.len();
        block.extend(lines);
        lines = block;
        block_urls.extend(urls);
        urls = block_urls;
    }
    // Above the oldest message, what is further back, as bsky's says.
    if all && y_left > 0 && open.messages.loaded && !open.messages.items.is_empty() {
        let above = if open.messages.loading {
            Some(i18n::t("loading earlier messages…"))
        } else if open.messages.cursor.is_none() {
            Some(i18n::t("the start of the conversation"))
        } else {
            None
        };
        if let Some(a) = above {
            lines.insert(0, Line::styled(a.to_string(), t.dim()));
        }
    }
    if !open.messages.loaded {
        lines = vec![Line::styled(format!(" {}", i18n::t("loading…")), t.dim())];
    } else if open.messages.items.is_empty() {
        lines = vec![match &open.messages.error {
            Some(e) => Line::styled(format!(" {e}  {}", i18n::t("(R to retry)")), t.error()),
            None => Line::styled(
                format!(" {}", i18n::t("No messages yet: i writes one.")),
                t.dim(),
            ),
        }];
    }
    let pad = usize::from(body.height).saturating_sub(lines.len());
    let mut shown: Vec<Line<'static>> = vec![Line::raw(""); pad];
    shown.extend(lines);
    let messages = Rect {
        x: body.x + 1,
        width: body.width.saturating_sub(1),
        ..body
    };
    frame.render_widget(Paragraph::new(shown), messages);
    if open.messages.loaded && !open.messages.items.is_empty() {
        draw_emoji_cells(frame, messages, &urls, images);
    }
    let label = if open.sending {
        format!(" {} ", i18n::t("sending…"))
    } else if open.typing {
        " › ".to_string()
    } else if open.input.text().is_empty() {
        format!(" {} ", i18n::t("i write a message"))
    } else {
        format!(" {} ", i18n::t("i continue:"))
    };
    frame.render_widget(
        Paragraph::new(label.as_str()).style(t.dim()),
        Rect {
            y: input.y + 1,
            height: 1,
            ..input
        },
    );
    let w = cells(&label) as u16;
    compose::draw_single_input(
        frame,
        Rect {
            x: input.x + w,
            y: input.y + 1,
            width: input.width.saturating_sub(w + 1),
            height: 1,
        },
        &open.input,
        open.typing && !open.sending,
    );
}
