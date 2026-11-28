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

pub(super) use super::keys::HELP;

pub(super) fn draw(frame: &mut Frame, app: &App, images: &mut Images) {
    let area = frame.area();
    images.begin_frame(area.as_size(), app.theme.dim());
    frame.render_widget(Block::new().style(app.theme.base()), area);
    if area.width < 24 || area.height < 8 {
        frame.render_widget(Paragraph::new(app.t("Terminal too small")), area);
        return;
    }
    let message_lines = wrap(&app.message, usize::from(area.width));
    let status_height = (message_lines.len() as u16 + 1).clamp(2, (area.height / 3).max(2));
    let search_hints = super::keys::search_hints(app)
        .map(|hints| terminal_kit::view::hint_lines(&hints, area.width, &app.theme));
    let hint_height = search_hints
        .as_ref()
        .map_or(1, |lines| lines.len() as u16)
        .clamp(1, 3);
    let [tabs, body, hints, status] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(hint_height),
        Constraint::Length(status_height),
    ])
    .areas(area);
    let home = if app.searching() {
        format!(
            "1 {}",
            app.tf("Search: {}", &[&drawable(&app.search.query)])
        )
    } else {
        app.t("1 Home").into()
    };
    let labels = [
        (Tab::Home, home.as_str()),
        (Tab::Notifications, app.t("2 Notifications")),
        (Tab::Profile, app.t("3 Profile")),
        (Tab::Chat, app.t("4 Chat")),
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
    if let Some(lines) = search_hints {
        frame.render_widget(Paragraph::new(lines), hints);
    } else {
        frame.render_widget(
            Paragraph::new(super::keys::hint(app)).style(app.theme.dim()),
            hints,
        );
    }
    frame.render_widget(
        Paragraph::new(format!(
            "nstr · {} {}/{} · {}{}\n{}",
            app.t("relays"),
            app.answered,
            app.relays,
            app.public,
            if app.list().loading.is_some() {
                app.t(" · Loading…")
            } else {
                ""
            },
            message_lines.join("\n")
        ))
        .style(app.theme.dim()),
        status,
    );
    if let Some(browser) = &app.extra.browser {
        browser.clone().draw(
            frame,
            body,
            images,
            &app.theme,
            if app.language == "ja" {
                super::keys::browser_ja
            } else {
                |s| s
            },
        );
    } else if let Some(dialog) = &app.extra.dialog {
        if matches!(
            dialog,
            super::extras::Dialog::Input {
                purpose: super::extras::Purpose::Search,
                ..
            }
        ) {
            draw_lists(frame, body, app, images);
        }
        draw_dialog(frame, body, app, dialog);
    } else if app.tab == Tab::Chat && app.overlay.is_none() {
        let text = app
            .extra
            .messages
            .iter()
            .enumerate()
            .skip(app.extra.chat_selected)
            .map(|(i, row)| {
                let m = if row.get("lastMessage").is_some() {
                    &row["lastMessage"]
                } else {
                    row
                };
                format!(
                    "{} {} · {} · {}\n{}",
                    if i == app.extra.chat_selected {
                        "›"
                    } else {
                        " "
                    },
                    m["user"]["username"].as_str().unwrap_or(""),
                    m["createdAt"].as_str().unwrap_or(""),
                    row["unreadCount"].as_u64().unwrap_or(0),
                    m["text"].as_str().unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        frame.render_widget(
            Paragraph::new(drawable(&text)).block(
                Block::bordered().title(app.t("Chat · m recipient · i message · ctrl+s send")),
            ),
            body,
        );
    } else if let Some(overlay) = &app.overlay {
        draw_overlay(frame, body, app, overlay, images);
    } else {
        draw_lists(frame, body, app, images);
    }
}

fn draw_lists(frame: &mut Frame, body: Rect, app: &App, images: &mut Images) {
    if app.tab == Tab::Home
        && app.threads.is_empty()
        && let Some(focus) = app.extra.column
    {
        for (i, area) in terminal_kit::columns::visible(body, app.extra.columns.len(), focus, 40) {
            frame.render_widget(
                Paragraph::new(drawable(&app.extra.columns[i])).style(if i == focus {
                    app.theme.selected()
                } else {
                    app.theme.dim()
                }),
                Rect {
                    height: 1.min(area.height),
                    ..area
                },
            );
            draw_notes(
                frame,
                Rect {
                    y: area.y + 1,
                    height: area.height.saturating_sub(1),
                    ..area
                },
                app,
                images,
                app.extra.column_lists.get(i),
            );
        }
    } else {
        draw_notes(frame, body, app, images, None);
    }
}

fn draw_notes(
    frame: &mut Frame,
    mut area: Rect,
    app: &App,
    images: &mut Images,
    list: Option<&super::app::List>,
) {
    let unread = app.lists[1]
        .notes
        .iter()
        .filter(|n| !app.extra.seen.contains(&n.id))
        .count();
    let summary = format!(
        "{}: {unread} · {}",
        app.t("Unread"),
        app.extra.columns.join(" | ")
    );
    let title = if app.searching() && app.threads.is_empty() {
        ""
    } else if app.threads.is_empty() {
        summary.as_str()
    } else {
        app.t("Thread")
    };
    let block = Block::default().borders(Borders::TOP).title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    area = inner;
    if app.tab == Tab::Profile && app.threads.is_empty() {
        let value: serde_json::Value = serde_json::from_str(&app.profile).unwrap_or_default();
        if images.shows() && area.height >= 10 {
            let height = (area.height / 4).clamp(3, 6);
            for (field, x, width) in [
                ("picture", area.x, 12.min(area.width)),
                (
                    "banner",
                    area.x + 12.min(area.width),
                    area.width.saturating_sub(12),
                ),
            ] {
                if width > 0
                    && let Some(url) = value[field]
                        .as_str()
                        .filter(|u| crate::api::media::valid_url(u))
                {
                    images.draw(frame, Rect::new(x, area.y, width, height), url);
                }
            }
            area.y += height;
            area.height -= height;
        }
        let mut text = ["display_name", "name", "about"]
            .iter()
            .filter_map(|key| value[*key].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if let Some((author, follow, mute)) = &app.extra.relationship
            && app.extra.author.as_ref() == Some(author)
        {
            text.push_str(&format!(
                "\n{}: {} · {}: {}",
                app.t("Following"),
                if *follow { app.t("yes") } else { app.t("no") },
                app.t("Muted"),
                if *mute { app.t("yes") } else { app.t("no") }
            ));
        }
        let lines = wrap(&text, usize::from(area.width));
        let height = (lines.len() as u16).min(area.height / 2);
        frame.render_widget(
            Paragraph::new(lines.join("\n")).style(app.theme.accent()),
            Rect { height, ..area },
        );
        area.y += height;
        area.height -= height;
    }
    let list = list.unwrap_or_else(|| app.list());
    if list.notes.is_empty() {
        frame.render_widget(
            Paragraph::new(if app.searching() && app.threads.is_empty() {
                app.tf("No results for \"{}\"", &[&drawable(&app.search.query)])
            } else {
                app.t("No notes. R refreshes.").into()
            }),
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
        let metadata: serde_json::Value = app
            .extra
            .metadata
            .get(&note.pubkey)
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let author = metadata["display_name"]
            .as_str()
            .or_else(|| metadata["name"].as_str())
            .unwrap_or(&note.pubkey[..note.pubkey.len().min(12)]);
        let hidden = app.hidden(note);
        let embedded = crate::api::actions::reposted(note);
        let shown = embedded.as_ref().unwrap_or(note);
        let mut lines = vec![format!(
            "{} {} · {}",
            if selected { "›" } else { " " },
            author,
            timestamp
        )];
        if embedded.is_some() {
            lines.push(format!("⟳ {}", &shown.pubkey[..12]));
        }
        let mut emoji_urls = Vec::new();
        let content = if hidden {
            format!(
                "{}: {} · w {}",
                app.t("Content warning"),
                shown.tag_values("content-warning").next().unwrap_or(""),
                app.t("open")
            )
        } else if matches!(note.kind, 6 | 16) && embedded.is_none() {
            app.t("Repost (original unavailable)").into()
        } else if images.shows() {
            terminal_kit::emoji::with_emoji_cells(
                &shown.content,
                &mut |name| {
                    shown
                        .tags
                        .iter()
                        .find(|t| {
                            t.first().is_some_and(|s| s == "emoji")
                                && t.get(1).is_some_and(|s| s == name)
                        })
                        .and_then(|t| t.get(2))
                        .filter(|u| crate::api::media::valid_url(u))
                        .cloned()
                },
                &mut emoji_urls,
            )
        } else {
            shown.content.clone()
        };
        lines.extend(wrap(&content, usize::from(area.width.saturating_sub(2))));
        if !hidden && let Some(id) = shown.tag_values("q").next() {
            lines.push(format!(
                "↪ {} · V {}",
                &id[..id.len().min(12)],
                app.t("open quote")
            ));
            if let Some(quote) = app.extra.references.get(id) {
                let body = if app.hidden(quote) {
                    app.t("Content warning · V to open quote")
                } else {
                    &quote.content
                };
                lines.extend(
                    wrap(body, usize::from(area.width.saturating_sub(4)))
                        .into_iter()
                        .take(4)
                        .map(|s| format!("│ {s}")),
                );
            }
        }
        let liked = app.marks.contains(&(note.id.clone(), 7));
        let reposted = app.marks.contains(&(note.id.clone(), 6));
        lines.push(format!(
            "{}   {}",
            if liked { "♥ +" } else { "♡" },
            if reposted { "⟳ +" } else { "⟳" }
        ));
        let urls = if hidden { vec![] } else { note::pictures(note) };
        if !urls.is_empty() {
            lines.push(format!("▣ {} {}", urls.len(), app.t("pictures")));
        }
        if !hidden && note::has_video(note) {
            lines.push(app.t("▶ video · space to play").into());
        }
        let height = (lines.len() as u16).min(area.height);
        frame.render_widget(
            Paragraph::new(lines.join("\n")).style(style),
            Rect { height, ..area },
        );
        terminal_kit::emoji::draw_emoji_cells(frame, Rect { height, ..area }, &emoji_urls, images);
        area.y += height;
        area.height -= height;
        if images.shows() && !urls.is_empty() && area.height >= 4 {
            let height = area.height.min(6);
            let width = (area.width / urls.len().min(4) as u16).max(1);
            for (i, url) in urls.iter().filter(|u| !note::video(u)).take(4).enumerate() {
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
    let title = match overlay {
        Overlay::Help(_) => app.t("Help"),
        Overlay::Compose {
            target: Some(_), ..
        } => app.t("Reply"),
        Overlay::Compose { .. } => app.t("New note"),
        Overlay::Settings(_) => app.t("Settings"),
        Overlay::Themes { .. } => app.t("Themes"),
        Overlay::Languages(_) => app.t("Language"),
        Overlay::Pictures { .. } => app.t("Pictures"),
    };
    let block = Block::bordered().title(title).style(app.theme.base());
    let inner = terminal_kit::view::draw_panel(frame, area, block, None);
    match overlay {
        Overlay::Compose { text, .. } => {
            frame.render_widget(
                Paragraph::new(format!(
                    "{} {} · CW: {} · ctrl+o {} · tab alt",
                    app.extra.attachments.len(),
                    app.t("attachments"),
                    app.extra.cw.as_deref().unwrap_or(""),
                    app.t("attach")
                )),
                Rect {
                    height: 1.min(inner.height),
                    ..inner
                },
            );
            let inner = Rect {
                y: inner.y.saturating_add(1),
                height: inner.height.saturating_sub(1),
                ..inner
            };
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
                .map(|(key, en)| format!("{key:<18} {}", app.t(en)))
                .collect::<Vec<_>>()
                .join("\n");
            frame.render_widget(Paragraph::new(text), inner);
        }
        Overlay::Settings(selected) => menu(
            frame,
            inner,
            app,
            &[app.t("Theme"), app.t("Language")],
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
                let area = Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1);
                if super::note::video(url)
                    || app.selected().is_some_and(|e| {
                        crate::api::media::files(e)
                            .iter()
                            .any(|f| f["url"] == *url && f["type"] == "video")
                    })
                {
                    let state = images.draw_video(frame, area, url, app.extra.replay);
                    if let terminal_kit::media::player::State::Warning(w) = state {
                        frame.render_widget(Paragraph::new(drawable(&w)), area);
                    }
                } else {
                    images.draw(frame, area, url);
                }
            }
        }
    }
}

fn menu(frame: &mut Frame, area: Rect, app: &App, choices: &[&str], selected: usize) {
    let first = terminal_kit::view::first_visible(selected, area.height);
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

fn draw_dialog(frame: &mut Frame, area: Rect, app: &App, dialog: &super::extras::Dialog) {
    use super::{
        extras::{Dialog, Purpose},
        keys::ACTIONS,
    };
    if let Dialog::Input {
        purpose: Purpose::Search,
        text,
    } = dialog
    {
        terminal_kit::view::draw_search_box(
            frame,
            area,
            text,
            None,
            terminal_kit::i18n::Lang::from_code(app.language)
                .unwrap_or(terminal_kit::i18n::Lang::En),
            &app.theme,
        );
        return;
    }
    let (title, text) = match dialog {
        Dialog::Profile(editor) => (
            app.t("Edit profile · tab field · ctrl+o picture · ctrl+s save"),
            format!(
                "{}\n{}",
                editor.summary(app.language == "ja"),
                app.extra
                    .profile_files
                    .iter()
                    .map(|(field, path)| format!(
                        "{field}: {}",
                        path.file_name()
                            .unwrap_or(path.as_os_str())
                            .to_string_lossy()
                    ))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
        ),
        Dialog::Confirm { .. } => (
            app.t("Confirm · y yes / n no · esc cancel"),
            app.t("Publish this request? Relays decide what they honor.")
                .to_owned(),
        ),
        Dialog::Input { purpose, text } => {
            let title = match purpose {
                Purpose::AddAccount => app.t("Add account: name JSON · enter"),
                Purpose::Search => app.t("Search"),
                Purpose::ChatPeer => app.t("Recipient public key · enter"),
                Purpose::ChatMessage => {
                    app.t("Private message · ctrl+s")
                }
                Purpose::Reaction => app.t("Reaction · enter"),
                Purpose::Report => app.t("Report: reason comment · enter"),
                Purpose::Warning => app.t("Content warning · enter"),
                Purpose::Alt => app.t("Attachment alt text · tab next · shift+tab previous · ctrl+x remove · enter done"),
            };
            let body = if matches!(purpose, Purpose::Alt) {
                format!(
                    "{}/{} {}\n{}",
                    app.extra.attachment + 1,
                    app.extra.attachments.len(),
                    app.extra
                        .attachments
                        .get(app.extra.attachment)
                        .and_then(|(p, _)| p.file_name())
                        .unwrap_or_default()
                        .to_string_lossy(),
                    text.text()
                )
            } else {
                text.text()
            };
            (title, body)
        }
        Dialog::Actions(selected) => (
            app.t("Actions"),
            ACTIONS
                .iter()
                .enumerate()
                .map(|(i, (_, en))| {
                    format!("{} {}", if i == *selected { "›" } else { " " }, app.t(en))
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        Dialog::Accounts { names, selected } => (
            app.t("Saved accounts · enter switch · a add · x remove"),
            names
                .iter()
                .enumerate()
                .map(|(i, n)| format!("{} {n}", if i == *selected { "›" } else { " " }))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
    };
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(drawable(&text)).block(Block::bordered().title(title)),
        area,
    );
}
