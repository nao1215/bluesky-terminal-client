use super::*;
use crate::config::Session;
use crate::tui::worker::{Event, MorePage};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui_image::picker::Picker;
use serde_json::json;

mod compose;
mod hints;
mod panels;
mod posts_and_help;
mod screens;
mod themes_and_lists;

fn render(app: &mut App, w: u16, h: u16) -> String {
    let mut images = Images::new(Picker::halfblocks(), None);
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| draw(f, app, &mut images)).unwrap();
    let buf = term.backend().buffer().clone();
    (0..h)
        .map(|y| {
            (0..w)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn session() -> Session {
    Session {
        service: "https://pds.test".into(),
        did: "did:plc:me".into(),
        handle: "me.test".into(),
        access_jwt: "a".into(),
        refresh_jwt: "r".into(),
    }
}

fn posts(n: usize) -> Vec<Post> {
    (0..n)
        .map(|i| {
            serde_json::from_value(json!({
                "uri": format!("at://p/{i}"), "cid": "c",
                "author": {"did": "did:plc:a", "handle": "alice.test", "displayName": "Alice"},
                "record": {"text": format!("post number {i}"), "createdAt": "2026-09-22T00:00:00Z"},
                "likeCount": i,
            }))
            .unwrap()
        })
        .collect()
}

fn render_text_only(app: &mut App, w: u16, h: u16) -> String {
    let mut images = Images::none();
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| draw(f, app, &mut images)).unwrap();
    let buf = term.backend().buffer().clone();
    // The cells a wide character covers are skipped, so the text reads
    // as the terminal shows it.
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
        .collect::<Vec<_>>()
        .join("\n")
}

/// Each row's cells, skipping the cells a wide character covers, so the
/// text reads as a terminal shows it.
fn cells(app: &mut App, w: u16, h: u16) -> Vec<Vec<String>> {
    let mut images = Images::new(Picker::halfblocks(), None);
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| draw(f, app, &mut images)).unwrap();
    let buf = term.backend().buffer().clone();
    (0..h)
        .map(|y| {
            let mut row = Vec::new();
            let mut x = 0;
            while x < w {
                let sym = buf[(x, y)].symbol().to_string();
                x += sym.width().max(1) as u16;
                row.push(sym);
            }
            row
        })
        .collect()
}

/// A piece of a cluster that only shows when the cluster was cut: a
/// skin tone alone, one letter of a flag, a joiner or a variation
/// selector at an end, a combining mark alone.
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

fn own_profile() -> Profile {
    serde_json::from_value(json!({
        "did": "did:plc:me", "handle": "me.test",
        "displayName": "👨\u{200d}👩\u{200d}👧 Me 🇯🇵",
        "description": "مرحبا 1️⃣ 今日は👍🏽",
        "followersCount": 12, "followsCount": 3, "postsCount": 42
    }))
    .unwrap()
}

fn column_app(n: usize) -> App {
    let (mut app, _) = App::new(Some(session()), "x");
    let sources: Vec<crate::config::ColumnSource> = (0..n)
        .map(|i| crate::config::ColumnSource::Search {
            query: format!("q{i}"),
        })
        .collect();
    app.columns = crate::tui::columns::Columns::from_sources(&sources);
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('5'),
    ));
    for c in app.columns.items.clone() {
        app.handle_event(Event::Column {
            id: c.id,
            generation: c.generation,
            cursor: None,
            result: Ok(MorePage::Posts(posts(1).into())),
        });
    }
    app
}

fn render_buffer(app: &mut App, w: u16, h: u16) -> ratatui::buffer::Buffer {
    let mut images = Images::new(Picker::halfblocks(), None);
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| draw(f, app, &mut images)).unwrap();
    term.backend().buffer().clone()
}

/// Timeline posts the size real ones are: a few lines of mixed text, a
/// reply's context, pictures, and an avatar.
fn heavy_posts(n: usize) -> Vec<Post> {
    let text = "今日は山に登りました 🏔️ The view from the top was worth every step, \
                and the clouds rolled in just as we left 👨‍👩‍👧 #hiking https://example.com/trip \
                また行きたい！ @bob.test ";
    (0..n)
        .map(|i| {
            serde_json::from_value(json!({
                "uri": format!("at://did:plc:a/app.bsky.feed.post/{i}"), "cid": "c",
                "author": {"did": "did:plc:a", "handle": "alice.test", "displayName": "Alice 🌸",
                           "avatar": "http://127.0.0.1:9/avatar.jpg"},
                "record": {"text": text.repeat(1 + i % 3), "createdAt": "2026-09-22T00:00:00Z",
                           "reply": if i % 4 == 0 { json!({
                               "root": {"uri": "at://did:plc:b/app.bsky.feed.post/r", "cid": "c"},
                               "parent": {"uri": "at://did:plc:b/app.bsky.feed.post/r", "cid": "c"}
                           }) } else { json!(null) }},
                "embed": if i % 2 == 0 { json!({"$type": "app.bsky.embed.images#view", "images": [
                    {"thumb": format!("http://127.0.0.1:9/{i}a.jpg"), "fullsize": "x", "alt": "", "aspectRatio": {"width": 4, "height": 3}},
                    {"thumb": format!("http://127.0.0.1:9/{i}b.jpg"), "fullsize": "x", "alt": "", "aspectRatio": {"width": 3, "height": 4}}
                ]}) } else { json!(null) },
                "likeCount": i, "repostCount": 1, "replyCount": 2,
            }))
            .unwrap()
        })
        .collect()
}
