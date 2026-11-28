//! Stateless list geometry and terminal drawing shared by clients.
use crate::{
    input::TextInput,
    text::{cells, cluster_width, drawable, graphemes, truncate, wrap},
    theme::Theme,
};
use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
};
use unicode_width::UnicodeWidthStr;

/// Keep the selected variable-height item visible and fill unused space at the end.
/// Callers pass a valid selection for nonempty lists. Empty lists return zero;
/// a zero viewport or an oversized item retains the selected start.
pub fn scroll_offset(
    selected: usize,
    offset: usize,
    len: usize,
    viewport: u16,
    mut height: impl FnMut(usize) -> u16,
) -> usize {
    if len == 0 {
        return 0;
    }
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
    // When the list ends on screen with room to spare (the terminal grew, or
    // the selection is near the end), the room goes to the posts above
    // rather than staying empty. Only the rows that fit are measured.
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

/// First fixed-height row that keeps the selection visible.
pub fn first_visible(selected: usize, rows: u16) -> usize {
    (selected + 1).saturating_sub(usize::from(rows.max(1)))
}

/// Draw the visual line containing the input cursor, optionally focusing it.
pub fn draw_single_input(frame: &mut Frame, area: Rect, input: &TextInput, focused: bool) {
    if area.is_empty() {
        return;
    }
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

/// Draw input lines, scrolling vertically to keep the cursor visible.
pub fn draw_multi_input(frame: &mut Frame, area: Rect, input: &TextInput, focused: bool) {
    if area.is_empty() {
        return;
    }
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

/// Pack translated key hints into rows without splitting individual hints.
pub fn hint_lines(hints: &[(&str, &str)], width: u16, t: &Theme) -> Vec<Line<'static>> {
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
        row.push(Span::styled((*key).to_owned(), t.accent().bold()));
        row.push(Span::styled(format!(" {what}"), t.dim()));
        used += w;
    }
    lines.push(row);
    lines
        .into_iter()
        .map(|spans| truncate_line(Line::from(spans), width))
        .collect()
}

/// Clamp a rectangle to its parent and center it within that parent.
pub fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}

/// Clear a panel, optionally clearing a wide character crossing its left edge.
/// `left_boundary` is the parent area's left edge; `None` preserves neighboring cells.
/// Titles, borders and colors come entirely from the supplied block.
pub fn draw_panel(
    frame: &mut Frame,
    area: Rect,
    block: Block<'_>,
    left_boundary: Option<u16>,
) -> Rect {
    frame.render_widget(Clear, area);
    if left_boundary.is_some_and(|x| area.x > x) {
        for y in area.y..area.bottom() {
            let cell = &mut frame.buffer_mut()[(area.x - 1, y)];
            if cell.symbol().width() > 1 {
                cell.set_symbol(" ");
            }
        }
    }
    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
}

/// Draw a single-line dialog near the top of its parent, with a wrapping footer.
/// The supplied block determines the title, border and input background.
pub fn draw_input_box(
    frame: &mut Frame,
    area: Rect,
    input: &TextInput,
    block: Block<'_>,
    footer: &str,
    footer_style: Style,
) {
    let width = area.width.saturating_sub(4).min(76);
    let lines: Vec<Line> = wrap(footer, usize::from(width.saturating_sub(4)).max(1))
        .into_iter()
        .map(|line| Line::styled(line, footer_style))
        .collect();
    let height = (lines.len() as u16 + 3).min(area.height);
    let rect = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + u16::from(area.height > height),
        width,
        height,
    );
    let inner = draw_panel(frame, rect, block, Some(area.x));
    if inner.is_empty() {
        return;
    }
    draw_single_input(frame, Rect { height: 1, ..inner }, input, true);
    frame.render_widget(
        Paragraph::new(lines),
        Rect {
            y: inner.y + 1,
            height: inner.height - 1,
            ..inner
        },
    );
}

/// Width of the key column in the two-column help layout.
pub const HELP_KEYS: usize = 18;

/// Lay out translated help sections, putting descriptions below keys on narrow screens.
pub fn help_lines(
    sections: Vec<(&str, Vec<(&str, &str)>)>,
    width: u16,
    t: &Theme,
) -> Vec<Line<'static>> {
    let inner_w = usize::from(width);
    let beside = inner_w >= HELP_KEYS + 20;
    let mut lines: Vec<Line> = Vec::new();
    for (i, (title, keys)) in sections.into_iter().enumerate() {
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
    lines
}

/// Truncate a styled line at a grapheme boundary, retaining the cut span's style.
pub fn truncate_line(line: Line<'static>, width: usize) -> Line<'static> {
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

/// Join styled tab labels with spaces and place an optional account at the right.
/// `minimum_gap` preserves whether an exact fit (zero) or a separating cell (one) is required.
pub fn tab_line(
    labels: Vec<Span<'static>>,
    right: Option<Span<'static>>,
    width: u16,
    minimum_gap: usize,
) -> Line<'static> {
    let mut spans: Vec<_> = labels
        .into_iter()
        .flat_map(|label| [label, Span::raw(" ")])
        .collect();
    let used: usize = spans.iter().map(|s| cells(&s.content)).sum();
    if let Some(right) = right {
        let room = usize::from(width).saturating_sub(used);
        let name_w = cells(&right.content);
        if name_w.saturating_add(minimum_gap) <= room {
            spans.push(Span::raw(" ".repeat(room - name_w)));
            spans.push(right);
        }
    }
    Line::from(spans)
}

/// Draw an optional left message, leaving room for the activity text on the right.
/// The left text is truncated and prefixed with one space, as in the client status rows.
pub fn draw_status_line(frame: &mut Frame, area: Rect, left: Option<Span<'_>>, right: Span<'_>) {
    let w = cells(&right.content) as u16;
    if let Some(left) = left {
        let room = usize::from(area.width.saturating_sub(w + 2));
        frame.render_widget(
            Line::styled(format!(" {}", truncate(&left.content, room)), left.style),
            area,
        );
    }
    if w < area.width {
        frame.render_widget(
            Paragraph::new(right),
            Rect {
                x: area.right() - w,
                width: w,
                ..area
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend, widgets::BorderType};

    #[test]
    fn an_empty_list_never_measures_an_item() {
        assert_eq!(scroll_offset(0, 0, 0, 10, |_| unreachable!()), 0);
        assert_eq!(scroll_offset(1, 0, 3, 0, |_| 2), 1);
        assert_eq!(scroll_offset(1, 0, 3, 2, |_| 20), 1);
    }

    #[test]
    fn input_boxes_fit_small_terminals_and_keep_unicode_cursors_in_bounds() {
        let mut input = TextInput::default();
        input.insert_str("猫🐈‍⬛".repeat(30).as_str());
        for width in 1..90 {
            for height in 1..9 {
                let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
                term.draw(|frame| {
                    draw_input_box(
                        frame,
                        frame.area(),
                        &input,
                        Block::bordered()
                            .border_type(BorderType::Rounded)
                            .title("検索"),
                        "Enter 検索 · Esc キャンセル",
                        Style::default(),
                    )
                })
                .unwrap();
                let cursor = term.get_cursor_position().unwrap();
                assert!(
                    cursor.x < width && cursor.y < height,
                    "{width}x{height}: {cursor:?}"
                );
            }
        }
    }

    #[test]
    fn clearing_a_panel_preserves_ordinary_neighbors_and_removes_crossing_wide_cells() {
        let mut term = Terminal::new(TestBackend::new(12, 4)).unwrap();
        term.draw(|frame| {
            frame.render_widget(Paragraph::new("猫\nx"), Rect::new(0, 0, 12, 4));
            draw_panel(frame, Rect::new(1, 0, 8, 4), Block::bordered(), Some(0));
        })
        .unwrap();
        let buffer = term.backend().buffer();
        assert_eq!(buffer[(0, 0)].symbol(), " ");
        assert_eq!(buffer[(0, 1)].symbol(), "x");
        assert_eq!(buffer[(1, 0)].symbol(), "┌");
    }

    #[test]
    fn right_tab_labels_preserve_each_clients_exact_fit_policy() {
        let labels = || vec![Span::raw("tab")];
        assert_eq!(
            tab_line(labels(), Some(Span::raw("name")), 8, 0).to_string(),
            "tab name"
        );
        assert_eq!(
            tab_line(labels(), Some(Span::raw("name")), 8, 1).to_string(),
            "tab "
        );
    }
}
