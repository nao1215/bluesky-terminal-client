use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn press(app: &mut App, key: KeyCode) {
    app.handle_key(KeyEvent::from(key));
}

#[rstest::rstest]
#[case(
    crate::i18n::Lang::En,
    "Search",
    "Search: cats",
    "No results for \"cats\"",
    "Esc back to timeline"
)]
#[case(
    crate::i18n::Lang::Ja,
    "検索",
    "検索: cats",
    "「cats」の検索結果はありません",
    "Esc タイムラインに戻る"
)]
fn box_title_results_title_and_empty_message_are_translated(
    #[case] lang: crate::i18n::Lang,
    #[case] title: &str,
    #[case] results: &str,
    #[case] empty: &str,
    #[case] back: &str,
) {
    struct ResetLanguage;
    impl Drop for ResetLanguage {
        fn drop(&mut self) {
            crate::i18n::set(crate::i18n::Lang::En);
        }
    }
    let _reset = ResetLanguage;
    crate::i18n::set(lang);
    let (mut app, _) = App::new(Some(session()), "x");
    app.handle_event(Event::Timeline(Ok(posts(8).into())));
    app.timeline.selected = 6;
    render_text_only(&mut app, 100, 24);
    let before = format!("{:?}", app.timeline);
    press(&mut app, KeyCode::Char('/'));
    let screen = render_text_only(&mut app, 100, 24);
    assert!(screen.contains(&format!("╭ {title} ·")), "{screen}");
    assert_eq!(format!("{:?}", app.timeline), before);
    if lang == crate::i18n::Lang::En {
        assert!(
            screen.contains("Enter search · Ctrl+T posts/accounts · Esc cancel"),
            "{screen}"
        );
    }
    app.handle_paste("cats");
    press(&mut app, KeyCode::Enter);
    app.handle_event(Event::SearchPosts {
        query: "cats".into(),
        result: Ok(Vec::new().into()),
    });
    let screen = render_text_only(&mut app, 100, 24);
    assert!(
        screen
            .lines()
            .next()
            .unwrap()
            .contains(&format!("1 {results}")),
        "{screen}"
    );
    assert!(screen.contains(empty), "{screen}");
    assert!(screen.contains(back), "{screen}");
    assert!(!screen.contains("post number"), "{screen}");
    press(&mut app, KeyCode::Esc);
    render_text_only(&mut app, 100, 24);
    assert_eq!(format!("{:?}", app.timeline), before);
}

#[test]
fn accounts_mode_is_in_the_box_title_and_cancel_preserves_post_results() {
    let (mut app, _) = App::new(Some(session()), "x");
    press(&mut app, KeyCode::Char('/'));
    app.handle_paste("cats");
    press(&mut app, KeyCode::Enter);
    app.handle_event(Event::SearchPosts {
        query: "cats".into(),
        result: Ok(posts(3).into()),
    });
    press(&mut app, KeyCode::Char('/'));
    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
    let screen = render_text_only(&mut app, 100, 24);
    assert!(screen.contains("╭ Search · Accounts"), "{screen}");
    press(&mut app, KeyCode::Esc);
    let screen = render_text_only(&mut app, 100, 24);
    assert!(screen.contains("post number 0"), "{screen}");
    assert!(!screen.contains("╭ Search"), "{screen}");
    press(&mut app, KeyCode::Char('/'));
    press(&mut app, KeyCode::Enter);
    app.handle_event(Event::SearchActors {
        query: "cats".into(),
        result: Ok(Vec::new().into()),
    });
    let screen = render_text_only(&mut app, 100, 24);
    assert!(screen.contains("No results for \"cats\""), "{screen}");
    assert!(!screen.contains("post number"), "{screen}");
}

#[test]
fn search_failure_stays_in_status_without_covering_the_previous_list() {
    let (mut app, _) = App::new(Some(session()), "x");
    app.handle_event(Event::Timeline(Ok(posts(3).into())));
    press(&mut app, KeyCode::Char('/'));
    app.handle_paste("cats");
    press(&mut app, KeyCode::Enter);
    app.handle_event(Event::SearchPosts {
        query: "cats".into(),
        result: Err(crate::error::Error::api("search unavailable")),
    });
    let screen = render_text_only(&mut app, 100, 24);
    assert!(screen.contains("post number 0"), "{screen}");
    assert!(
        screen
            .lines()
            .last()
            .unwrap()
            .contains("search unavailable"),
        "{screen}"
    );
    assert!(!screen.contains("any key closes this"), "{screen}");
    assert!(!screen.contains("Search: cats"), "{screen}");
}
