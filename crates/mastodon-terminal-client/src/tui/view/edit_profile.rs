//! The profile form scrolls to keep the focused field visible.
use super::*;
use crate::tui::app::EditProfile;

pub(super) fn draw(
    frame: &mut Frame,
    area: Rect,
    e: &mut EditProfile,
    images: &mut Images,
    th: &Theme,
) {
    let inner = popup(frame, area, 76, 28, t("Edit profile"), th);
    if e.loaded.is_none() {
        frame.render_widget(Paragraph::new(t("loading…")), inner);
        return;
    }
    let width = usize::from(inner.width.saturating_sub(2)).max(1);
    let mut lines = Vec::new();
    let mut cursor = (0, 0);
    for (i, label) in EditProfile::LABELS.iter().enumerate() {
        lines.push(Line::styled(
            t(label),
            if e.focus == i { th.accent() } else { th.dim() },
        ));
        if (2..5).contains(&i) {
            if e.focus == i {
                cursor = (lines.len(), 0);
            }
            lines.push(Line::raw(if e.toggles[i - 2] { "[x]" } else { "[ ]" }));
        } else {
            let layout = e.fields[i].layout(width);
            if e.focus == i {
                cursor = (lines.len() + layout.cursor.0, layout.cursor.1);
            }
            lines.extend(layout.lines.into_iter().map(Line::raw));
        }
    }
    let skip = (cursor.0 + 2).saturating_sub(usize::from(inner.height));
    frame.render_widget(
        Paragraph::new(lines.into_iter().skip(skip).collect::<Vec<_>>()),
        inner,
    );
    if e.waiting.is_none() && e.browser.is_none() {
        frame.set_cursor_position((
            inner.x + cursor.1 as u16,
            inner.y + (cursor.0 - skip) as u16,
        ));
    }
    if let Some(b) = &mut e.browser {
        b.draw(frame, area, images, th, t);
    }
}
