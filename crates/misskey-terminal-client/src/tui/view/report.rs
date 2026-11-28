//! The report form: whom it reports, the reasons, and a comment.

use super::*;
use crate::cli::more::ReportReason;
use crate::i18n;
use crate::tui::app::Report;

pub(super) fn draw_report(frame: &mut Frame, area: Rect, r: &Report, t: &Theme) {
    let inner = popup(
        frame,
        area,
        76,
        ReportReason::ALL.len() as u16 + 12,
        i18n::t("Report"),
        t,
    );
    let width = usize::from(inner.width);
    let mut lines = vec![
        Line::styled(truncate(&drawable(&r.shown), width), Style::new().bold()),
        Line::raw(""),
    ];
    for (i, reason) in ReportReason::ALL.iter().enumerate() {
        let marker = if i == r.reason { "▶ " } else { "  " };
        let style = if i == r.reason && !r.writing {
            t.base().bold()
        } else {
            t.dim()
        };
        lines.push(Line::from(vec![
            Span::styled(marker, t.accent().bold()),
            Span::styled(i18n::t(reason.label()), style),
        ]));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        i18n::t("Comment (optional)"),
        if r.writing {
            t.accent().bold()
        } else {
            t.dim()
        },
    ));
    let used = lines.len() as u16;
    frame.render_widget(Paragraph::new(lines), inner);
    let field = Rect {
        x: inner.x + 1,
        y: inner.y + used,
        width: inner.width.saturating_sub(2),
        height: inner.height.saturating_sub(used + 1).min(4),
    };
    compose::draw_multi_input(frame, field, &r.comment, r.writing && !r.sending);
    let foot = if r.sending {
        i18n::t("sending…").to_string()
    } else if r.confirming {
        i18n::t("y sends the report; any other key goes back").to_string()
    } else {
        String::new()
    };
    frame.render_widget(
        Paragraph::new(foot).style(t.accent()),
        Rect {
            y: inner.bottom().saturating_sub(1),
            height: 1,
            ..inner
        },
    );
}
