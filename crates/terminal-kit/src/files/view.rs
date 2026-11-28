use super::{Browser, EntryKind};
use crate::{
    media::images::Images,
    text::{cells, human_bytes, truncate, truncate_start},
    theme::Theme,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{Block, BorderType, Clear, Paragraph},
};

impl Browser {
    /// Draw the browser and its picture preview using the client's translation function.
    pub fn draw(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        images: &mut Images,
        theme: &Theme,
        translate: fn(&str) -> &str,
    ) {
        draw_browser(frame, area, self, images, theme, translate);
    }
}

/// The file browser: the folder on top, its folders and files on the
/// left, the selected picture previewed on the right.
fn draw_browser(
    frame: &mut Frame,
    area: Rect,
    b: &mut Browser,
    images: &mut Images,
    t: &Theme,
    translate: fn(&str) -> &str,
) {
    let title = if b.videos {
        translate("Attach pictures (up to {}) or a video")
    } else {
        translate("Attach pictures (up to {})")
    }
    .replace("{}", &b.room.to_string());
    let width = area.width.saturating_sub(4).min(110);
    let height = area.height.saturating_sub(2).min(34);
    let r = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, r);
    if r.x > area.x {
        for y in r.y..r.bottom() {
            let cell = &mut frame.buffer_mut()[(r.x - 1, y)];
            if cells(cell.symbol()) > 1 {
                cell.set_symbol(" ");
            }
        }
    }
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .style(t.base())
        .border_style(t.accent())
        .title(format!(" {title} "));
    let inner = block.inner(r);
    frame.render_widget(block, r);
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
    if !b.items.is_empty() {
        let selected = b.selected.min(b.items.len() - 1);
        let rows = usize::from(rows.height).max(1);
        b.offset = b
            .offset
            .min(selected)
            .max((selected + 1).saturating_sub(rows))
            .min(b.items.len().saturating_sub(rows));
    }
    let width = usize::from(rows.width);
    for (row, (i, e)) in b
        .items
        .iter()
        .enumerate()
        .skip(b.offset)
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
        let size_w = cells(&size);
        let name = truncate(&name, width.saturating_sub(size_w + 4));
        let pad = width.saturating_sub(2 + cells(&name) + size_w + 1);
        let line = format!("{mark}{name}{}{size} ", " ".repeat(pad));
        let style = if i == b.selected {
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
            let pic = Rect {
                height: preview.height.saturating_sub(3),
                ..preview
            };
            if super::is_picture_name(&e.name) {
                images.draw_file(frame, pic, &e.path);
            }
            frame.render_widget(
                Paragraph::new(vec![
                    Line::styled(
                        truncate(&e.name, usize::from(preview.width)),
                        Style::new().bold(),
                    ),
                    Line::styled(human_bytes(e.bytes), t.dim()),
                ]),
                Rect {
                    y: pic.bottom() + 1,
                    height: 2,
                    ..preview
                },
            );
        }
        Some(_) => frame.render_widget(
            Paragraph::new(translate("enter opens the folder")).style(t.dim()),
            preview,
        ),
        None => frame.render_widget(
            Paragraph::new(translate("no folders, pictures, or videos here")).style(t.dim()),
            preview,
        ),
    }
    let foot_line = match &b.note {
        Some(n) => Line::styled(format!(" {n}"), t.error()),
        None => Line::styled(
            format!(
                " {}",
                translate("enter choose  space mark  h up  . hidden  esc cancel")
            ),
            t.dim(),
        ),
    };
    let foot_line = Line::styled(
        truncate(&foot_line.to_string(), usize::from(foot.width)),
        if b.note.is_some() { t.error() } else { t.dim() },
    );
    frame.render_widget(foot_line, foot);
}
