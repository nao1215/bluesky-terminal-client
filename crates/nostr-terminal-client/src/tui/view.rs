//! Layout and bilingual UI text. Relay text is sanitized before drawing.
use super::{
    app::{App, Overlay, Tab},
    note,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use terminal_kit::{
    media::images::Images,
    text::{drawable, wrap},
    theme::THEMES,
};

pub(super) const HELP: &[(&str, &str, &str)] = &[
    (
        "1 2 3 / tab",
        "Home / Notifications / Profile",
        "ホーム / 通知 / プロフィール",
    ),
    ("j k / ↓ ↑", "move the selection", "選択を移動"),
    ("g G / home end", "first / last note", "最初 / 最後のノート"),
    (
        "pgdn pgup",
        "move five notes; more loads near the end",
        "5件移動・末尾近くで続きを取得",
    ),
    ("R F5", "refresh", "再読み込み"),
    ("v / esc", "open thread / back", "スレッド / 戻る"),
    ("n / r", "new note / reply", "新規ノート / 返信"),
    ("ctrl+s", "send from composer", "投稿画面から送信"),
    (
        "ctrl+u / esc",
        "clear line / cancel composer",
        "行頭まで削除 / 投稿を中止",
    ),
    ("l", "send a + reaction", "＋リアクションを送信"),
    ("b", "repost", "リポスト"),
    (
        "c",
        "copy nostr:nevent identifier",
        "nostr:nevent 識別子をコピー",
    ),
    ("space", "open pictures", "画像を開く"),
    (
        "h l / ← →",
        "previous / next picture in viewer",
        "画像ビューアで前 / 次の画像",
    ),
    ("s / T", "settings / themes", "設定 / テーマ"),
    (
        "j k / enter",
        "choose a setting or theme",
        "設定やテーマを選択・決定",
    ),
    ("x", "reset the selected setting", "選択中の設定を初期化"),
    ("? / esc", "help / close", "ヘルプ / 閉じる"),
    ("q ctrl+c", "quit", "終了"),
];

pub(super) fn draw(frame: &mut Frame, app: &App, images: &mut Images) {
    let area = frame.area();
    images.begin_frame(area.as_size(), app.theme.dim());
    frame.render_widget(Block::new().style(app.theme.base()), area);
    if area.width < 24 || area.height < 8 {
        frame.render_widget(
            Paragraph::new(app.t("Terminal too small", "端末が小さすぎます")),
            area,
        );
        return;
    }
    let message_lines = wrap(&app.message, usize::from(area.width));
    let status_height = (message_lines.len() as u16 + 1).clamp(2, (area.height / 3).max(2));
    let [tabs, body, hints, status] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(status_height),
    ])
    .areas(area);
    let labels = [
        (Tab::Home, app.t("1 Home", "1 ホーム")),
        (Tab::Notifications, app.t("2 Notifications", "2 通知")),
        (Tab::Profile, app.t("3 Profile", "3 プロフィール")),
    ];
    let spans = labels
        .into_iter()
        .flat_map(|(tab, label)| {
            [
                Span::styled(
                    label,
                    if tab == app.tab {
                        app.theme.selected()
                    } else {
                        app.theme.dim()
                    },
                ),
                Span::raw("   "),
            ]
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(Line::from(spans)), tabs);
    let hint = match app.overlay {
        Some(Overlay::Compose { .. }) => app.t("ctrl+s send   esc cancel", "ctrl+s 送信   esc 中止"),
        Some(Overlay::Themes { .. }) => app.t("j k preview   enter apply   esc cancel", "j k プレビュー   enter 適用   esc 中止"),
        Some(Overlay::Settings(_)) => app.t("j k move   enter choose   x reset   esc close", "j k 移動   enter 選択   x 初期化   esc 閉じる"),
        Some(Overlay::Languages(_)) => app.t("j k move   enter choose   esc close", "j k 移動   enter 選択   esc 閉じる"),
        Some(Overlay::Pictures { .. }) => app.t("h l previous / next   esc close", "h l 前 / 次   esc 閉じる"),
        Some(Overlay::Help(_)) => app.t("j k scroll   esc close", "j k スクロール   esc 閉じる"),
        None => app.t("? help   j k move   tab next tab   n post   r reply   l like   b repost   v thread   c copy   s settings   T themes   q quit", "? ヘルプ   j k 移動   tab タブ   n 投稿   r 返信   l リアクション   b リポスト   v スレッド   c コピー   s 設定   T テーマ   q 終了"),
    };
    frame.render_widget(Paragraph::new(hint).style(app.theme.dim()), hints);
    frame.render_widget(
        Paragraph::new(format!(
            "nstr · {} {}/{} · {}{}\n{}",
            app.t("relays", "リレー"),
            app.answered,
            app.relays,
            app.public,
            if app.list().loading.is_some() {
                app.t(" · Loading…", " · 読み込み中…")
            } else {
                ""
            },
            message_lines.join("\n")
        ))
        .style(app.theme.dim()),
        status,
    );
    if let Some(overlay) = &app.overlay {
        draw_overlay(frame, body, app, overlay, images);
    } else {
        draw_notes(frame, body, app, images);
    }
}

fn draw_notes(frame: &mut Frame, mut area: Rect, app: &App, images: &mut Images) {
    let title = if app.threads.is_empty() {
        ""
    } else {
        app.t("Thread", "スレッド")
    };
    let block = Block::default().borders(Borders::TOP).title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    area = inner;
    if app.tab == Tab::Profile && app.threads.is_empty() {
        let value: serde_json::Value = serde_json::from_str(&app.profile).unwrap_or_default();
        let text = ["display_name", "name", "about"]
            .iter()
            .filter_map(|key| value[*key].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let lines = wrap(&text, usize::from(area.width));
        let height = (lines.len() as u16).min(area.height / 2);
        frame.render_widget(
            Paragraph::new(lines.join("\n")).style(app.theme.accent()),
            Rect { height, ..area },
        );
        area.y += height;
        area.height -= height;
    }
    let list = app.list();
    if list.notes.is_empty() {
        frame.render_widget(
            Paragraph::new(app.t(
                "No notes. R refreshes.",
                "ノートはありません。R で再読み込み。",
            )),
            area,
        );
    }
    // Keep the selection visible even when the preceding note fills the screen.
    let first = list.selected;
    for (index, note) in list.notes.iter().enumerate().skip(first) {
        if area.height < 3 {
            break;
        }
        let selected = index == list.selected;
        let style = if selected {
            app.theme.selected()
        } else {
            app.theme.base()
        };
        let timestamp = i64::try_from(note.created_at)
            .ok()
            .and_then(|s| jiff::Timestamp::from_second(s).ok())
            .map(|t| terminal_kit::text::format_time(&t.to_string()))
            .unwrap_or_default();
        let author = &note.pubkey[..note.pubkey.len().min(12)];
        let mut lines = vec![format!(
            "{} {} · {}",
            if selected { "›" } else { " " },
            author,
            timestamp
        )];
        lines.extend(wrap(
            &note.content,
            usize::from(area.width.saturating_sub(2)),
        ));
        let liked = app.marks.contains(&(note.id.clone(), 7));
        let reposted = app.marks.contains(&(note.id.clone(), 6));
        lines.push(format!(
            "{}   {}",
            if liked { "♥ +" } else { "♡" },
            if reposted { "⟳ +" } else { "⟳" }
        ));
        let urls = note::pictures(note);
        if !urls.is_empty() {
            lines.push(format!("▣ {} {}", urls.len(), app.t("pictures", "画像")));
        }
        let height = (lines.len() as u16).min(area.height);
        frame.render_widget(
            Paragraph::new(lines.join("\n")).style(style),
            Rect { height, ..area },
        );
        area.y += height;
        area.height -= height;
        if images.shows() && !urls.is_empty() && area.height >= 4 {
            let height = area.height.min(6);
            let width = (area.width / urls.len().min(4) as u16).max(1);
            for (i, url) in urls.iter().take(4).enumerate() {
                images.draw(
                    frame,
                    Rect::new(area.x + i as u16 * width, area.y, width, height),
                    url,
                );
            }
            area.y += height;
            area.height -= height;
        }
        if area.height > 0 {
            area.y += 1;
            area.height -= 1;
        }
    }
}

fn draw_overlay(frame: &mut Frame, area: Rect, app: &App, overlay: &Overlay, images: &mut Images) {
    frame.render_widget(Clear, area);
    let title = match overlay {
        Overlay::Help(_) => app.t("Help", "ヘルプ"),
        Overlay::Compose {
            target: Some(_), ..
        } => app.t("Reply", "返信"),
        Overlay::Compose { .. } => app.t("New note", "新規ノート"),
        Overlay::Settings(_) => app.t("Settings", "設定"),
        Overlay::Themes { .. } => app.t("Themes", "テーマ"),
        Overlay::Languages(_) => app.t("Language", "言語"),
        Overlay::Pictures { .. } => app.t("Pictures", "画像"),
    };
    let block = Block::bordered().title(title).style(app.theme.base());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    match overlay {
        Overlay::Compose { text, .. } => {
            let layout = text.layout(usize::from(inner.width));
            let offset = layout
                .cursor
                .0
                .saturating_sub(usize::from(inner.height.saturating_sub(1)));
            frame.render_widget(
                Paragraph::new(
                    layout
                        .lines
                        .into_iter()
                        .skip(offset)
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                inner,
            );
            if inner.width > 0 && inner.height > 0 {
                frame.set_cursor_position((
                    inner.x + (layout.cursor.1 as u16).min(inner.width - 1),
                    inner.y + (layout.cursor.0 - offset) as u16,
                ));
            }
        }
        Overlay::Help(scroll) => {
            let text = HELP
                .iter()
                .skip(*scroll)
                .map(|(key, en, ja)| format!("{key:<18} {}", app.t(en, ja)))
                .collect::<Vec<_>>()
                .join("\n");
            frame.render_widget(Paragraph::new(text), inner);
        }
        Overlay::Settings(selected) => menu(
            frame,
            inner,
            app,
            &[app.t("Theme", "テーマ"), app.t("Language", "言語")],
            *selected,
        ),
        Overlay::Themes { selected, .. } => menu(
            frame,
            inner,
            app,
            &THEMES.iter().map(|t| t.name).collect::<Vec<_>>(),
            *selected,
        ),
        Overlay::Languages(selected) => menu(frame, inner, app, &["English", "日本語"], *selected),
        Overlay::Pictures { urls, selected } => {
            let url = &urls[*selected];
            frame.render_widget(
                Paragraph::new(drawable(url)),
                Rect {
                    height: 1.min(inner.height),
                    ..inner
                },
            );
            if inner.height > 1 {
                images.draw(
                    frame,
                    Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1),
                    url,
                );
            }
        }
    }
}

fn menu(frame: &mut Frame, area: Rect, app: &App, choices: &[&str], selected: usize) {
    let first = selected.saturating_sub(usize::from(area.height.saturating_sub(1)));
    let lines = choices
        .iter()
        .enumerate()
        .skip(first)
        .map(|(i, text)| {
            Line::styled(
                format!("{} {text}", if i == selected { "›" } else { " " }),
                if i == selected {
                    app.theme.selected()
                } else {
                    app.theme.base()
                },
            )
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), area);
}
