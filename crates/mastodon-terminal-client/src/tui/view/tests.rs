use super::*;
use crate::api::types::Account;
use crate::api::{Next, Page};
use crate::cli::TimelineKind;
use crate::config::Session;
use crate::tui::worker::Event;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn status(id: &str, text: &str) -> Status {
    Status {
        id: id.into(),
        created_at: "2026-11-26T10:00:00.000Z".into(),
        account: Account {
            id: "a".into(),
            username: "alice".into(),
            acct: "alice".into(),
            display_name: "アリス 👩🏽‍💻".into(),
            ..Account::default()
        },
        content: format!("<p>{text}</p>"),
        visibility: "public".into(),
        favourites_count: 5,
        reblogs_count: 1,
        ..Status::default()
    }
}

fn app_with(items: Vec<Status>) -> App {
    let session = Session {
        service: "http://127.0.0.1:9".into(),
        user_id: "7".into(),
        username: "me".into(),
        token: "fake-token-for-tests".into(),
        scopes: Vec::new(),
    };
    let (mut app, jobs) = App::new(Some(session), "http://127.0.0.1:9");
    app.handle_answer(
        jobs[0].0,
        Event::Timeline {
            kind: TimelineKind::Home,
            max_id: None,
            result: Ok(Page {
                items,
                next: Next::End,
            }),
        },
    );
    app
}

/// The screen `app` draws at `w` x `h`, a line a row, the second cell of
/// a wide character left out.
fn screen(app: &mut App, w: u16, h: u16) -> String {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| draw(f, app)).unwrap();
    let buf = term.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..h {
        let mut skip = 0;
        for x in 0..w {
            if skip > 0 {
                skip -= 1;
                continue;
            }
            let s = buf[(x, y)].symbol();
            skip = cells(s).saturating_sub(1);
            out.push_str(s);
        }
        out.push('\n');
    }
    out
}

fn press(app: &mut App, c: char) {
    app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
}

#[test]
fn the_home_timeline_draws_its_tabs_status_and_keys() {
    let mut boost = status("b", "");
    boost.account.display_name = "Carol".into();
    boost.account.acct = "carol".into();
    boost.reblog = Some(Box::new(status("1", "おはよう 👨‍👩‍👧 🇯🇵")));
    let mut app = app_with(vec![boost]);
    let s = screen(&mut app, 100, 20);
    for want in [
        " 1 Home   2 Local   3 Federated   4 Notifications ",
        "@me@127.0.0.1:9",
        "⟳ boosted by Carol @carol",
        "アリス 👩🏽‍💻 @alice · 2026-11-26",
        "おはよう 👨‍👩‍👧 🇯🇵",
        "☆ 5   ⟳ 1   ↩ 0",
        "? help  / search  j k move  tab next tab",
    ] {
        assert!(s.contains(want), "{want:?} not in\n{s}");
    }
}

// A warning hides the text until w, and says how much it hides.
#[test]
fn a_warning_hides_the_text_until_w() {
    let mut s = status("1", "犯人は執事");
    s.spoiler_text = "ネタバレ".into();
    let mut app = app_with(vec![s]);
    let shown = screen(&mut app, 80, 16);
    assert!(shown.contains("⚠ CW: ネタバレ"), "{shown}");
    assert!(shown.contains("w shows it (5 characters)"), "{shown}");
    assert!(!shown.contains("犯人は執事"), "{shown}");
    press(&mut app, 'w');
    assert!(screen(&mut app, 80, 16).contains("犯人は執事"));
}

// What a server wrote reaches the screen as text: an escape sequence in a
// post or a name is drawn, not obeyed.
#[test]
fn a_servers_escape_sequences_are_drawn_as_text() {
    let mut s = status("1", "a\u{1b}]52;c;cHduZWQ=\u{7}b");
    s.account.display_name = "\u{1b}[2Jevil".into();
    let mut app = app_with(vec![s]);
    let shown = screen(&mut app, 80, 16);
    assert!(!shown.contains('\u{1b}'), "{shown:?}");
    assert!(shown.contains("evil"), "{shown}");
}

// A favourite and a boost of your own show in the counts.
#[test]
fn your_favourite_is_a_full_star() {
    let mut s = status("1", "x");
    s.favourited = Some(true);
    let mut app = app_with(vec![s]);
    assert!(screen(&mut app, 80, 12).contains("★ 5"));
}

// The files of a post, sensitive or not, and their alt text.
#[test]
fn a_posts_files_are_counted_with_their_alt_text() {
    let mut s = status("1", "写真");
    s.sensitive = true;
    s.media_attachments = vec![
        Media {
            kind: "image".into(),
            description: Some("山の写真".into()),
            ..Media::default()
        },
        Media {
            kind: "video".into(),
            ..Media::default()
        },
    ];
    let mut app = app_with(vec![s]);
    let shown = screen(&mut app, 80, 16);
    assert!(
        shown.contains("⚠ sensitive ▣ 1 picture · ▶ 1 video"),
        "{shown}"
    );
    assert!(shown.contains("ALT 山の写真"), "{shown}");
}

// At every width the client draws, a wide character is never cut in
// half and nothing is drawn past the edge.
#[test]
fn no_width_cuts_a_wide_character() {
    let text = "家族👨‍👩‍👧と🇯🇵の旗 ".repeat(8);
    let mut app = app_with(vec![status("1", &text), status("2", "二つ目")]);
    for w in MIN_W..=100 {
        let mut term = Terminal::new(TestBackend::new(w, 20)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let buf = term.backend().buffer();
        for y in 0..20 {
            for x in 0..w {
                let s = buf[(x, y)].symbol();
                assert!(
                    x + cells(s) as u16 <= w,
                    "width {w}: {s:?} at {x} runs past the edge"
                );
            }
        }
    }
}

#[test]
fn a_terminal_too_small_says_so() {
    let mut app = app_with(Vec::new());
    let s = screen(&mut app, 20, 6);
    assert!(s.contains("Terminal too small"), "{s}");
    assert!(s.contains("24x8 needed"), "{s}");
}

// A visitor's Home says how to log in.
#[test]
fn a_visitors_home_says_how_to_log_in() {
    let (mut app, _) = App::new(None, "http://127.0.0.1:9");
    press(&mut app, '1');
    let s = screen(&mut app, 120, 10);
    assert!(s.contains("127.0.0.1:9 (visitor)"), "{s}");
    assert!(s.contains("run `mstdn login --host 127.0.0.1:9`"), "{s}");
}

#[test]
fn notifications_say_what_happened() {
    let mut app = app_with(Vec::new());
    let jobs = app.handle_key(KeyEvent::new(KeyCode::Char('4'), KeyModifiers::NONE));
    let mut gus = status("x", "").account;
    gus.display_name = "Gus 🐈‍⬛".into();
    gus.acct = "gus@x.test".into();
    app.handle_answer(
        jobs[0].0,
        Event::Notifications {
            max_id: None,
            result: Ok(Page {
                items: vec![Notification {
                    id: "n1".into(),
                    kind: "favourite".into(),
                    created_at: "2026-11-26T06:00:00.000Z".into(),
                    account: Some(gus),
                    status: Some(status("1", "おはよう")),
                }],
                next: Next::End,
            }),
        },
    );
    let s = screen(&mut app, 100, 12);
    assert!(s.contains("Gus 🐈‍⬛ @gus@x.test favourited your post"), "{s}");
    assert!(s.contains("おはよう"), "{s}");
}

// The composer shows who it replies to, the count and who may see it.
#[test]
fn the_composer_shows_the_count_and_visibility() {
    let mut app = app_with(vec![status("1", "x")]);
    press(&mut app, 'r');
    for c in "よろしく".chars() {
        press(&mut app, c);
    }
    let s = screen(&mut app, 100, 24);
    assert!(s.contains("Reply"), "{s}");
    assert!(s.contains("replying to @alice"), "{s}");
    assert!(s.contains("@alice よろしく"), "{s}");
    assert!(s.contains("11/500 · public"), "{s}");
}

#[test]
fn the_help_lists_every_section() {
    let mut app = app_with(Vec::new());
    press(&mut app, '?');
    let mut shown = String::new();
    for _ in 0..20 {
        shown.push_str(&screen(&mut app, 100, 30));
        app.handle_key(KeyEvent::from(KeyCode::PageDown));
    }
    for section in crate::tui::keys::HELP {
        assert!(shown.contains(section.title), "missing {}", section.title);
    }
}

#[test]
fn the_tabs_and_help_speak_japanese() {
    crate::i18n::set(crate::i18n::Lang::Ja);
    let mut app = app_with(vec![status("1", "x")]);
    let s = screen(&mut app, 100, 12);
    crate::i18n::set(crate::i18n::Lang::En);
    assert!(s.contains("1 ホーム"), "{s}");
    assert!(s.contains("4 通知"), "{s}");
}

#[test]
fn the_scroll_keeps_the_selection_on_screen() {
    for selected in 0..20 {
        let top = scroll_offset(selected, 0, 20, 10, |_| 3);
        assert!(top <= selected);
        assert!((selected - top + 1) * 3 <= 10 || top == selected);
    }
}

#[test]
fn new_screens_draw_in_both_languages_and_small_terminals() {
    for lang in crate::i18n::ALL {
        crate::i18n::set(lang);
        let mut app = app_with(vec![status("1", "<script>bad</script> hello")]);
        app.accounts = vec![app.session.clone().unwrap()];
        app.profile.account = Some(Account {
            acct: "bob\x1b[31m".into(),
            note: "<p>Biography</p>".into(),
            ..Default::default()
        });
        app.profile.posts.loaded = true;
        for tab in [Tab::Home, Tab::Profile] {
            app.tab = tab;
            app.search.editing = tab == Tab::Home;
            for (w, h) in [(120, 40), (24, 8), (10, 4)] {
                let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
                term.draw(|f| draw(f, &mut app)).unwrap();
            }
        }
        for overlay in [
            Overlay::Settings { selected: 0 },
            Overlay::Languages { selected: 1 },
            Overlay::Accounts { selected: 0 },
            Overlay::Viewer {
                files: vec![Media {
                    kind: "video".into(),
                    url: Some("http://127.0.0.1:9/clip.mp4".into()),
                    description: Some("Caption".into()),
                    ..Default::default()
                }],
                index: 0,
                generation: 0,
            },
        ] {
            app.overlay = Some(overlay);
            let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
            term.draw(|f| draw(f, &mut app)).unwrap();
            let text: String = term
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect();
            if matches!(app.overlay, Some(Overlay::Viewer { .. })) {
                assert!(text.contains("Caption"));
            }
        }
    }
    crate::i18n::set(crate::i18n::Lang::En);
}

#[test]
fn search_scrolls_long_input_and_places_the_cursor_at_the_edit_position() {
    let mut app = app_with(Vec::new());
    app.handle_key(KeyEvent::from(KeyCode::Char('/')));
    app.handle_paste(&"a".repeat(60));
    let mut term = Terminal::new(TestBackend::new(24, 12)).unwrap();
    term.draw(|f| draw(f, &mut app)).unwrap();
    // The common popup has an 18-cell input; the last segment holds six characters.
    assert_eq!(term.get_cursor_position().unwrap(), (9, 3).into());
    app.handle_key(KeyEvent::from(KeyCode::Home));
    term.draw(|f| draw(f, &mut app)).unwrap();
    assert_eq!(term.get_cursor_position().unwrap(), (3, 3).into());
}

#[test]
fn confirmation_and_report_draw_their_target_and_keys_in_both_languages() {
    use crossterm::event::{KeyCode, KeyEvent};
    for (lang, confirm, comment, title) in [
        (
            crate::i18n::Lang::En,
            "y confirms; any other key cancels",
            "optional comment",
            "Report",
        ),
        (
            crate::i18n::Lang::Ja,
            "y で確定、それ以外のキーで取消",
            "コメント（任意）",
            "通報",
        ),
    ] {
        crate::i18n::set(lang);
        let mut app = app_with(vec![status("s1", "post")]);
        app.handle_key(KeyEvent::from(KeyCode::Char('M')));
        assert!(screen(&mut app, 100, 30).contains(confirm));
        app.handle_key(KeyEvent::from(KeyCode::Char('n')));
        app.handle_key(KeyEvent::from(KeyCode::Char('!')));
        let drawn = screen(&mut app, 100, 30);
        assert!(
            drawn.contains(title) && drawn.contains("@alice · s1") && drawn.contains(comment),
            "{drawn}"
        );
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        assert!(screen(&mut app, 100, 30).contains(confirm));
    }
    crate::i18n::set(crate::i18n::Lang::En);
}

#[test]
fn accepted_quotes_render_without_version_and_unapproved_quotes_hide_text() {
    let mut app = App::new(None, "http://127.0.0.1:9").0;
    let mut s: Status = serde_json::from_value(serde_json::json!({"id":"q","content":"Comment","quote":{"state":"accepted","quoted_status":{"id":"original","content":"Quoted body","account":{"acct":"alice"}}}})).unwrap();
    app.timelines[1].loaded = true;
    app.timelines[1].items = vec![s.clone()];
    let render = |app: &mut App| {
        let backend = ratatui::backend::TestBackend::new(120, 30);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|f| super::draw(f, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>()
    };
    assert!(render(&mut app).contains("Quoted body"));
    s.quote.as_mut().unwrap().state = "pending".into();
    app.timelines[1].items = vec![s];
    assert!(!render(&mut app).contains("Quoted body"));
}

#[test]
fn custom_emoji_resolve_from_the_entity_and_fall_back_without_graphics() {
    let emojis = vec![crate::api::types::Emoji {
        shortcode: "cat".into(),
        url: "https://example.test/animated.gif".into(),
        static_url: "https://example.test/static.png".into(),
    }];
    let mut urls = Vec::new();
    assert_eq!(
        emoji_text("猫 :cat: :missing:", &emojis, false, &mut urls),
        "猫 :cat: :missing:"
    );
    assert!(urls.is_empty());
    assert_eq!(
        emoji_text("猫 :cat: :missing:", &emojis, true, &mut urls),
        format!("猫 {} :missing:", terminal_kit::emoji::EMOJI_CELLS)
    );
    assert_eq!(urls, ["https://example.test/static.png"]);
    let mut other = emojis;
    other[0].static_url.clear();
    emoji_text(":cat:", &other, true, &mut urls);
    assert_eq!(urls[1], "https://example.test/animated.gif");
}

#[test]
fn stage_four_screens_fit_in_both_languages() {
    use crate::api::types::{AccountSource, Conversation};
    use crate::tui::worker::Event;
    for lang in crate::i18n::ALL {
        crate::i18n::set(lang);
        for width in [40, 80, 160] {
            let mut app = app_with(vec![status("s", "body")]);
            app.tab = Tab::Profile;
            let id = app.session.as_ref().unwrap().user_id.clone();
            let a = Account {
                id,
                display_name: "Me".into(),
                source: Some(AccountSource::default()),
                ..Account::default()
            };
            app.profile.account = Some(a.clone());
            let jobs = app.handle_key(KeyEvent::from(KeyCode::Char('e')));
            app.handle_answer(jobs[0].0, Event::ProfileEditor(Ok(a)));
            assert!(screen(&mut app, width, 24).contains(t("Edit profile")));
            for _ in 0..14 {
                app.handle_key(KeyEvent::from(KeyCode::Tab));
            }
            assert!(screen(&mut app, width, 24).contains(t("Header (ctrl+o)")));
            app.handle_key(KeyEvent::from(KeyCode::Esc));
            app.tab = Tab::Chat;
            app.chat.conversations.loaded = true;
            app.chat.conversations.items = vec![Conversation {
                id: "c".into(),
                accounts: vec![Account {
                    acct: "alice".into(),
                    ..Account::default()
                }],
                ..Conversation::default()
            }];
            assert!(screen(&mut app, width, 24).contains("@alice"));
            app.tab = Tab::Home;
            app.handle_key(KeyEvent::from(KeyCode::Char('+')));
            assert!(screen(&mut app, width, 24).contains(t("Add column")));
        }
    }
    crate::i18n::set(crate::i18n::Lang::En);
}

#[test]
fn link_cards_follow_the_content_warning_and_open_only_on_explicit_key() {
    let mut post = status("42", "article");
    post.spoiler_text = "spoiler".into();
    post.card = Some(crate::api::types::PreviewCard {
        url: "https://example.test/article".into(),
        title: "A card title".into(),
        description: "Card description".into(),
        image: Some("https://example.test/card.png".into()),
    });
    let mut app = app_with(vec![post]);
    assert!(!screen(&mut app, 100, 25).contains("A card title"));
    assert!(
        app.handle_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::NONE))
            .is_empty()
    );
    app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
    let shown = screen(&mut app, 100, 25);
    assert!(shown.contains("A card title"));
    assert!(shown.contains("Card description"));
    assert!(
        matches!(&app.handle_key(KeyEvent::new(KeyCode::Char('O'),KeyModifiers::NONE))[0].1,crate::tui::worker::Job::OpenLink {url,..} if url=="https://example.test/article")
    );
}

#[test]
fn search_box_results_title_empty_message_and_cancel_in_both_languages() {
    use crate::i18n::Lang;
    for (lang, title, hint, results, back, empty) in [
        (
            Lang::En,
            "Search · Posts",
            "Enter search · Ctrl+T posts/accounts · Esc cancel",
            "1 Search: rust",
            "Esc back to timeline",
            "No results for \"rust\"",
        ),
        (
            Lang::Ja,
            "検索 · 投稿",
            "Enter で検索 · Ctrl+T 投稿/アカウント · Esc キャンセル",
            "1 検索: rust",
            "Esc タイムラインに戻る",
            "「rust」の検索結果はありません",
        ),
    ] {
        crate::i18n::set(lang);
        let mut app = app_with(vec![status("1", "Original timeline")]);
        let before = screen(&mut app, 120, 30);
        press(&mut app, '/');
        let opened = screen(&mut app, 120, 30);
        assert!(opened.contains(title), "{opened}");
        assert!(opened.contains(hint), "{opened}");
        assert!(opened.contains('╭') && opened.contains('╰'), "{opened}");
        app.handle_paste("rust");
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(screen(&mut app, 120, 30), before);
        press(&mut app, '/');
        let seq = app.handle_key(KeyEvent::from(KeyCode::Enter))[0].0;
        app.handle_answer(
            seq,
            Event::Search {
                next: None,
                result: Ok(crate::tui::worker::SearchPage {
                    posts: Vec::new(),
                    accounts: Vec::new(),
                    next: None,
                }),
            },
        );
        let shown = screen(&mut app, 120, 30);
        for text in [results, back, empty] {
            assert!(shown.contains(text), "{text}: {shown}");
        }
        assert!(!shown.contains("Original timeline"));
        assert!(!shown.contains(title));
        let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let cell = &term.backend().buffer()[(1, 0)];
        assert_eq!(cell.fg, app.theme.selected().fg.unwrap());
        assert!(cell.modifier.contains(app.theme.selected().add_modifier));
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(screen(&mut app, 120, 30), before);
    }
    crate::i18n::set(Lang::En);
}

#[test]
fn cancel_search_box_preserves_scroll_at_the_bottom_of_a_short_screen() {
    let mut app = app_with(
        (0..20)
            .map(|i| status(&i.to_string(), "post body"))
            .collect(),
    );
    app.timelines[0].selected = 19;
    let before = screen(&mut app, 80, 12);
    let offset = app.timelines[0].offset;
    press(&mut app, '/');
    screen(&mut app, 80, 12);
    app.handle_key(KeyEvent::from(KeyCode::Esc));
    assert_eq!(app.timelines[0].offset, offset);
    assert_eq!(screen(&mut app, 80, 12), before);
}
