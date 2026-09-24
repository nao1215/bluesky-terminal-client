//! The screens in every language the client speaks.

use super::*;
use crate::i18n::{self, Lang};
use crossterm::event::{KeyCode, KeyEvent};

fn press(app: &mut App, c: char) {
    app.handle_key(KeyEvent::from(KeyCode::Char(c)));
}

/// Every language is drawn back in English when the test is done, even if
/// it fails, so the next test on this thread starts in English.
struct English;

impl Drop for English {
    fn drop(&mut self) {
        i18n::set(Lang::En);
    }
}

// Each description in the help fits beside its keys on one line, as the
// English ones do, in every language.
#[test]
fn every_help_line_fits_on_one_line_in_every_language() {
    let _english = English;
    let room = usize::from(HELP_W - 2) - HELP_KEYS;
    let mut long = Vec::new();
    for lang in Lang::ALL {
        i18n::set(lang);
        for (_, keys) in keys::help(true).into_iter().chain(keys::help(false)) {
            for (k, d) in keys {
                if crate::tui::text::cells(d) > room {
                    long.push(format!("{}: {k} {d}", lang.code()));
                }
            }
        }
    }
    assert!(long.is_empty(), "{}", long.join("\n"));
}

// The main screens and the lists over them draw in every language without
// cutting a wide character or an emoji in half, and say what they say in
// that language.
#[test]
fn the_screens_draw_in_every_language() {
    let _english = English;
    for lang in Lang::ALL {
        let (mut app, _) = App::new(Some(session()), "x");
        let settings = crate::config::Settings {
            language: Some(lang.code().into()),
            ..Default::default()
        };
        app.apply_settings(settings, crate::tui::theme::ColorDepth::TrueColor, None);
        assert_eq!(i18n::current(), lang);
        app.handle_event(Event::Timeline(Ok(posts(8).into())));
        // The rows as the terminal shows them, a wide character in one cell.
        let screen: Vec<String> = cells(&mut app, 80, 24)
            .into_iter()
            .map(|row| row.concat())
            .collect();
        let tab = i18n::t("Timeline");
        assert!(screen[0].contains(tab), "{}: {screen:#?}", lang.code());
        for keys in ["?", "5", "s", "\u{1b}", "+", "\u{1b}", ".", "\u{1b}", "A"] {
            match keys {
                "\u{1b}" => {
                    app.handle_key(KeyEvent::from(KeyCode::Esc));
                }
                k => press(&mut app, k.chars().next().unwrap()),
            }
            for row in cells(&mut app, 80, 24) {
                for sym in &row {
                    assert!(!is_fragment(sym), "{}: {keys}: {row:?}", lang.code());
                }
            }
        }
    }
}
