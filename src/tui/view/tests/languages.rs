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

// A box over wide characters keeps its left border: a wide character just
// left of it would otherwise cover the border's cell with its second half.
#[test]
fn a_box_over_wide_characters_keeps_its_left_border() {
    let _english = English;
    let (mut app, _) = App::new(Some(session()), "x");
    let settings = crate::config::Settings {
        language: Some("ja".into()),
        ..Default::default()
    };
    app.apply_settings(settings, crate::tui::theme::ColorDepth::TrueColor, None);
    app.handle_event(Event::Timeline(Ok(posts(8).into())));
    press(&mut app, '?');
    let buf = render_buffer(&mut app, 80, 24);
    let left = (0..80u16)
        .find(|x| buf[(*x, 1)].symbol() == "│")
        .expect("the help's left border");
    for y in 1..23u16 {
        let before = buf[(left - 1, y)].symbol();
        assert!(
            before.width() < 2,
            "row {y}: a wide {before:?} covers the border"
        );
    }
}

// In German, whose words are long, the composer still says how to cancel,
// and the login form shows its whole first line.
#[test]
fn long_words_do_not_push_out_how_to_leave() {
    let _english = English;
    i18n::set(Lang::De);
    let (mut app, _) = App::new(Some(session()), "x");
    let settings = crate::config::Settings {
        language: Some("de".into()),
        ..Default::default()
    };
    app.apply_settings(settings, crate::tui::theme::ColorDepth::TrueColor, None);
    app.handle_event(Event::Timeline(Ok(posts(3).into())));
    press(&mut app, 'n');
    let screen: String = cells(&mut app, 80, 24)
        .into_iter()
        .map(|row| row.concat())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(screen.contains("esc abbrechen"), "{screen}");
    let (mut login, _) = App::new(None, "x");
    login.apply_settings(
        crate::config::Settings {
            language: Some("de".into()),
            ..Default::default()
        },
        crate::tui::theme::ColorDepth::TrueColor,
        None,
    );
    let screen: String = cells(&mut login, 80, 30)
        .into_iter()
        .map(|row| row.concat())
        .collect::<Vec<_>>()
        .join(" ");
    let squeezed: String = screen.split_whitespace().collect::<Vec<_>>().join(" ");
    let want = i18n::t("Your Bluesky password works; an app password is safer.");
    for word in want.split_whitespace() {
        assert!(squeezed.contains(word), "{word}: {squeezed}");
    }
}

// The theme picker says how to apply and how to cancel in every language:
// the box is wide enough for its keys, and a screen too narrow for them
// drops the middle ones rather than cutting a word. In German the box was
// one cell short and ended on "esc abbreche".
#[test]
fn the_theme_picker_keeps_its_keys_whole_in_every_language() {
    let _english = English;
    for lang in Lang::ALL {
        i18n::set(lang);
        let (mut app, _) = App::new(Some(session()), "x");
        app.handle_event(Event::Timeline(Ok(posts(3).into())));
        press(&mut app, 'T');
        for _ in 0..12 {
            app.handle_key(KeyEvent::from(KeyCode::Down));
        }
        let keys = i18n::t("enter apply  esc cancel");
        // The box's own row of keys, not the hint row under the screen.
        let foot = |screen: &str| {
            let row = screen.lines().find(|l| l.contains("13/42")).unwrap_or("");
            row.to_string()
        };
        let screen = render_text_only(&mut app, 80, 24);
        assert!(foot(&screen).contains(keys), "{}:\n{screen}", lang.code());
        let leave = keys.rsplit("  ").next().unwrap();
        let narrow = render_text_only(&mut app, 30, 24);
        assert!(foot(&narrow).contains(leave), "{}:\n{narrow}", lang.code());
    }
}
