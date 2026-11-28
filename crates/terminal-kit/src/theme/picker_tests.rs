use super::*;
#[test]
fn theme_names_are_unique_and_found_ignoring_case() {
    for (i, t) in THEMES.iter().enumerate() {
        assert_eq!(index_of(t.name), Some(i));
        assert_eq!(index_of(&t.name.to_uppercase()), Some(i));
    }
    assert_eq!(index_of("no-such-theme"), None);
}

// The same 42 as bsky's and msky's, in their order.
#[test]
fn the_picker_lists_bluesky_then_terminal_then_the_editors_then_monochrome() {
    assert_eq!(THEMES.len(), 42);
    assert_eq!(THEMES[3], TERMINAL);
    assert_eq!(THEMES[4..41], EDITOR_THEMES);
    assert_eq!(THEMES[41], MONOCHROME);
}

#[test]
fn a_256_color_terminal_gets_no_24_bit_colors() {
    for t in &THEMES {
        let t = t.for_depth(ColorDepth::Ansi256);
        for c in [t.bg, t.fg, t.dim, t.accent, t.error, t.ok, t.like, t.repost] {
            assert!(!matches!(c, Color::Rgb(..)), "{}: {c:?}", t.name);
        }
    }
}

#[test]
fn no_color_means_monochrome_whatever_was_chosen() {
    assert_eq!(chosen(Some("nord"), ColorDepth::None).1, MONOCHROME);
}

#[test]
fn the_default_is_terminal_and_an_unknown_name_falls_back_to_it() {
    let (i, t) = chosen(None, ColorDepth::TrueColor);
    assert_eq!(THEMES[i].name, "terminal");
    assert_eq!(t, TERMINAL);
    assert_eq!(chosen(Some("no-such-theme"), ColorDepth::TrueColor).0, i);
    let (i, _) = chosen(Some("Dracula"), ColorDepth::TrueColor);
    assert_eq!(THEMES[i].name, "dracula");
}
