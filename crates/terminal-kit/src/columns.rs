//! Column placement with the focused column kept on screen.
use ratatui::layout::{Constraint, Layout, Rect};

/// Visible column indices and rectangles, equally wide and containing `focus`.
/// `minimum` is the preferred minimum width; a narrow screen shows one column.
pub fn visible(area: Rect, count: usize, focus: usize, minimum: u16) -> Vec<(usize, Rect)> {
    if count == 0 {
        return Vec::new();
    }
    let fit = usize::from((area.width / minimum.max(1)).max(1)).min(count);
    let first = focus
        .min(count - 1)
        .saturating_sub(fit - 1)
        .min(count - fit);
    let areas = Layout::horizontal(vec![Constraint::Ratio(1, fit as u32); fit]).split(area);
    areas
        .iter()
        .enumerate()
        .map(|(slot, area)| (first + slot, *area))
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn focus_is_visible_at_every_width() {
        for width in 0..150 {
            for focus in 0..9 {
                let area = Rect::new(0, 0, width, 20);
                let shown = visible(area, 9, focus, 36);
                assert!(shown.iter().any(|(i, _)| *i == focus));
                assert_eq!(shown.iter().map(|(_, r)| r.width).sum::<u16>(), width);
                assert!(shown.iter().all(|(_, r)| r.right() <= area.right()));
            }
        }
        assert!(visible(Rect::default(), 0, 99, 0).is_empty());
    }
}
