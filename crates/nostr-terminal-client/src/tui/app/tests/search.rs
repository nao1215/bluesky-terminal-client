use super::*;
use crate::tui::extras::{Dialog, Purpose};

fn submit(app: &mut App, query: &str) -> u64 {
    ch(app, '/');
    app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    app.paste(query);
    let jobs = key(app, KeyCode::Enter);
    assert!(app.extra.dialog.is_none());
    assert!(app.searching());
    let Job::Load {
        seq,
        target: Target::Search(text),
        count: 20,
        until: None,
    } = &jobs[0]
    else {
        panic!("expected a fresh search")
    };
    assert_eq!(text, query);
    *seq
}

fn page(seq: u64, notes: Vec<Event>, append: bool, more: bool) -> Answer {
    let until = notes.iter().map(|n| n.created_at).min();
    let boundary = notes.iter().filter(|n| Some(n.created_at) == until).count();
    Answer::Loaded(
        seq,
        Ok(Outcome {
            value: Page {
                notes,
                profile: None,
                more,
                append,
                until,
                boundary,
            },
            answered: 1,
            warnings: vec![],
        }),
    )
}

fn screen(app: &App) -> String {
    let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 25)).unwrap();
    term.draw(|f| crate::tui::view::draw(f, app, &mut terminal_kit::media::images::Images::none()))
        .unwrap();
    term.backend()
        .buffer()
        .content
        .chunks(100)
        .map(|row| {
            let mut text = String::new();
            let mut x = 0;
            while x < row.len() {
                let symbol = row[x].symbol();
                text.push_str(symbol);
                x += terminal_kit::text::cells(symbol).max(1);
            }
            text
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_list(actual: &List, expected: &List) {
    assert_eq!(actual.notes, expected.notes);
    assert_eq!(actual.selected, expected.selected);
    assert_eq!(actual.loading, expected.loading);
    assert_eq!(actual.count, expected.count);
    assert_eq!(actual.more, expected.more);
    assert_eq!(actual.loaded, expected.loaded);
    assert_eq!(actual.until, expected.until);
    assert_eq!(actual.boundary, expected.boundary);
}

#[test]
fn box_cancels_without_changing_screen_and_keeps_last_submitted_query() {
    let (mut app, _) = fixture();
    let before = screen(&app);
    ch(&mut app, '/');
    assert!(key(&mut app, KeyCode::Enter).is_empty());
    assert!(app.extra.dialog.is_some());
    app.paste("unsent\nquery");
    assert!(
        matches!(&app.extra.dialog, Some(Dialog::Input { purpose: Purpose::Search, text }) if text.text() == "unsent query")
    );
    assert!(key(&mut app, KeyCode::Esc).is_empty());
    assert_eq!(screen(&app), before);
    let seq = submit(&mut app, "猫");
    app.answer(page(seq, vec![], false, false));
    key(&mut app, KeyCode::Esc);
    ch(&mut app, '/');
    assert!(
        matches!(&app.extra.dialog, Some(Dialog::Input { text, .. }) if text.text() == "猫" && text.layout(20).cursor == (0, 2))
    );
    // nstr searches posts only; Ctrl+T must not enable a nonexistent account mode.
    assert!(
        app.key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL))
            .is_empty()
    );
    assert!(
        matches!(&key(&mut app, KeyCode::Enter)[0], Job::Load { target: Target::Search(q), .. } if q == "猫")
    );
}

#[test]
fn results_page_deduplicates_and_escape_restores_every_timeline_field() {
    let (mut app, mut note) = fixture();
    note.id = "second".into();
    app.lists[0].notes.push(note.clone());
    app.lists[0].selected = 1;
    app.lists[0].count = 40;
    app.lists[0].until = Some(100);
    app.lists[0].boundary = 2;
    app.lists[0].more = true;
    let before = app.list().clone();
    let seq = submit(&mut app, "hello");
    app.answer(page(seq, vec![note.clone()], false, true));
    let jobs = ch(&mut app, 'G');
    let Job::Load {
        seq,
        target: Target::Search(ref q),
        count: 21,
        until: Some(100),
    } = jobs[0]
    else {
        panic!()
    };
    assert_eq!(q, "hello");
    assert!(ch(&mut app, 'j').is_empty());
    let mut older = note.clone();
    older.id = "older".into();
    older.created_at = 90;
    app.answer(page(seq, vec![note, older.clone()], true, false));
    assert_eq!(app.list().notes.len(), 2);
    ch(&mut app, 'G');
    assert_eq!(app.selected(), Some(&older));
    assert!(key(&mut app, KeyCode::Esc).is_empty());
    assert!(!app.searching());
    assert_list(app.list(), &before);
}

#[test]
fn failures_keep_content_and_late_answers_cannot_replace_the_restored_timeline() {
    let (mut app, note) = fixture();
    let seq = submit(&mut app, "unavailable");
    app.answer(Answer::Loaded(seq, Err(Error::network("search failed"))));
    assert_eq!(app.message, "search failed");
    assert_eq!(app.selected(), Some(&note));
    let seq = submit(&mut app, "retry");
    key(&mut app, KeyCode::Esc);
    app.answer(page(seq, vec![], false, false));
    assert_eq!(app.selected(), Some(&note));
    assert!(!app.searching());
}

#[test]
fn successful_new_query_selects_first_result_but_failure_keeps_selection() {
    let (mut app, mut second) = fixture();
    second.id = "second".into();
    app.lists[0].notes.push(second);
    app.lists[0].selected = 1;
    let previous = app.list().clone();
    let seq = submit(&mut app, "fails");
    app.answer(Answer::Loaded(seq, Err(Error::network("failed"))));
    assert_eq!(app.list().selected, 1);
    let seq = submit(&mut app, "hello");
    app.answer(page(seq, previous.notes.clone(), false, false));
    assert_eq!(app.list().selected, 0);
    key(&mut app, KeyCode::Esc);
    assert_list(app.list(), &previous);
}

#[test]
fn newer_query_wins_and_failed_replacement_keeps_previous_results() {
    let (mut app, mut note) = fixture();
    let old = submit(&mut app, "old");
    let new = submit(&mut app, "new");
    note.content = "new result".into();
    app.answer(page(new, vec![note.clone()], false, false));
    app.answer(page(old, vec![], false, false));
    assert_eq!(app.selected(), Some(&note));
    let seq = submit(&mut app, "failure");
    app.answer(Answer::Loaded(seq, Err(Error::network("failed"))));
    assert_eq!(app.selected(), Some(&note));
    assert_eq!(app.message, "failed");
}

#[test]
fn results_support_threads_profiles_actions_replies_reactions_and_viewer() {
    let (mut app, mut note) = fixture();
    note.content = "result http://127.0.0.1:9/picture.png".into();
    let seq = submit(&mut app, "result");
    app.answer(page(seq, vec![note.clone()], false, false));
    assert!(
        matches!(&ch(&mut app, 'v')[0], Job::Load { target: Target::Thread(n), .. } if n == &note)
    );
    key(&mut app, KeyCode::Esc);
    assert!(app.searching());
    ch(&mut app, '.');
    assert!(matches!(app.extra.dialog, Some(Dialog::Actions(_))));
    key(&mut app, KeyCode::Esc);
    ch(&mut app, 'r');
    assert!(matches!(&app.overlay, Some(Overlay::Compose { target: Some(n), .. }) if n == &note));
    key(&mut app, KeyCode::Esc);
    ch(&mut app, ' ');
    assert!(matches!(app.overlay, Some(Overlay::Pictures { .. })));
    key(&mut app, KeyCode::Esc);
    for c in ['l', 'b'] {
        assert!(
            matches!(&ch(&mut app, c)[0], Job::Write { event, .. } if event.tags.iter().any(|t| t.first().is_some_and(|s| s == "e") && t.get(1) == Some(&note.id)))
        );
    }
    assert!(
        matches!(&key(&mut app, KeyCode::Enter)[0], Job::Load { target: Target::Author(p), .. } if p == &note.pubkey)
    );
    assert_eq!(app.tab, Tab::Profile);
    key(&mut app, KeyCode::Esc);
    assert!(app.searching());
    assert_eq!(app.selected(), Some(&note));
    key(&mut app, KeyCode::Esc);
    assert!(!app.searching());
}

#[test]
fn searches_restore_notifications_profile_columns_and_threads() {
    for tab in [Tab::Home, Tab::Notifications, Tab::Profile] {
        for column in [false, true] {
            let (mut app, note) = fixture();
            app.tab = tab;
            app.lists[tab.index()] = app.lists[0].clone();
            app.extra.author = Some(note.pubkey.clone());
            app.profile = "saved profile".into();
            app.extra.relationship = Some((note.pubkey.clone(), true, false));
            if column && tab == Tab::Home {
                ch(&mut app, '+');
            }
            let before = app.list().clone();
            let previous_column = app.extra.column;
            let seq = submit(&mut app, "query");
            app.answer(page(seq, vec![], false, false));
            key(&mut app, KeyCode::Esc);
            assert_eq!(app.tab, tab);
            assert_eq!(app.extra.column, previous_column);
            assert_eq!(app.extra.author, Some(note.pubkey.clone()));
            assert_eq!(app.profile, "saved profile");
            assert_eq!(
                app.extra.relationship,
                Some((note.pubkey.clone(), true, false))
            );
            assert_list(app.list(), &before);
            app.threads.push((note.clone(), before.clone()));
            let seq = submit(&mut app, "thread search");
            app.answer(page(seq, vec![], false, false));
            key(&mut app, KeyCode::Esc);
            assert_eq!(app.threads.len(), 1);
            assert_list(app.list(), &before);
        }
    }
}

#[test]
fn search_columns_keep_query_and_tab_keys_select_four_remaining_tabs() {
    let (mut app, note) = fixture();
    let seq = submit(&mut app, "hello");
    app.answer(page(seq, vec![note], false, true));
    ch(&mut app, '+');
    assert_eq!(app.extra.columns, ["search:hello"]);
    assert!(matches!(app.target(), Target::Search(q) if q == "hello"));
    assert!(app.extra.persist);
    ch(&mut app, '1');
    assert_eq!(app.tab, Tab::Home);
    assert!(!app.searching());
    assert!(app.extra.column.is_none());
    ch(&mut app, '4');
    assert_eq!(app.tab, Tab::Chat);
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.tab, Tab::Home);
    key(&mut app, KeyCode::BackTab);
    assert_eq!(app.tab, Tab::Chat);
    ch(&mut app, '5');
    assert_eq!(app.tab, Tab::Chat);
}

#[test]
fn view_has_bordered_box_bilingual_hints_result_title_and_empty_message() {
    for (language, title, hint, back, empty) in [
        (
            "en",
            "Search",
            "Enter search · Esc cancel",
            "Esc back to timeline",
            r#"No results for "猫""#,
        ),
        (
            "ja",
            "検索",
            "Enter 検索 · Esc キャンセル",
            "Esc タイムラインに戻る",
            "「猫」の検索結果はありません",
        ),
    ] {
        let (mut app, notification) = fixture();
        app.lists[1].notes.push(notification);
        app.language = language;
        ch(&mut app, '/');
        let box_screen = screen(&app);
        assert!(box_screen.contains(&format!("╭ {title}")), "{box_screen}");
        assert!(box_screen.contains(hint));
        assert_eq!(box_screen.matches(hint).count(), 1, "{box_screen}");
        assert!(
            box_screen
                .lines()
                .rev()
                .take(5)
                .any(|line| line.contains(hint))
        );
        assert!(box_screen.contains('╰'));
        assert!(!box_screen.contains("Ctrl+T"));
        assert!(!box_screen.contains("5 Chat"));
        app.paste("猫");
        let jobs = key(&mut app, KeyCode::Enter);
        let Job::Load { seq, .. } = jobs[0] else {
            panic!()
        };
        app.answer(page(seq, vec![], false, false));
        let results = screen(&app);
        assert!(results.contains(&format!("1 {title}: 猫")));
        assert!(results.contains(back));
        assert!(!results.contains(app.t("Unread")), "{results}");
        assert!(results.contains(empty), "{results}");
        assert!(!results.contains(hint));
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 25)).unwrap();
        term.draw(|f| {
            crate::tui::view::draw(f, &app, &mut terminal_kit::media::images::Images::none())
        })
        .unwrap();
        let selected = term.backend().buffer()[(0, 0)].style();
        assert_eq!(selected.fg, app.theme.selected().fg);
        assert_eq!(selected.add_modifier, app.theme.selected().add_modifier);
    }
}

#[test]
fn search_box_clears_the_selected_row_at_both_edges() {
    let (mut app, _) = fixture();
    ch(&mut app, '/');
    let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 25)).unwrap();
    term.draw(|f| {
        crate::tui::view::draw(f, &app, &mut terminal_kit::media::images::Images::none())
    })
    .unwrap();
    let buf = term.backend().buffer();
    for y in 2..=4 {
        for x in [0, 11, 88, 99] {
            let cell = &buf[(x, y)];
            assert_eq!(cell.symbol(), " ");
            assert_eq!(cell.bg, app.theme.base().bg.unwrap());
            assert_eq!(cell.modifier, app.theme.base().add_modifier);
        }
    }
    let seq = submit(&mut app, "cats");
    app.answer(page(seq, vec![], false, false));
    assert_eq!(
        crate::tui::keys::search_hints(&app),
        Some(terminal_kit::view::search_result_hints(
            false,
            terminal_kit::i18n::Lang::En
        ))
    );
}
