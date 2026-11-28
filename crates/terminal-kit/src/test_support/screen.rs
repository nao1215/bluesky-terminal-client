//! Replay the actual ANSI diff into a cell grid to test cursor placement.
use std::io;

use ratatui::{backend::Backend, buffer::Buffer, style::Style};

/// Render a diff through the production backend, then replay cursor moves and
/// printable graphemes. Colors are ignored; graphics protocols are unsupported.
/// Unlike TestBackend this observes the cursor advances of the emitted bytes.
pub fn render_diff(previous: &Buffer, next: &Buffer) -> io::Result<Buffer> {
    let mut bytes = Vec::new();
    crate::screen::TerminalBackend::new(&mut bytes).draw(previous.diff(next).into_iter())?;
    let text = String::from_utf8(bytes).map_err(io::Error::other)?;
    let mut rest = text.as_str();
    let mut actual = previous.clone();
    let (mut x, mut y) = (0, 0);
    while !rest.is_empty() {
        if let Some(csi) = rest.strip_prefix("\x1b[") {
            let end = csi
                .find(|c: char| ('@'..='~').contains(&c))
                .ok_or_else(|| io::Error::other("unterminated CSI"))?;
            let (args, tail) = csi.split_at(end);
            if tail.starts_with('H') {
                let (row, col) = args
                    .split_once(';')
                    .ok_or_else(|| io::Error::other("invalid CUP"))?;
                y = row
                    .parse::<u16>()
                    .map_err(io::Error::other)?
                    .saturating_sub(1);
                x = col
                    .parse::<u16>()
                    .map_err(io::Error::other)?
                    .saturating_sub(1);
            } else if !tail.starts_with('m') {
                return Err(io::Error::other("unsupported CSI"));
            }
            rest = &tail[1..];
        } else {
            let end = rest.find('\x1b').unwrap_or(rest.len());
            if end == 0 {
                return Err(io::Error::other("unsupported escape"));
            }
            for (_, cluster) in crate::text::graphemes(&rest[..end]) {
                actual.set_string(x, y, cluster, Style::default());
                x += crate::text::cluster_width(cluster).max(1) as u16;
            }
            rest = &rest[end..];
        }
    }
    Ok(actual)
}
