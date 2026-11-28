//! Home columns and their source picker.
use super::*;
use crate::tui::columns::Source;
use terminal_kit::input::TextInput;
pub(super) fn draw(frame: &mut Frame, area: Rect, app: &mut App, images: &mut Images, ctx: &Ctx) {
    let visible = terminal_kit::columns::visible(area, app.deck.columns.len(), app.deck.focus, 36);
    let first = visible.first().map_or(0, |(i, _)| *i);
    let end = first + visible.len();
    for (id, area) in visible {
        let c = &mut app.deck.columns[id];
        let [head, mut body] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
        let left = if id == first && first > 0 {
            format!("‹{first} ")
        } else {
            String::new()
        };
        let title = format!(" {left}{}", c.source.title());
        frame.render_widget(
            Paragraph::new(truncate(&title, usize::from(head.width))).style(
                if id == app.deck.focus {
                    ctx.t.selected()
                } else {
                    ctx.t.dim()
                },
            ),
            head,
        );
        if id + 1 < end {
            body.width = body.width.saturating_sub(1);
        }
        if c.source == Source::Notifications {
            draw_notifications(frame, body, &mut c.notifications, ctx, images);
        } else {
            draw_statuses(
                frame,
                body,
                &mut c.posts,
                n!("No posts yet. Press R to refresh."),
                ctx,
                images,
            );
        }
    }
}
pub(super) fn picker(
    frame: &mut Frame,
    area: Rect,
    selected: usize,
    query: &Option<TextInput>,
    choices: &[Source],
    th: &Theme,
) {
    if let Some(input) = query {
        let inner = popup(frame, area, 70, 6, t("Hashtag or search query"), th);
        let layout = input.layout(usize::from(inner.width.max(1)));
        frame.render_widget(Paragraph::new(layout.lines.join("\n")), inner);
        frame.set_cursor_position((
            inner.x + layout.cursor.1 as u16,
            inner.y + layout.cursor.0 as u16,
        ));
    } else {
        let labels: Vec<_> = choices
            .iter()
            .map(|s| match s {
                Source::Tag(q) if q.is_empty() => t("Hashtag…").into(),
                Source::Search(q) if q.is_empty() => t("Search…").into(),
                _ => s.title(),
            })
            .collect();
        draw_choices(frame, area, t("Add column"), &labels, selected, th);
    }
}
