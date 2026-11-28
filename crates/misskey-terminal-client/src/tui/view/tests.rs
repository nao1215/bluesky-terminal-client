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
        lines[0].starts_with(" 1 Home   2 Local   3 Global   4 Notifications   5 Profile   6 Chat"),
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
        lines[9].starts_with(
            " ? help  j k move  tab next tab  . actions  space open in browser  n note  R refresh  / search"
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

/// `n` laid out as the timeline lays it out at `width`, with pictures.
fn laid_out(app: &App, n: &Note, width: u16) -> NoteLines {
    let ctx = Ctx {
        opened: &app.opened,
        emojis: &app.emojis,
        me: None,
        renoted: &app.renoted,
        fresh: &app.fresh,
        t: app.theme,
    };
    NoteLines::new(n, width, Some((8, 16)), &ctx)
}

// A video is drawn by the poster the server made of it (thumbnailUrl), in
// the box a picture gets and in the note's order among the pictures, under
// the line that says it is a video, as bsky draws a video's thumbnail under
// "▶ video". A video without a poster stays that line alone, and the file
// itself is never asked for to draw the list.
#[test]
fn a_video_is_drawn_by_its_thumbnail_among_the_pictures() {
    let app = with_notes(Vec::new());
    let mixed = note(json!({
        "id": "v", "createdAt": "t", "user": {"username": "vic"}, "text": "動画 🎬",
        "files": [
            {"id": "a", "type": "video/mp4", "url": format!("{NOWHERE}/a.mp4"),
             "thumbnailUrl": format!("{NOWHERE}/a.webp"), "isSensitive": true,
             "properties": {"width": 1920, "height": 1080}},
            {"id": "b", "type": "image/png", "url": format!("{NOWHERE}/b.png"),
             "thumbnailUrl": format!("{NOWHERE}/b.webp")},
            {"id": "c", "type": "video/webm", "url": format!("{NOWHERE}/c.webm")}
        ]
    }));
    let nl = laid_out(&app, &mixed, 80);
    assert_eq!(
        nl.images,
        vec![
            (format!("{NOWHERE}/a.webp"), Some((1920, 1080))),
            (format!("{NOWHERE}/b.webp"), None),
        ]
    );
    // Two boxes of 36 cells: the 16:9 poster needs 11 rows, the picture
    // without a shape the default 8; the taller decides.
    assert_eq!(nl.image_rows, 11);
    let body: Vec<String> = nl.body.iter().map(ToString::to_string).collect();
    assert!(
        body.contains(&"⚠ 1 sensitive file ▶ 2 videos".to_string()),
        "{body:?}"
    );

    let bare = note(json!({
        "id": "w", "createdAt": "t", "user": {"username": "vic"},
        "files": [{"id": "c", "type": "video/mp4", "url": format!("{NOWHERE}/c.mp4")}]
    }));
    let nl = laid_out(&app, &bare, 80);
    assert!(nl.images.is_empty());
    assert_eq!(nl.image_rows, 0);

    // Posters count among the four drawn, and the rest are counted.
    let mut files: Vec<Value> = (0..4)
        .map(|i| json!({"id": format!("p{i}"), "type": "image/png", "url": format!("{NOWHERE}/p{i}.png")}))
        .collect();
    files.insert(
        1,
        json!({"id": "v", "type": "video/mp4", "url": format!("{NOWHERE}/v.mp4"), "thumbnailUrl": format!("{NOWHERE}/v.webp")}),
    );
    let five =
        note(json!({"id": "x", "createdAt": "t", "user": {"username": "vic"}, "files": files}));
    let nl = laid_out(&app, &five, 80);
    let urls: Vec<&str> = nl.images.iter().map(|(u, _)| u.as_str()).collect();
    assert_eq!(
        urls,
        [
            format!("{NOWHERE}/p0.png"),
            format!("{NOWHERE}/v.webp"),
            format!("{NOWHERE}/p1.png"),
            format!("{NOWHERE}/p2.png"),
        ]
    );
    let body: Vec<String> = nl.body.iter().map(ToString::to_string).collect();
    assert!(
        body.contains(&"+1 more: space shows them all".to_string()),
        "{body:?}"
    );

    // Without pictures the note still says what it carries.
    let mut app = with_notes(vec![json!({
        "id": "v", "createdAt": "t", "user": {"username": "vic"}, "text": "動画 🎬",
        "files": [
            {"id": "a", "type": "video/mp4", "thumbnailUrl": format!("{NOWHERE}/a.webp")},
            {"id": "b", "type": "image/png", "url": format!("{NOWHERE}/b.png")}
        ]
    })]);
    let text = render_text(&mut app, 80, 12);
    assert!(text.contains("▣ 1 picture · ▶ 1 video"), "{text}");
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
    let screen = render(&mut app, 100, 140);
    for want in [
        "Keys",
        "General",
        "1 2 3 4 5 6",
        "Columns",
        "← → H L",
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
        // Wide: Windows says why the connection failed at more length, and
        // the hint comes after it.
        screen = rows(&mut app, &mut images, 240, 20).join("\n");
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
               "text": "$[x2 $[tada 🎉祝]] <small>まんなか</small> **太字👨‍👩‍👧** ?[リンク](https://e.test/a) :blobcat:"}),
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
fn profiles_and_search_results_draw_their_headers_and_lists() {
    let mut app = with_notes(vec![]);
    let jobs = app.handle_key(key('5'));
    app.handle_answer(
        jobs[0].0,
        Event::Profile(Ok(Box::new((
            serde_json::from_value(json!({"id": "u-me", "username": "me", "name": "わたし 👩🏽‍💻",
                "description": "$[x2 Rust] と **Go** が好き\n二行目", "notesCount": 12, "followingCount": 3, "followersCount": 4})).unwrap(),
            vec![note(json!({"id": "n1", "createdAt": "t", "user": {"username": "me"}, "text": "自分のノート"}))],
        )))),
    );
    let screen = render_text(&mut app, 90, 20);
    assert!(screen.contains("わたし 👩🏽‍💻\n"), "{screen}");
    assert!(
        screen.contains("4 followers  3 following  12 notes"),
        "{screen}"
    );
    // Your own: the buttons for the keys that edit it, as bsky's.
    assert!(
        screen.contains("this is you  [ e Edit profile ]  [ s Settings ]"),
        "{screen}"
    );
    assert!(screen.contains("Rust と Go が好き"), "{screen}");
    assert!(screen.contains("自分のノート"), "{screen}");
    assert!(screen.contains("e edit profile  s settings"), "{screen}");
    crate::i18n::set(crate::i18n::Lang::Ja);
    let ja = render_text(&mut app, 90, 20);
    crate::i18n::set(crate::i18n::Lang::En);
    assert!(ja.contains("4 フォロワー  3 フォロー  12 ノート"), "{ja}");
    assert!(ja.contains("[ e プロフィールを編集 ]"), "{ja}");
    app.handle_key(key('/'));
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
    assert!(screen.contains("Search: rust"), "{screen}");
    assert!(
        screen.contains("Ferris 🦀 @ferris@x.test  ✓ following"),
        "{screen}"
    );
    assert!(screen.contains("crab"), "{screen}");
    assert!(
        screen.contains("j k move  . actions  / edit query"),
        "{screen}"
    );
    // . lists what the keys do to the account found.
    let names: Vec<&str> = keys::actions(&app).iter().map(|(k, _)| *k).collect();
    assert_eq!(names, ["enter", "f", "M", "B"]);
    assert_eq!(keys::actions(&app)[1].1, "unfollow");
}

// Another account's profile says how you are related, as bsky's: whether
// you follow it, it follows you, you muted or blocked it, with the key
// that changes it; its avatar is drawn big beside the header.
#[test]
fn another_profile_says_how_you_are_related() {
    let mut app = with_notes(vec![json!({"id": "n1", "createdAt": "t",
        "user": {"id": "u-ann", "username": "ann", "host": "x.test"}, "text": "hi"})]);
    let jobs = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_answer(
        jobs[0].0,
        Event::Profile(Ok(Box::new((
            serde_json::from_value(json!({"id": "u-ann", "username": "ann", "host": "x.test",
                "name": "アン 🇯🇵", "isFollowed": true, "isMuted": true, "isBlocking": true,
                "isLocked": true, "avatarUrl": format!("{NOWHERE}/a.png")}))
            .unwrap(),
            vec![],
        )))),
    );
    let screen = render_text(&mut app, 100, 16);
    assert!(
        screen.contains("@ann@x.test  🔒 approves followers"),
        "{screen}"
    );
    assert!(
        screen.contains("not following  · follows you  · muted  · blocked  (f to toggle)"),
        "{screen}"
    );
    assert!(!screen.contains("this is you"), "{screen}");
    assert!(
        screen.contains("esc back to timeline  j k move  . actions"),
        "{screen}"
    );
    let names: Vec<&str> = keys::actions(&app).iter().map(|(k, _)| *k).collect();
    assert_eq!(names, ["m", "f", "M", "B"]);
    // With pictures, the text starts right of the big avatar.
    let pictures = render(&mut app, 100, 16);
    let line = pictures.lines().find(|l| l.contains("アン")).unwrap();
    let x = crate::tui::text::cells(&line[..line.find("アン").unwrap()]);
    assert_eq!(x, usize::from(MARK_W + BIG_AVATAR.0 + 2), "{pictures}");
}

// The notifications not read when the tab showed them keep a dot; the
// Chat tab counts the conversations with a message not read.
#[test]
fn unread_notifications_and_chats_are_marked() {
    let mut app = with_notes(vec![]);
    app.unread = Some(1);
    app.chat.convos.loaded = true;
    app.chat.convos.items = vec![
        serde_json::from_value(json!({"id": "c1", "createdAt": "t", "fromUserId": "u-ann",
            "fromUser": {"id": "u-ann", "username": "ann"}, "toUserId": "u-me", "text": "やあ", "isRead": false}))
        .unwrap(),
        serde_json::from_value(json!({"id": "c2", "createdAt": "t", "fromUserId": "u-me",
            "toUserId": "u-bob", "toUser": {"id": "u-bob", "username": "bob"}, "text": "どうも", "isRead": false}))
        .unwrap(),
    ];
    let screen = render_text(&mut app, 100, 12);
    assert!(screen.contains(" 6 Chat (1)"), "{screen}");
    // Read in the background at the start, then shown.
    app.notifications.loaded = true;
    app.notifications.items = vec![
        serde_json::from_value(json!({"id": "a", "createdAt": "t", "type": "follow",
            "user": {"username": "new"}}))
        .unwrap(),
        serde_json::from_value(json!({"id": "b", "createdAt": "t", "type": "follow",
            "user": {"username": "old"}}))
        .unwrap(),
    ];
    let jobs = app.handle_key(key('4'));
    assert_eq!(jobs[0].1, crate::tui::worker::Job::MarkRead);
    let screen = render_text(&mut app, 100, 12);
    assert!(screen.contains("● new @new followed you"), "{screen}");
    assert!(screen.contains("\n   old @old followed you"), "{screen}");
    app.handle_key(key('6'));
    let screen = render_text(&mut app, 100, 12);
    assert!(screen.contains("you: どうも"), "{screen}");
}

// The Chat tab names whom a conversation is with by display name and
// address, as bsky's list of conversations does, and draws the custom
// emoji of the names and the messages as a note's: the sender's own
// (user.emojis), else the server's for one of its accounts; without
// pictures their names stay.
#[test]
fn chat_names_and_custom_emoji_are_drawn_as_a_notes_are() {
    let mut app = with_notes(vec![]);
    app.emojis = [("budou".to_string(), format!("{NOWHERE}/budou.png"))]
        .into_iter()
        .collect();
    let mika = json!({"id": "u-mika", "username": "mika", "host": "mi.example",
        "name": "みか :iruka:", "emojis": {"iruka": format!("{NOWHERE}/iruka.png")}});
    let theirs = json!({"id": "c1", "createdAt": "t", "fromUserId": "u-mika", "fromUser": mika,
        "toUserId": "u-me", "text": "やあ :iruka: 👨‍👩‍👧"});
    let mine = json!({"id": "c2", "createdAt": "t", "fromUserId": "u-me",
        "fromUser": {"id": "u-me", "username": "me"}, "toUserId": "u-mika", "toUser": mika,
        "text": "ぶどう :budou: 🇯🇵"});
    app.chat.convos.loaded = true;
    app.chat.convos.items = vec![serde_json::from_value(theirs.clone()).unwrap()];
    drop(app.handle_key(key('6')));
    let screen = render(&mut app, 100, 12);
    assert!(screen.contains("みか"), "{screen}");
    assert!(screen.contains("@mika@mi.example"), "{screen}");
    assert!(screen.contains("やあ"), "{screen}");
    assert!(!screen.contains(":iruka:"), "{screen}");
    let text = render_text(&mut app, 100, 12);
    assert!(text.contains("みか :iruka: @mika@mi.example"), "{text}");
    assert!(text.contains("やあ :iruka: 👨‍👩‍👧"), "{text}");
    // The list says whom the conversation is with once.
    assert!(!text.contains("@mika@mi.example:"), "{text}");

    app.pictures = true;
    drop(app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)));
    let open = app.chat.open.as_mut().unwrap();
    open.messages.loaded = true;
    open.messages.items = vec![
        serde_json::from_value(mine).unwrap(),
        serde_json::from_value(theirs).unwrap(),
    ];
    let screen = render(&mut app, 100, 14);
    assert!(screen.contains("@mika@mi.example"), "{screen}");
    assert!(screen.contains("ぶどう"), "{screen}");
    assert!(
        !screen.contains(":iruka:") && !screen.contains(":budou:"),
        "{screen}"
    );
    assert!(
        !screen.contains('\u{E000}') && !screen.contains('\u{E001}'),
        "{screen}"
    );
    let text = render_text(&mut app, 100, 14);
    assert!(text.contains("みか :iruka: @mika@mi.example"), "{text}");
    assert!(text.contains("みか :iruka: · t"), "{text}");
    assert!(text.contains("やあ :iruka: 👨‍👩‍👧"), "{text}");
    assert!(text.contains("ぶどう :budou: 🇯🇵"), "{text}");
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

// A custom emoji in a display name, in the note a notification is about,
// and in the quoted note's line is drawn as its picture too, as in a
// note's text; without pictures its name stays.
#[test]
fn custom_emoji_in_names_notifications_and_quotes_are_pictures() {
    let mut app = with_notes(vec![json!({
        "id": "q", "createdAt": "t",
        "user": {"username": "mika", "host": "x.test", "name": "みか :iruka:",
            "emojis": {"iruka": format!("{NOWHERE}/iruka.png")}},
        "text": "見て",
        "renoteId": "o",
        "renote": {"id": "o", "createdAt": "t", "user": {"username": "bob"}, "text": "ぶどう :budou: 🍇"}
    })]);
    app.emojis = [("budou".to_string(), format!("{NOWHERE}/budou.png"))]
        .into_iter()
        .collect();
    let screen = render(&mut app, 80, 12);
    assert!(screen.contains("みか"), "{screen}");
    assert!(!screen.contains(":iruka:"), "{screen}");
    assert!(screen.contains("❝ @bob: ぶどう"), "{screen}");
    assert!(!screen.contains(":budou:"), "{screen}");
    let text = render_text(&mut app, 80, 12);
    assert!(text.contains("みか :iruka: @mika@x.test"), "{text}");
    assert!(text.contains("❝ @bob: ぶどう :budou: 🍇"), "{text}");
    app.pictures = true;
    app.notifications.loaded = true;
    app.notifications.items = vec![serde_json::from_value(json!({
        "id": "a", "createdAt": "t", "type": "reaction", "reaction": ":budou@.:",
        "user": {"username": "mika", "host": "x.test", "name": "みか :iruka:",
            "emojis": {"iruka": format!("{NOWHERE}/iruka.png")}},
        "note": {"id": "m", "createdAt": "t", "user": {"username": "me"}, "text": "今日の :budou:"}
    }))
    .unwrap()];
    drop(app.handle_key(key('4')));
    let screen = render(&mut app, 80, 12);
    assert!(screen.contains("みか"), "{screen}");
    assert!(screen.contains("今日の"), "{screen}");
    assert!(
        !screen.contains(":iruka:") && !screen.contains(":budou:"),
        "{screen}"
    );
    let text = render_text(&mut app, 80, 12);
    assert!(
        text.contains("みか :iruka: @mika@x.test reacted :budou: to your note"),
        "{text}"
    );
    assert!(text.contains("今日の :budou:"), "{text}");
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

// A value too long for the settings screen keeps its end, cut between
// whole emoji: the start of a family or a skin tone is never left alone.
#[test]
fn a_long_setting_is_cut_between_whole_emoji() {
    let mut app = with_notes(vec![]);
    app.env.download_dir =
        Some("/home/me/写真/旅行👨‍👩‍👧👍🏽🇯🇵👨‍👩‍👧👍🏽🇯🇵👨‍👩‍👧👍🏽🇯🇵/家族👨‍👩‍👧👍🏽🇯🇵1️⃣e\u{301}👨‍👩‍👧👍🏽🇯🇵".into());
    app.overlay = Some(Overlay::Settings {
        selected: 0,
        edit: None,
    });
    for w in MIN_W..100 {
        let mut images = Images::none();
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
        }
    }
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

/// Notes as a busy timeline has them: MFM, CJK and emoji, custom emoji,
/// pictures, reactions, replies and renotes.
fn heavy_notes(n: usize) -> Vec<Value> {
    (0..n)
        .map(|i| {
            let text = format!(
                "$[x2 **見出し {i}**] と :blobcat: の話 👨‍👩‍👧 🇯🇵 <center>中央</center> @alice@x.test #タグ https://example.com/{i}\n{}",
                "長い本文が続きます。".repeat(8)
            );
            let mut v = json!({
                "id": format!("n{i:04}"), "createdAt": "2026-11-27T10:00:00.000Z",
                "userId": format!("u{i}"),
                "user": {"id": format!("u{i}"), "username": format!("user{i}"), "name": format!("名前 {i} :blobcat:"),
                    "avatarUrl": format!("{NOWHERE}/a{}.png", i % 20)},
                "text": text,
                "reactions": {"👍": 3, ":blobcat@.:": 2, "❤": 1},
                "renoteCount": 2, "repliesCount": 1,
            });
            if i % 3 == 0 {
                v["files"] = json!([{"id": format!("f{i}"), "type": "image/png", "name": "p.png",
                    "url": format!("{NOWHERE}/p{i}.png"), "thumbnailUrl": format!("{NOWHERE}/t{i}.png"),
                    "properties": {"width": 800, "height": 600}}]);
            }
            if i % 5 == 0 {
                v["reply"] = json!({"id": format!("r{i}"), "createdAt": "t", "user": {"username": "bob"}, "text": "返信先の本文 🙂"});
                v["replyId"] = json!(format!("r{i}"));
            }
            v
        })
        .collect()
}

/// Time to draw one frame of the timeline while `j` walks down 200 notes,
/// the cost a held key pays per step, as bsky's `frame_time`. Prints the
/// median and the slowest, with pictures (halfblocks) and without.
#[test]
#[ignore = "measurement"]
fn frame_time() {
    for pictures in [false, true] {
        let mut runs = Vec::new();
        for _ in 0..5 {
            let mut app = with_notes(heavy_notes(200));
            app.emojis = [("blobcat".to_string(), format!("{NOWHERE}/blobcat.png"))]
                .into_iter()
                .collect();
            let mut images = if pictures {
                Images::new(Picker::halfblocks(), None).unwrap()
            } else {
                app.pictures = false;
                Images::none()
            };
            let mut term = Terminal::new(TestBackend::new(120, 50)).unwrap();
            let mut times = Vec::new();
            let mut keys = Vec::new();
            for _ in 0..500 {
                let k = key(if times.len() % 250 < 199 { 'j' } else { 'k' });
                let start = std::time::Instant::now();
                drop(app.handle_key(k));
                keys.push(start.elapsed());
                let start = std::time::Instant::now();
                term.draw(|f| draw(f, &mut app, &mut images)).unwrap();
                times.push(start.elapsed());
            }
            times.sort();
            keys.sort();
            runs.push((
                times[times.len() / 2],
                times[times.len() - 1],
                keys[keys.len() / 2],
            ));
        }
        for (median, max, key) in runs {
            println!(
                "pictures {pictures}: frame median {median:?}, max {max:?}; key median {key:?}"
            );
        }
    }
}

/// Time to draw the first frame of each tab after switching to it, the
/// cost of `1` `2` ... on a full timeline.
#[test]
#[ignore = "measurement"]
fn tab_switch_time() {
    let mut app = with_notes(heavy_notes(200));
    app.pictures = false;
    let mut images = Images::none();
    let mut term = Terminal::new(TestBackend::new(120, 50)).unwrap();
    let mut times = Vec::new();
    for i in 0..200 {
        let k = key(if i % 2 == 0 { '4' } else { '1' });
        drop(app.handle_key(k));
        let start = std::time::Instant::now();
        term.draw(|f| draw(f, &mut app, &mut images)).unwrap();
        times.push(start.elapsed());
    }
    times.sort();
    println!(
        "tab switch: median {:?}, max {:?}",
        times[times.len() / 2],
        times[times.len() - 1]
    );
}

/// Where each update of `prev` to `next` lands on a terminal that moves
/// the cursor on by a character's width, as ratatui's crossterm backend
/// writes them: a cell right after the last one written is written where
/// the cursor is, without moving it there.
fn landed(prev: &ratatui::buffer::Buffer, next: &ratatui::buffer::Buffer) -> Vec<(u16, u16, u16)> {
    let mut out = Vec::new();
    let mut last: Option<(u16, u16)> = None;
    let mut cursor = 0;
    for (x, y, cell) in prev.diff(next) {
        let at = match last {
            Some((lx, ly)) if ly == y && x == lx + 1 => cursor,
            _ => x,
        };
        out.push((x, y, at));
        cursor = at + (cell.symbol().width() as u16).max(1);
        last = Some((x, y));
    }
    out
}

// A reaction with VS16 (🏛️) is two cells wide. ratatui writes the cell
// after such a character on its own, right after it, so the terminal put
// it, and the counts after it, one cell to the right of where ratatui
// thought they were: on kitty a later screen left "4   2" behind, and the
// timeline again drew "44  22". Every update lands where it is meant to.
#[test]
fn the_cells_after_a_vs16_emoji_land_where_they_are_meant_to() {
    let mut app = with_notes(vec![json!({
        "id": "b", "createdAt": "t", "user": {"username": "mika"},
        "text": "古い水族館 🏛️ と ❤️",
        "reactions": {"🏛️": 4, "❤️": 2}
    })]);
    let mut term = Terminal::new(TestBackend::new(80, 12)).unwrap();
    let mut images = Images::none();
    app.pictures = false;
    term.draw(|f| draw(f, &mut app, &mut images)).unwrap();
    let next = term.backend().buffer().clone();
    let mut prev = ratatui::buffer::Buffer::empty(next.area);
    for c in prev.content.iter_mut() {
        c.set_symbol("x");
    }
    let wrong: Vec<_> = landed(&prev, &next)
        .into_iter()
        .filter(|(x, _, at)| x != at)
        .collect();
    assert!(wrong.is_empty(), "{wrong:?}");
}

/// An app on the Home tab with columns of `sources`, each holding `note`.
fn with_columns(sources: &[crate::tui::columns::Source], text: &str) -> App {
    let mut app = with_notes(vec![]);
    app.settings
        .columns
        .insert("me@misskey.test".into(), sources.to_vec());
    for (seq, job) in app.show_columns() {
        if let crate::tui::worker::Job::Column { id, .. } = job {
            app.handle_answer(
                seq,
                Event::Column {
                    id,
                    until: None,
                    result: Ok(crate::tui::worker::ColumnPage::Notes(vec![note(json!({
                        "id": format!("n{id}"),
                        "createdAt": "2026-11-26T10:00:00.000Z",
                        "user": {"username": "ann"},
                        "text": text,
                    }))])),
                },
            );
        }
    }
    app.status = None;
    app
}

// Columns kept for the account are shown side by side on the Home tab, the
// tab bar still saying Home, each titled; the focused title is marked.
#[test]
fn the_home_tab_shows_its_columns_side_by_side() {
    use crate::tui::columns::Source;
    let sources = [
        Source::Home,
        Source::Antenna {
            id: "a1".into(),
            name: "ニュース 📰".into(),
        },
        Source::Search {
            query: "👨‍👩‍👧 🇯🇵".into(),
        },
    ];
    let mut app = with_columns(&sources, "家族 👨‍👩‍👧 🇯🇵 1️⃣ ❤️");
    assert_eq!(app.tab, Tab::Columns);
    let screen = render_text(&mut app, 120, 20);
    let lines: Vec<&str> = screen.lines().collect();
    assert!(lines[0].starts_with(" 1 Home "), "{screen}");
    assert!(lines[1].contains(" Home"), "{screen}");
    assert!(lines[1].contains(" 📡 ニュース 📰"), "{screen}");
    assert!(lines[1].contains(" Search: 👨‍👩‍👧 🇯🇵"), "{screen}");
    assert_eq!(screen.matches("家族 👨‍👩‍👧 🇯🇵 1️⃣ ❤️").count(), 3, "{screen}");
    assert!(screen.contains("← → column"), "{screen}");
    assert!(screen.contains("x remove"), "{screen}");
}

// A terminal too narrow for every column shows as many as fit, the focused
// one among them, and says how many are off screen on either side.
#[test]
fn a_narrow_terminal_shows_fewer_columns_and_counts_the_rest() {
    use crate::tui::columns::Source;
    let sources = [
        Source::Home,
        Source::Notifications,
        Source::Search {
            query: "猫".into()
        },
        Source::Antenna {
            id: "a1".into(),
            name: "長い名前のアンテナ 📡📡📡📡📡📡📡📡".into(),
        },
    ];
    let mut app = with_columns(&sources, "x");
    // 80 cells: two columns of at least 36.
    let screen = render_text(&mut app, 80, 20);
    let head = screen.lines().nth(1).unwrap().to_string();
    assert!(head.contains(" Home"), "{head}");
    assert!(head.contains("Notifications 2›"), "{head}");
    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    let screen = render_text(&mut app, 80, 20);
    let head = screen.lines().nth(1).unwrap().to_string();
    assert!(head.contains("‹2 Search: 猫"), "{head}");
    assert!(head.contains("📡 長い名前"), "{head}");
    assert!(!head.contains("›"), "nothing off to the right: {head}");
    // 30 cells: one column, the focused one.
    let screen = render_text(&mut app, 30, 20);
    let head = screen.lines().nth(1).unwrap().to_string();
    assert!(head.starts_with(" ‹3 📡"), "{head}");
}

// + opens the list of what a column can show; Enter on the search row asks
// for the query.
#[test]
fn plus_lists_what_a_column_can_show() {
    let mut app = with_notes(vec![]);
    app.handle_key(key('+'));
    let screen = render_text(&mut app, 100, 30);
    for want in [
        "Add a column",
        "▶ Home",
        "Notifications",
        "Your notes (@me)",
        "Search…",
        "enter add",
    ] {
        assert!(screen.contains(want), "{want}: {screen}");
    }
    app.handle_key(key('k'));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let screen = render_text(&mut app, 100, 30);
    assert!(screen.contains("Search notes for:"), "{screen}");
    assert!(screen.contains("esc back"), "{screen}");
}

// An account found by a search shows the custom emoji of its description
// as pictures, as its profile does; without pictures their names stay.
#[test]
fn an_account_found_draws_the_emoji_of_its_description() {
    let mut app = with_notes(vec![]);
    app.search.tab = Some(app.tab);
    app.search.result_mode = crate::tui::app::SearchMode::Accounts;
    app.search.users.loaded = true;
    app.search.users.items = vec![
        serde_json::from_value(json!({
            "id": "u-mika", "username": "mika", "host": "x.test", "name": "Mika",
            "description": "Aquariums :iruka: and the sea",
            "emojis": {"iruka": format!("{NOWHERE}/iruka.png")}
        }))
        .unwrap(),
    ];
    let text = render_text(&mut app, 80, 12);
    assert!(text.contains("Aquariums :iruka: and the sea"), "{text}");
    app.pictures = true;
    let screen = render(&mut app, 80, 12);
    assert!(screen.contains("Aquariums"), "{screen}");
    assert!(screen.contains("and the sea"), "{screen}");
    assert!(!screen.contains(":iruka:"), "{screen}");
}

#[test]
fn search_box_overlays_the_list_and_results_title_replaces_the_timeline_title() {
    let mut app = with_notes(vec![
        json!({"id":"home", "user":{"username":"alice"}, "text":"Original timeline"}),
    ]);
    app.handle_key(key('/'));
    let screen = render_text(&mut app, 100, 20);
    assert!(screen.contains("Search (Posts)"), "{screen}");
    assert!(
        screen.contains("Enter search · Ctrl+T posts/accounts · Esc cancel"),
        "{screen}"
    );
    assert!(screen.contains('┌') && screen.contains('┘'), "{screen}");
    assert!(screen.starts_with(" 1 Home"), "{screen}");
    assert!(!screen.contains("5 Search"), "{screen}");
    crate::i18n::set(crate::i18n::Lang::Ja);
    let ja = render_text(&mut app, 100, 20);
    crate::i18n::set(crate::i18n::Lang::En);
    assert!(ja.contains("検索 (投稿)"), "{ja}");
    app.handle_key(KeyEvent::from(KeyCode::Esc));
    assert!(render_text(&mut app, 100, 20).contains("Original timeline"));
    app.handle_key(key('/'));
    for c in "rust".chars() {
        app.handle_key(key(c));
    }
    let jobs = app.handle_key(KeyEvent::from(KeyCode::Enter));
    app.handle_answer(
        jobs[0].0,
        Event::SearchNotes {
            query: "rust".into(),
            until: None,
            result: Ok(vec![]),
        },
    );
    let screen = render_text(&mut app, 100, 20);
    assert!(screen.starts_with(" 1 Search: rust"), "{screen}");
    assert!(screen.contains("No results for \"rust\""), "{screen}");
    assert!(screen.contains("Esc back to timeline"), "{screen}");
    assert!(!screen.contains("Original timeline"), "{screen}");
    assert!(!screen.contains("Search (Posts)"), "{screen}");
    let mut term = Terminal::new(TestBackend::new(100, 20)).unwrap();
    term.draw(|f| draw(f, &mut app, &mut Images::none()))
        .unwrap();
    assert_eq!(
        term.backend().buffer()[(1, 0)].modifier,
        app.theme.selected().add_modifier
    );
    crate::i18n::set(crate::i18n::Lang::Ja);
    let ja = render_text(&mut app, 100, 20);
    crate::i18n::set(crate::i18n::Lang::En);
    assert!(ja.contains("検索: rust"), "{ja}");
    assert!(ja.contains("「rust」の検索結果はありません"), "{ja}");
}

#[test]
fn account_box_title_and_empty_results_use_the_same_search_area() {
    let mut app = with_notes(vec![]);
    app.handle_key(key('/'));
    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
    app.search.input = crate::tui::input::TextInput::single("nobody");
    assert!(render_text(&mut app, 100, 20).contains("Search (Accounts)"));
    let jobs = app.handle_key(KeyEvent::from(KeyCode::Enter));
    app.handle_answer(
        jobs[0].0,
        Event::SearchUsers {
            query: "nobody".into(),
            offset: 0,
            result: Ok(vec![]),
        },
    );
    let screen = render_text(&mut app, 100, 20);
    assert!(screen.starts_with(" 1 Search: nobody"), "{screen}");
    assert!(screen.contains("No results for \"nobody\""), "{screen}");
}
