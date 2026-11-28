//! The settings screen, the theme list and the language list, drawn as
//! bsky's.

use super::*;
use crate::i18n::{self, Lang};
use crate::tui::app::SettingRow;
use crate::tui::input::TextInput;
use crate::tui::text::truncate_start;
use crate::tui::theme::THEMES;

const SETTINGS_W: u16 = 76;
/// How many themes the list shows at once.
const THEME_ROWS: usize = 12;

pub(super) fn draw_settings(
    frame: &mut Frame,
    area: Rect,
    rows: &[SettingRow],
    selected: usize,
    typing: Option<&TextInput>,
    t: &Theme,
) {
    const NAME_W: usize = 18;
    let inner = popup(
        frame,
        area,
        SETTINGS_W,
        rows.len() as u16 + 6,
        i18n::t("Settings"),
        t,
    );
    let width = usize::from(inner.width);
    let mut lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let marker = if i == selected { "▶ " } else { "  " };
            // Padded by the cells it takes: a name in Japanese is twice as
            // wide as its characters.
            let name = truncate(i18n::t(r.name), NAME_W);
            let pad = " ".repeat(NAME_W.saturating_sub(cells(&name)));
            let head = format!("{marker}{name}{pad} ");
            let room = width.saturating_sub(cells(&head));
            let style = if i == selected {
                t.base().bold()
            } else {
                t.dim()
            };
            Line::from(vec![
                Span::styled(head, t.accent().bold()),
                Span::styled(truncate_start(&r.value, room), style),
            ])
        })
        .collect();
    lines.push(Line::raw(""));
    if let Some(input) = typing {
        lines.push(Line::styled(
            format!(
                " {}",
                i18n::t("enter keeps it, empty is the default, esc cancels")
            ),
            t.dim(),
        ));
        frame.render_widget(Paragraph::new(lines), inner);
        let field = Rect {
            x: inner.x + 1,
            y: inner.bottom().saturating_sub(1),
            width: inner.width.saturating_sub(2),
            height: 1,
        };
        compose::draw_single_input(frame, field, input, true);
        return;
    }
    if let Some(r) = rows.get(selected) {
        for l in wrap(&r.note, width.saturating_sub(1).max(1))
            .into_iter()
            .take(2)
        {
            lines.push(Line::styled(format!(" {l}"), t.dim()));
        }
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

pub(super) fn draw_languages(frame: &mut Frame, area: Rect, selected: usize, now: Lang, t: &Theme) {
    let inner = popup(
        frame,
        area,
        34,
        crate::i18n::ALL.len() as u16 + 2,
        i18n::t("Language"),
        t,
    );
    let lines: Vec<Line> = crate::i18n::ALL
        .iter()
        .enumerate()
        .map(|(i, lang)| {
            let marker = if i == selected { "▶ " } else { "  " };
            let style = if i == selected {
                t.base().bold()
            } else {
                t.base()
            };
            let mut row = vec![
                Span::styled(marker, t.accent().bold()),
                Span::styled(lang.name(), style),
            ];
            if *lang == now {
                row.push(Span::styled(format!("  {}", i18n::t("in use")), t.accent()));
            }
            Line::from(row)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

pub(super) fn draw_themes(frame: &mut Frame, area: Rect, selected: usize, t: &Theme) {
    let n = THEMES.len();
    // A window of a few themes that scrolls with the selection, keeping it
    // in the middle where it can.
    let rows = usize::from(area.height.saturating_sub(6)).clamp(1, THEME_ROWS.min(n));
    let first = selected.saturating_sub(rows / 2).min(n - rows);
    let width = THEMES.iter().map(|t| t.name.len()).max().unwrap_or(0) + 1;
    let lines: Vec<Line> = THEMES
        .iter()
        .enumerate()
        .skip(first)
        .take(rows)
        .map(|(i, theme)| {
            let marker = if i == selected { "▶ " } else { "  " };
            let name = format!("{marker}{:<width$}", theme.name);
            let mut spans = vec![if i == selected {
                Span::styled(name, t.accent().bold())
            } else {
                Span::raw(name)
            }];
            // A swatch in the theme's own colors, whatever the current one is.
            for c in [theme.accent, theme.like, theme.repost, theme.ok, theme.dim] {
                spans.push(Span::styled("██", Style::new().fg(c).bg(theme.bg)));
            }
            Line::from(spans)
        })
        .collect();
    let keys = i18n::t("enter apply  esc cancel");
    let count = format!(" {}/{n}  ", selected + 1);
    let w = (width + 2 + 10 + 4).max(cells(&format!("{count}{keys}")) + 2) as u16;
    let inner = popup(frame, area, w.max(34), rows as u16 + 4, i18n::t("Theme"), t);
    let [body, foot] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
    frame.render_widget(Paragraph::new(lines), body);
    frame.render_widget(
        Paragraph::new(format!("{count}{keys}")).style(t.dim()),
        foot,
    );
}
