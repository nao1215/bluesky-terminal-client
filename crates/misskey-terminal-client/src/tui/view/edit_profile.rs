//! The profile editor, drawn as bsky's: each field under its label, the
//! description over several lines, the picture browser over it.

use super::*;
use crate::i18n;
use crate::tui::app::EditProfile;

pub(super) fn draw_edit_profile(
    frame: &mut Frame,
    area: Rect,
    e: &mut EditProfile,
    images: &mut Images,
    t: &Theme,
) {
    let inner = popup(frame, area, 76, 20, i18n::t("Edit profile"), t);
    if e.loading {
        frame.render_widget(
            Paragraph::new(format!(" {}", i18n::t("loading…"))).style(t.dim()),
            inner,
        );
        return;
    }
    let heights = [1u16, 5, 1, 1];
    let mut y = inner.y;
    for (i, label) in EditProfile::LABELS.iter().enumerate() {
        if y >= inner.bottom() {
            break;
        }
        let style = if e.focus == i {
            t.accent().bold()
        } else {
            t.dim()
        };
        frame.render_widget(
            Paragraph::new(truncate(i18n::t(label), usize::from(inner.width))).style(style),
            Rect {
                y,
                height: 1,
                ..inner
            },
        );
        y += 1;
        let h = heights[i].min(inner.bottom().saturating_sub(y));
        let field = Rect {
            x: inner.x + 1,
            y,
            width: inner.width.saturating_sub(2),
            height: h,
        };
        if i == 1 {
            compose::draw_multi_input(frame, field, &e.fields[i], e.focus == i && !e.saving);
        } else {
            compose::draw_single_input(frame, field, &e.fields[i], e.focus == i && !e.saving);
        }
        y += h + 1;
    }
    if e.saving && y < inner.bottom() {
        frame.render_widget(
            Paragraph::new(format!(" {}", i18n::t("saving…"))).style(t.dim()),
            Rect {
                y: inner.bottom() - 1,
                height: 1,
                ..inner
            },
        );
    }
    if let Some(b) = &mut e.browser {
        b.draw(frame, area, images, t, i18n::t);
    }
}
