use super::*;
use crate::config::Session;
use crate::tui::worker::{Event, Thread};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui_image::picker::Picker;
use serde_json::{Value, json};

const ORIGIN: &str = "https://misskey.test";
/// Where the pictures of these notes are: nowhere, so a frame that asks
/// for one reaches no server.
const NOWHERE: &str = "http://127.0.0.1:9";

fn session() -> Session {
    Session {
        service: ORIGIN.into(),
        user_id: "u-me".into(),
        username: "me".into(),
        token: "fake-token-for-tests".into(),
        permissions: Vec::new(),
    }
}

fn note(v: Value) -> Note {
    serde_json::from_value(v).unwrap()
}

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

/// A logged-in app whose home timeline holds `notes`.
fn with_notes(notes: Vec<Value>) -> App {
    let (mut app, jobs) = App::new(Some(session()), ORIGIN);
    app.handle_answer(
        jobs[0].0,
        Event::Timeline {
            kind: TimelineKind::Home,
            until: None,
            result: Ok(notes.into_iter().map(note).collect()),
        },
    );
    app.status = None;
    app.pending = 0;
    app
}

/// The rows of a frame, each cell's symbol, a wide character once.
fn rows(app: &mut App, images: &mut Images, w: u16, h: u16) -> Vec<String> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| draw(f, app, images)).unwrap();
    let buf = term.backend().buffer().clone();
    (0..h)
        .map(|y| {
            let mut row = String::new();
            let mut x = 0;
            while x < w {
                let sym = buf[(x, y)].symbol();
                x += sym.width().max(1) as u16;
                row.push_str(sym);
            }
            row.trim_end().to_string()
        })
        .collect()
}

/// A frame with pictures drawn by halfblocks.
fn render(app: &mut App, w: u16, h: u16) -> String {
    let mut images = Images::new(Picker::halfblocks(), None).unwrap();
    rows(app, &mut images, w, h).join("\n")
}

/// A frame of a terminal that shows no pictures.
fn render_text(app: &mut App, w: u16, h: u16) -> String {
    app.pictures = false;
    rows(app, &mut Images::none(), w, h).join("\n")
}

/// A piece of a cluster that only shows when the cluster was cut: a skin
/// tone alone, one letter of a flag, a joiner or a variation selector at
/// an end, a combining mark alone.
fn is_fragment(sym: &str) -> bool {
    let first = sym.chars().next();
    let last = sym.chars().next_back();
    let lone_flag_letter =
        sym.chars().count() == 1 && matches!(first, Some('\u{1F1E6}'..='\u{1F1FF}'));
    matches!(
        first,
        Some(
            '\u{1F3FB}'..='\u{1F3FF}'
            | '\u{200D}'
            | '\u{FE0F}'
            | '\u{20E3}'
            | '\u{300}'..='\u{36F}',
        )
    ) || matches!(last, Some('\u{200D}'))
        || lone_flag_letter
}

// bsky's layout: the tabs with their keys on top, the account at the
// right, the note, the keys under it, and the picture protocol at the
// bottom right.
#[test]
fn the_timeline_draws_the_tabs_the_note_and_the_keys() {
    let mut app = with_notes(vec![json!({
        "id": "9c", "createdAt": "not a time",
        "user": {"username": "bob", "host": "misskey.design", "name": "ボブ"},
        "text": "おはよう :blobcat: 🇯🇵",
        "reactions": {"👍": 2, ":blobcat@.:": 3},
        "renoteCount": 1, "repliesCount": 4
    })]);
    let screen = render_text(&mut app, 100, 12);
    let lines: Vec<&str> = screen.lines().collect();
    assert!(
        lines[0].starts_with(
            " 1 Home  2 Local  3 Global  4 Notifications  5 Search  6 Profile  7 Chat"
        ),
        "{screen}"
    );
    assert!(lines[0].ends_with("@me@misskey.test"), "{screen}");
    assert!(
        screen.contains("ボブ @bob@misskey.design · not a time"),
        "{screen}"
    );
    // A custom emoji is shown by its name, as text.
    assert!(screen.contains("おはよう :blobcat: 🇯🇵"), "{screen}");
    assert!(
        screen.contains("♡ 5   ⟳ 1   ↩ 4   :blobcat: 3   👍 2"),
        "{screen}"
    );
    assert!(
        lines[10].starts_with(
            " ? help  j k move  tab next tab  . actions  space open in browser  n note  R refresh  q quit"
        ),
        "{screen}"
    );
    assert!(lines[11].ends_with("text"), "{screen}");
}

// A visitor sees why home is empty and how to log in, and the server it
// reads at the right.
#[test]
fn a_visitor_is_told_how_to_log_in_on_home() {
    let (mut app, _) = App::new(None, ORIGIN);
    let screen = render_text(&mut app, 160, 10);
    assert!(
        screen
            .lines()
            .next()
            .unwrap()
            .ends_with("misskey.test (visitor)"),
        "{screen}"
    );
    assert!(!screen.contains("R to retry"), "{screen}");
    assert!(
        screen.contains("no account is logged in on misskey.test"),
        "{screen}"
    );
    assert!(
        screen.contains("run `msky login --host misskey.test`"),
        "{screen}"
    );
    assert!(
        screen.contains("the Local and Global tabs show"),
        "{screen}"
    );
}

// A warned note shows its warning and how long the text behind it is;
// w shows the text, and the files with it.
#[test]
fn a_content_warning_is_folded_until_w_opens_it() {
    let mut app = with_notes(vec![json!({
        "id": "w", "createdAt": "t", "user": {"username": "carol"},
        "cw": "ネタバレ注意", "text": "犯人は👨‍👩‍👧",
        "files": [{"id": "f", "type": "image/png", "url": format!("{NOWHERE}/f.png")}]
    })]);
    let folded = render_text(&mut app, 80, 12);
    assert!(folded.contains("⚠ CW: ネタバレ注意"), "{folded}");
    assert!(
        folded.contains("w shows it (4 characters, 1 files)"),
        "{folded}"
    );
    assert!(!folded.contains("犯人"), "{folded}");
    assert!(!folded.contains("picture"), "{folded}");
    app.handle_key(key('w'));
    let open = render_text(&mut app, 80, 12);
    assert!(open.contains("⚠ CW: ネタバレ注意"), "{open}");
    assert!(open.contains("犯人は👨‍👩‍👧"), "{open}");
    assert!(open.contains("▣ 1 picture"), "{open}");
}

// A renote shows the note it renotes under who renoted it; a quote shows
// its own text and a line for the note it quotes; a reply the note it
// answers above it.
#[test]
fn renotes_quotes_and_replies_say_what_they_are() {
    let mut app = with_notes(vec![
        json!({"id": "rn", "createdAt": "t", "user": {"username": "alice", "name": "Alice"},
               "renoteId": "o", "renote": {"id": "o", "createdAt": "t0", "user": {"username": "bob"}, "text": "元のノート"}}),
        json!({"id": "q", "createdAt": "t", "user": {"username": "carol"}, "text": "これ見て",
               "renoteId": "o2", "renote": {"id": "o2", "user": {"username": "dave", "host": "x.test"}, "text": "引用元\n二行目"}}),
        json!({"id": "r", "createdAt": "t", "user": {"username": "erin"}, "text": "返事",
               "replyId": "p", "reply": {"id": "p", "user": {"username": "fay", "name": "Fay"}, "text": "質問です"}}),
    ]);
    let screen = render_text(&mut app, 80, 30);
    assert!(screen.contains("⟳ renoted by Alice @alice"), "{screen}");
    assert!(screen.contains("bob @bob · t0"), "{screen}");
    assert!(screen.contains("元のノート"), "{screen}");
    assert!(screen.contains("❝ @dave@x.test: 引用元"), "{screen}");
    assert!(!screen.contains("二行目"), "{screen}");
    assert!(screen.contains("┆ Fay @fay: 質問です"), "{screen}");
    assert!(screen.contains("erin @erin · t ↩ reply"), "{screen}");
}

// A file the uploader marked sensitive is drawn as any other, with a mark
// that says what it is.
#[test]
fn a_sensitive_file_is_shown_with_a_mark() {
    let mut app = with_notes(vec![json!({
        "id": "s", "createdAt": "t", "user": {"username": "gus"}, "text": "注意",
        "files": [
            {"id": "a", "type": "image/png", "url": format!("{NOWHERE}/a.png"), "isSensitive": true},
            {"id": "b", "type": "video/mp4", "url": format!("{NOWHERE}/b.mp4")}
        ]
    })]);
    let screen = render(&mut app, 80, 24);
    assert!(screen.contains("⚠ 1 sensitive file ▶ 1 video"), "{screen}");
    let text = render_text(&mut app, 80, 24);
    assert!(
        text.contains("⚠ 1 sensitive file ▣ 1 picture · ▶ 1 video"),
        "{text}"
    );
}

// Emoji (a family, a skin tone, flags, a keycap, VS16) and CJK are wrapped
// and cut between whole clusters at every width: no row is wider than the
// screen, and no piece of a cluster is drawn on its own.
#[test]
fn emoji_and_cjk_are_never_split_at_any_width() {
    let text = "家族👨‍👩‍👧と旅行🇯🇵🇺🇸に行きました👍🏽 1️⃣ ❤️ かﾞ e\u{301} 日本語の長い文章がここに続きます";
    let mut app = with_notes(vec![json!({
        "id": "e", "createdAt": "2026-11-26T10:00:00.000Z",
        "user": {"username": "hal", "name": "名前👩🏽‍💻がとても長いアカウント🇯🇵"},
        "cw": "注意👨‍👩‍👧", "text": text, "reactions": {"👍🏽": 1, ":ねこ@.:": 2}
    })]);
    app.handle_key(key('w'));
    for w in MIN_W..70 {
        let mut images = Images::none();
        app.pictures = false;
        let mut term = Terminal::new(TestBackend::new(w, 30)).unwrap();
        term.draw(|f| draw(f, &mut app, &mut images)).unwrap();
        let buf = term.backend().buffer().clone();
        for y in 0..30 {
            let mut x = 0;
            while x < w {
                let sym = buf[(x, y)].symbol();
                assert!(!is_fragment(sym), "width {w} row {y}: {sym:?}");
                x += sym.width().max(1) as u16;
            }
            assert!(x <= w, "width {w} row {y} ran over");
        }
    }
    // At a width with room, every cluster of the text is on screen.
    let screen = render_text(&mut app, 60, 30);
    for piece in ["👨‍👩‍👧", "🇯🇵🇺🇸", "👍🏽", "1️⃣", "❤️", "e\u{301}"]
    {
        assert!(screen.contains(piece), "{piece}: {screen}");
    }
}

#[test]
fn a_terminal_too_small_says_the_size_it_needs() {
    let mut app = with_notes(vec![]);
    let screen = render_text(&mut app, 20, 6);
    assert!(screen.contains("Terminal too small"), "{screen}");
    assert!(screen.contains("24x8 needed, now"), "{screen}");
    assert!(screen.contains("20x6"), "{screen}");
}

// The help lists every section, and says how to close it.
#[test]
fn the_help_lists_the_keys() {
    let mut app = with_notes(vec![]);
    app.handle_key(key('?'));
    let screen = render(&mut app, 100, 120);
    for want in [
        "Keys",
        "General",
        "1 2 3 …",
        "Notes",
        "w",
        "Viewer",
        "Accounts",
        "esc/q/? close",
    ] {
        assert!(screen.contains(want), "{want}: {screen}");
    }
    // Without pictures there is no viewer, and space opens the browser.
    let text = render_text(&mut app, 100, 120);
    assert!(!text.contains(" Viewer"), "{text}");
    assert!(text.contains("open the note in the browser"), "{text}");
}

// The thread is titled, its replies indented with guides.
#[test]
fn a_thread_draws_its_replies_indented() {
    let mut app = with_notes(vec![
        json!({"id": "n", "createdAt": "t", "user": {"username": "ian"}, "text": "親"}),
    ]);
    let jobs = app.handle_key(key('v'));
    app.handle_answer(
        jobs[0].0,
        Event::Thread {
            id: "n".into(),
            result: Ok(Box::new(Thread {
                ancestors: vec![],
                note: note(json!({"id": "n", "createdAt": "t", "user": {"username": "ian"}, "text": "親"})),
                replies: vec![(
                    note(json!({"id": "r", "createdAt": "t", "user": {"username": "jo"}, "text": "子"})),
                    vec![note(json!({"id": "g", "createdAt": "t", "user": {"username": "kay"}, "text": "孫"}))],
                )],
            })),
        },
    );
    let screen = render_text(&mut app, 60, 20);
    assert!(screen.contains(" Thread   esc back"), "{screen}");
    let line = |s: &str| screen.lines().find(|l| l.contains(s)).unwrap().to_string();
    assert!(line("jo @jo").starts_with("  │"), "{screen}");
    assert!(line("kay @kay").starts_with("  │ │"), "{screen}");
}

// A notification says who did what to which note.
#[test]
fn notifications_say_who_did_what() {
    let (mut app, jobs) = App::new(Some(session()), ORIGIN);
    let items: Vec<Notification> = serde_json::from_value(json!([
        {"id": "1", "createdAt": "t", "type": "reaction", "reaction": ":blobcat@.:",
         "user": {"username": "bob", "host": "x.test", "name": "Bob 🐈‍⬛"}, "note": {"id": "n", "text": "朝ごはん 🍙"}},
        {"id": "2", "createdAt": "t", "type": "follow", "user": {"username": "eve"}},
        {"id": "3", "createdAt": "t", "type": "reply", "user": {"username": "dan"},
         "note": {"id": "m", "cw": "秘密", "text": "隠れた"}}
    ]))
    .unwrap();
    app.handle_answer(
        jobs[1].0,
        Event::Notifications {
            until: None,
            result: Ok(items),
        },
    );
    app.handle_key(key('4'));
    let screen = render_text(&mut app, 80, 16);
    assert!(
        screen.contains("Bob 🐈‍⬛ @bob@x.test reacted :blobcat: to your note · t"),
        "{screen}"
    );
    assert!(screen.contains("朝ごはん 🍙"), "{screen}");
    assert!(screen.contains("eve @eve followed you · t"), "{screen}");
    assert!(screen.contains("⚠ CW: 秘密"), "{screen}");
    assert!(!screen.contains("隠れた"), "{screen}");
}

// The viewer names a file that is not a picture, with the keys that save
// or open it, and its alt text.
#[test]
fn the_viewer_names_a_file_it_cannot_draw() {
    let mut app = with_notes(vec![json!({
        "id": "v", "createdAt": "t", "user": {"username": "lee"},
        "files": [{"id": "a", "name": "資料📄.pdf", "type": "application/pdf", "size": 3_355_443,
                   "url": format!("{NOWHERE}/a.pdf"), "comment": "猫が走る", "isSensitive": true}]
    })]);
    app.handle_key(key(' '));
    let screen = render(&mut app, 80, 20);
    assert!(
        screen.contains("1/1  資料📄.pdf  application/pdf · 3.2 MB  ⚠ sensitive"),
        "{screen}"
    );
    assert!(
        screen.contains("a file: d saves it, o opens it in the browser"),
        "{screen}"
    );
    assert!(screen.contains("猫が走る"), "{screen}");
    assert!(screen.contains(" d download  o open  esc back"), "{screen}");
}

// A video is played in the viewer, r replaying it, and says where it is;
// one that cannot be loaded says why, with the keys that save or open it.
#[test]
fn the_viewer_plays_a_video_and_says_where_it_is() {
    let mut app = with_notes(vec![json!({
        "id": "v", "createdAt": "t", "user": {"username": "lee"},
        "files": [{"id": "a", "name": "動画🎬.mp4", "type": "video/mp4", "size": 2000,
                   "url": format!("{NOWHERE}/a.mp4"), "properties": {"width": 64, "height": 36}}]
    })]);
    app.handle_key(key(' '));
    let mut images = Images::new(Picker::halfblocks(), None).unwrap();
    let screen = rows(&mut app, &mut images, 120, 20).join("\n");
    assert!(
        screen.contains("1/1  動画🎬.mp4  video/mp4 · 1 KB  "),
        "{screen}"
    );
    assert!(
        screen.contains(" r replay  d download  o open  esc back"),
        "{screen}"
    );
    let start = std::time::Instant::now();
    let mut screen = String::new();
    while start.elapsed() < std::time::Duration::from_secs(10) {
        images.poll();
        screen = rows(&mut app, &mut images, 120, 20).join("\n");
        if screen.contains("⚠") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(screen.contains("⚠ cannot load the video"), "{screen}");
    assert!(screen.contains("d saves it, o opens"), "{screen}");
}

// The account list marks the one in use.
#[test]
fn the_account_list_marks_the_one_in_use() {
    let mut app = with_notes(vec![]);
    app.accounts = vec![
        crate::tui::app::Account {
            host: "a.test".into(),
            username: "ann".into(),
        },
        crate::tui::app::Account {
            host: "misskey.test".into(),
            username: "me".into(),
        },
    ];
    app.handle_key(key('A'));
    let screen = render(&mut app, 80, 20);
    assert!(screen.contains("@me@misskey.test  in use"), "{screen}");
    assert!(
        screen.contains("enter use  x log out  a log in another"),
        "{screen}"
    );
}

// An error is shown in a box over everything, and the next key closes it.
#[test]
fn an_error_is_a_box_the_next_key_closes() {
    let mut app = with_notes(vec![
        json!({"id": "a", "createdAt": "t", "user": {"username": "x"}}),
    ]);
    app.handle_answer(
        99,
        Event::Downloaded(Err(crate::error::Error::io(
            "cannot write /dl/a.png: disk full",
        ))),
    );
    let screen = render_text(&mut app, 80, 20);
    assert!(
        screen.contains("cannot write /dl/a.png: disk full"),
        "{screen}"
    );
    assert!(screen.contains("any key closes this"), "{screen}");
    app.handle_key(key('j'));
    assert!(!render_text(&mut app, 80, 20).contains("any key closes this"));
}

// The login form lists the servers with their accounts, and shows the
// address to allow msky at while it waits.
#[test]
fn the_login_form_lists_servers_then_shows_the_address() {
    let (mut app, _) = App::new(None, ORIGIN);
    app.open_login();
    let screen = render(&mut app, 80, 24);
    assert!(screen.contains("Log in"), "{screen}");
    assert!(screen.contains("▶ misskey.io  Misskey.io"), "{screen}");
    assert!(screen.contains("takusuki.com  TRPGがすきー！"), "{screen}");
    assert!(screen.contains("reading Misskey Hub's list…"), "{screen}");
    assert!(
        screen.contains("j k move  enter log in  / other server  esc back"),
        "{screen}"
    );
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let screen = render(&mut app, 80, 24);
    assert!(screen.contains("signed in on misskey.io"), "{screen}");
    assert!(screen.contains("https://misskey.io/miauth/"), "{screen}");
    assert!(
        screen.contains("o open the page again  esc call it off"),
        "{screen}"
    );
}

// The viewer draws the picture shown, once it is downloaded and encoded,
// at its own shape: a picture the list drew is drawn again at the
// viewer's size.
#[test]
fn the_viewer_draws_the_picture_full_screen() {
    use crate::api::stub_tests::{Reply, serve};
    use std::sync::Arc;
    let png = std::fs::read("e2e/atago/testdata/photo.png").unwrap();
    let (url, _) = serve(Arc::new(move |_, _, _| Reply {
        status: 200,
        headers: Vec::new(),
        body: png.clone(),
    }));
    let pic = format!("{url}/photo.png");
    let mut app = with_notes(vec![json!({
        "id": "p", "createdAt": "t", "user": {"username": "pat"},
        "files": [{"id": "a", "name": "photo.png", "type": "image/png", "size": 242,
                   "url": pic, "thumbnailUrl": pic, "properties": {"width": 120, "height": 90}}]
    })]);
    let mut images = Images::new(Picker::halfblocks(), None).unwrap();
    let settle = |app: &mut App, images: &mut Images| {
        let start = std::time::Instant::now();
        loop {
            let screen = rows(app, images, 80, 30);
            images.poll();
            if !images.loading() || start.elapsed() > std::time::Duration::from_secs(10) {
                return screen;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    };
    settle(&mut app, &mut images);
    app.handle_key(key(' '));
    settle(&mut app, &mut images);
    // Half blocks paint a picture in the colors of its cells: the
    // picture covers most of the screen above the caption.
    let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
    term.draw(|f| draw(f, &mut app, &mut images)).unwrap();
    let buf = term.backend().buffer().clone();
    let painted = (0..26)
        .flat_map(|y| (0..80).map(move |x| (x, y)))
        .filter(|&(x, y)| buf[(x, y)].bg != ratatui::style::Color::Reset)
        .count();
    assert!(painted > 1000, "{painted} cells painted");
}

// MFM is drawn as its text: functions, tags and labeled links are their
// content, **bold** is bold, custom emoji stay as their names, and a quote
// or a reply line shows the text the same way.
#[test]
fn mfm_is_drawn_as_plain_text_with_bold_kept() {
    let mut app = with_notes(vec![
        json!({"id": "m", "createdAt": "t", "user": {"username": "mia"},
               "text": "$[x2 $[tada 🎉祝]] <center>まんなか</center> **太字👨‍👩‍👧** ?[リンク](https://e.test/a) :blobcat:"}),
        json!({"id": "q", "createdAt": "t", "user": {"username": "quin"}, "text": "引用",
               "renoteId": "o", "renote": {"id": "o", "user": {"username": "olly"}, "text": "$[spin 回る]ノート"}}),
    ]);
    app.pictures = false;
    let mut images = Images::none();
    let mut term = Terminal::new(TestBackend::new(80, 16)).unwrap();
    term.draw(|f| draw(f, &mut app, &mut images)).unwrap();
    let buf = term.backend().buffer().clone();
    let row = |y: u16| -> String {
        let mut s = String::new();
        let mut x = 0;
        while x < 80 {
            let sym = buf[(x, y)].symbol();
            x += sym.width().max(1) as u16;
            s.push_str(sym);
        }
        s
    };
    let screen: Vec<String> = (0..16).map(row).collect();
    let text = screen.join("\n");
    assert!(
        text.contains("🎉祝 まんなか 太字👨‍👩‍👧 リンク :blobcat:"),
        "{text}"
    );
    assert!(text.contains("❝ @olly: 回るノート"), "{text}");
    assert!(!text.contains("$["), "{text}");
    // 太 is bold, まんなか is not.
    let y = screen.iter().position(|l| l.contains("太字")).unwrap() as u16;
    let x_of = |s: &str| (0..80u16).find(|&x| buf[(x, y)].symbol() == s).unwrap();
    use ratatui::style::Modifier;
    assert!(buf[(x_of("太"), y)].modifier.contains(Modifier::BOLD));
    assert!(!buf[(x_of("ま"), y)].modifier.contains(Modifier::BOLD));
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

/// `with_notes` as an account that may write.
fn writer(notes: Vec<Value>) -> App {
    let mut app = with_notes(notes);
    app.session.as_mut().unwrap().permissions = crate::miauth::PERMISSIONS
        .iter()
        .map(|p| p.to_string())
        .collect();
    app
}

// The composer says what it answers, shows the warning and the text as
// typed, emoji and CJK whole, and the visibility and length under them.
#[test]
fn the_composer_shows_the_reply_the_warning_and_the_visibility() {
    let mut app = writer(vec![json!({"id": "a", "createdAt": "t",
        "user": {"username": "bob", "host": "x.test"}, "text": "$[x2 質問]です"})]);
    app.handle_key(key('r'));
    for c in "答え👨‍👩‍👧🇯🇵".chars() {
        app.handle_key(key(c));
    }
    app.handle_key(ctrl('w'));
    for c in "注意".chars() {
        app.handle_key(key(c));
    }
    app.handle_key(ctrl('p'));
    let screen = render(&mut app, 90, 30);
    assert!(screen.contains("Reply to @bob@x.test"), "{screen}");
    assert!(screen.contains("↩ @bob@x.test: 質問です"), "{screen}");
    assert!(screen.contains("CW: 注意"), "{screen}");
    assert!(screen.contains("答え👨‍👩‍👧🇯🇵"), "{screen}");
    // Counted in code points, as the server counts them.
    assert!(screen.contains("9 characters  ·  home"), "{screen}");
    assert!(
        screen
            .contains("ctrl+s send  ctrl+o attach  ctrl+w warning  ctrl+p visibility  esc cancel"),
        "{screen}"
    );
}

// e shows the reactions in a row, the chosen one marked.
#[test]
fn the_reaction_picker_shows_the_reactions() {
    let mut app = writer(vec![
        json!({"id": "a", "createdAt": "t", "user": {"username": "bob"}}),
    ]);
    app.handle_key(key('e'));
    let screen = render(&mut app, 80, 20);
    assert!(screen.contains("React"), "{screen}");
    assert!(
        screen.contains("type to search the server's emoji"),
        "{screen}"
    );
    assert!(screen.contains("▶    👍"), "{screen}");
    assert!(screen.contains("↑ ↓ choose  enter react"), "{screen}");
}

// The Profile tab shows the account's header (MFM of its description as
// plain text, its counts, how you are related) over its notes; the Search
// tab its box over the accounts found.
#[test]
fn the_profile_and_search_tabs_draw_their_headers_and_lists() {
    let mut app = with_notes(vec![]);
    let jobs = app.handle_key(key('6'));
    app.handle_answer(
        jobs[0].0,
        Event::Profile(Ok(Box::new((
            serde_json::from_value(json!({"id": "u-me", "username": "me", "name": "わたし 👩🏽‍💻",
                "description": "$[x2 Rust] と **Go** が好き\n二行目", "notesCount": 12, "followingCount": 3, "followersCount": 4})).unwrap(),
            vec![note(json!({"id": "n1", "createdAt": "t", "user": {"username": "me"}, "text": "自分のノート"}))],
        )))),
    );
    let screen = render_text(&mut app, 90, 20);
    assert!(screen.contains("わたし 👩🏽‍💻 @me"), "{screen}");
    assert!(
        screen.contains("12 notes  3 following  4 followers"),
        "{screen}"
    );
    assert!(screen.contains("Rust と Go が好き"), "{screen}");
    assert!(screen.contains("自分のノート"), "{screen}");
    app.handle_key(key('5'));
    for c in "rust".chars() {
        app.handle_key(key(c));
    }
    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
    let jobs = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_answer(
        jobs[0].0,
        Event::SearchUsers {
            query: "rust".into(),
            offset: 0,
            result: Ok(vec![
                serde_json::from_value(json!({"id": "u1", "username": "ferris", "host": "x.test",
                "name": "Ferris 🦀", "description": "**crab**", "isFollowing": true}))
                .unwrap(),
            ]),
        },
    );
    let screen = render_text(&mut app, 90, 20);
    assert!(screen.contains("Search accounts: rust"), "{screen}");
    assert!(
        screen.contains("Ferris 🦀 @ferris@x.test  ✓ following"),
        "{screen}"
    );
    assert!(screen.contains("crab"), "{screen}");
    assert!(screen.contains("enter profile  f follow"), "{screen}");
}

// A custom emoji the server has is drawn as its picture where pictures
// are shown, in the text and in the counts; without pictures, and for one
// it does not have, its name stays.
#[test]
fn custom_emoji_are_drawn_as_pictures_or_by_name() {
    let mut app = with_notes(vec![json!({
        "id": "e", "createdAt": "t", "user": {"username": "emi"},
        "text": "おはよう :blobcat: と :unknown: 👨‍👩‍👧",
        "reactions": {":blobcat@.:": 3, ":far@x.test:": 2},
        "reactionEmojis": {"far@x.test": format!("{NOWHERE}/far.png")}
    })]);
    app.emojis = [("blobcat".to_string(), format!("{NOWHERE}/blobcat.png"))]
        .into_iter()
        .collect();
    let screen = render(&mut app, 80, 12);
    assert!(!screen.contains(":blobcat:"), "{screen}");
    assert!(screen.contains(":unknown:"), "{screen}");
    assert!(
        !screen.contains('\u{E000}') && !screen.contains('\u{E001}'),
        "{screen}"
    );
    assert!(screen.contains("おはよう"), "{screen}");
    assert!(!screen.contains(":far@x.test:"), "{screen}");
    let text = render_text(&mut app, 80, 12);
    assert!(
        text.contains("おはよう :blobcat: と :unknown: 👨‍👩‍👧"),
        "{text}"
    );
    assert!(text.contains(":blobcat: 3"), "{text}");
}

// The settings screen in Japanese: the names padded by the cells they
// take, so the values line up; the tabs and hints translated too.
#[test]
fn the_settings_screen_lines_up_in_japanese() {
    let mut app = with_notes(vec![]);
    crate::i18n::set(crate::i18n::Lang::Ja);
    app.overlay = Some(Overlay::Settings {
        selected: 0,
        edit: None,
    });
    let screen = render_text(&mut app, 100, 24);
    crate::i18n::set(crate::i18n::Lang::En);
    assert!(screen.contains("1 ホーム"), "{screen}");
    assert!(screen.contains("設定"), "{screen}");
    let cols: Vec<usize> = ["terminal", "English"]
        .iter()
        .map(|v| {
            let line = screen.lines().find(|l| l.contains(v)).unwrap();
            crate::tui::text::cells(&line[..line.find(v).unwrap()])
        })
        .collect();
    assert_eq!(cols[0], cols[1], "{screen}");
    assert!(screen.contains("enter 変更"), "{screen}");
}

// The theme list shows where it is and its keys in every theme.
#[test]
fn the_theme_list_draws_every_theme() {
    let mut app = with_notes(vec![]);
    for i in 0..crate::tui::theme::THEMES.len() {
        app.overlay = Some(Overlay::Themes {
            selected: i,
            previous: 0,
        });
        let screen = render_text(&mut app, 60, 20);
        assert!(
            screen.contains(crate::tui::theme::THEMES[i].name),
            "{screen}"
        );
        assert!(screen.contains(&format!("{}/", i + 1)), "{screen}");
    }
}
