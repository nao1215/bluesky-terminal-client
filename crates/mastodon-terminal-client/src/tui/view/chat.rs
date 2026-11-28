//! Conversation list and the direct-message input.
use super::*;
pub(super) fn draw(frame: &mut Frame, area: Rect, app: &mut App, images: &mut Images, ctx: &Ctx) {
    if app.session.is_none() {
        draw_needs_login(frame, area, app.host(), &ctx.t);
        return;
    }
    if let Some(open) = &mut app.chat.open {
        let [head, body, input] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .areas(area);
        let title = open.accounts.iter().map(acct).collect::<Vec<_>>().join(" ");
        frame.render_widget(
            Paragraph::new(truncate(&title, usize::from(head.width))).style(ctx.t.accent()),
            head,
        );
        draw_statuses(
            frame,
            body,
            &mut open.posts,
            n!("No messages yet."),
            ctx,
            images,
        );
        let block = Block::bordered().title(t(if open.sending.is_some() {
            n!("sending…")
        } else if open.typing {
            n!("Message (enter sends)")
        } else {
            n!("i to write a message")
        }));
        let inner = block.inner(input);
        frame.render_widget(block, input);
        let layout = open.input.layout(usize::from(inner.width.max(1)));
        frame.render_widget(
            Paragraph::new(
                layout
                    .lines
                    .get(layout.cursor.0)
                    .cloned()
                    .unwrap_or_default(),
            ),
            inner,
        );
        if open.typing && open.sending.is_none() {
            frame.set_cursor_position((
                inner.x + (layout.cursor.1 as u16).min(inner.width.saturating_sub(1)),
                inner.y,
            ));
        }
    } else {
        let list = &mut app.chat.conversations;
        if list.items.is_empty() {
            empty_message(frame, area, list, n!("No conversations yet."), &ctx.t);
            return;
        }
        list.offset = scroll_offset(
            list.selected,
            list.offset,
            list.items.len(),
            area.height,
            |_| 3,
        );
        for (i, c) in list
            .items
            .iter()
            .enumerate()
            .skip(list.offset)
            .take(usize::from(area.height / 3))
        {
            let row = Rect::new(area.x, area.y + (i - list.offset) as u16 * 3, area.width, 3);
            draw_marker(frame, row, 3, i == list.selected, &ctx.t);
            let mut urls = Vec::new();
            let names = c
                .accounts
                .iter()
                .map(|a| {
                    emoji_text(
                        &format!("{} {}", display_name(a), acct(a)),
                        &a.emojis,
                        images.shows(),
                        &mut urls,
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let text = c
                .last_status
                .as_ref()
                .map(|s| emoji_text(&first_line(s, false), &s.emojis, images.shows(), &mut urls))
                .unwrap_or_default();
            let content = content_rect(row, 0);
            let lines = vec![
                Line::raw(truncate(
                    &format!("{} {names}", if c.unread { "●" } else { " " }),
                    usize::from(content.width),
                )),
                Line::raw(truncate(&text, usize::from(content.width))),
            ];
            frame.render_widget(Paragraph::new(lines), content);
            terminal_kit::emoji::draw_emoji_cells(frame, content, &urls, images);
        }
    }
}
