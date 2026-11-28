//! Browser OAuth progress, with the URL visible when the opener is unavailable.
use super::*;
pub(super) fn login(
    frame: &mut Frame,
    area: Rect,
    input: &terminal_kit::input::TextInput,
    url: Option<&str>,
    waiting: bool,
    theme: &Theme,
) {
    let inner = popup(frame, area, 88, 14, t("Log in"), theme);
    let text = if waiting {
        format!(
            "{}\n\n{}\n\n{}",
            t("Allow mstdn in your browser."),
            url.unwrap_or(t("Preparing login…")),
            t("o opens the browser; esc cancels")
        )
    } else {
        format!("{}\n\n{}", t("Server address"), input.text())
    };
    frame.render_widget(wrapped(&text, inner.width), inner);
    if !waiting {
        frame.set_cursor_position((
            inner.x + (cells(&input.text()) as u16).min(inner.width.saturating_sub(1)),
            inner.y + 2,
        ));
    }
}
