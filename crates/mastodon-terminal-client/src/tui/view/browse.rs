//! Search results, profiles and the full-screen media viewer.
use super::*;

pub(super) fn thumbnails<'a>(s: &'a Status, ctx: &Ctx, pictures: bool) -> Vec<&'a str> {
    if !pictures || (!s.spoiler_text.is_empty() && !ctx.opened.contains(&s.id)) {
        return Vec::new();
    }
    s.media_attachments
        .iter()
        .filter_map(|m| match m.kind.as_str() {
            "image" => m
                .preview_url
                .as_deref()
                .filter(|u| !u.is_empty())
                .or(m.source()),
            "video" | "gifv" => m.preview_url.as_deref().filter(|u| !u.is_empty()),
            _ => None,
        })
        .take(4)
        .collect()
}

pub(super) fn draw_browse(
    frame: &mut Frame,
    area: Rect,
    app: &mut App,
    images: &mut Images,
    ctx: &Ctx,
) {
    if app.tab == Tab::Search {
        let [query, body] =
            Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).areas(area);
        let title = format!(
            " {} · {} ",
            t("Search"),
            t(if app.search.accounts {
                n!("Accounts")
            } else {
                n!("Posts")
            })
        );
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(title)
            .border_style(if app.search.editing {
                ctx.t.accent()
            } else {
                ctx.t.dim()
            });
        let inner = block.inner(query);
        frame.render_widget(block, query);
        let input = app.search.input.layout(usize::from(inner.width.max(1)));
        let line = input.lines.get(input.cursor.0).cloned().unwrap_or_default();
        frame.render_widget(Paragraph::new(line), inner);
        if app.search.editing {
            frame.set_cursor_position((inner.x + input.cursor.1 as u16, inner.y));
        }
        if !app.search.accounts {
            if !app.search.posts.loaded && app.search.posts.waiting.is_none() {
                frame.render_widget(
                    wrapped(t("type a query and press enter"), body.width).style(ctx.t.dim()),
                    body,
                );
            } else {
                draw_statuses(
                    frame,
                    body,
                    &mut app.search.posts,
                    n!("No posts found."),
                    ctx,
                    images,
                );
            }
        } else {
            let list = &mut app.search.users;
            if list.items.is_empty() {
                empty_message(
                    frame,
                    body,
                    list,
                    if list.waiting.is_some() {
                        n!("loading…")
                    } else {
                        n!("No accounts found.")
                    },
                    &ctx.t,
                );
            } else {
                list.offset = scroll_offset(
                    list.selected,
                    list.offset,
                    list.items.len(),
                    body.height,
                    |_| 3,
                );
                for (i, a) in list
                    .items
                    .iter()
                    .enumerate()
                    .skip(list.offset)
                    .take(usize::from(body.height.div_ceil(3)))
                {
                    let row = Rect::new(
                        body.x,
                        body.y + (i - list.offset) as u16 * 3,
                        body.width,
                        3.min(body.bottom() - (body.y + (i - list.offset) as u16 * 3)),
                    );
                    draw_marker(frame, row, 3, i == list.selected, &ctx.t);
                    let text = format!(
                        "{} {}\n{}",
                        display_name(a),
                        acct(a),
                        crate::html::to_text(&a.note)
                    );
                    frame.render_widget(
                        wrapped(&text, row.width.saturating_sub(3)),
                        content_rect(row, 0),
                    );
                }
            }
        }
    } else {
        if app.profile.account.is_none() && app.session.is_none() {
            draw_needs_login(frame, area, app.host(), &ctx.t);
            return;
        }
        let header = match &app.profile.account {
            Some(a) => format!(
                "{} {}\n{}\n{}",
                display_name(a),
                acct(a),
                crate::html::to_text(&a.note),
                tf(
                    "{} followers · {} following · {} posts",
                    &[
                        &a.followers_count.unwrap_or(0).to_string(),
                        &a.following_count.unwrap_or(0).to_string(),
                        &a.statuses_count.unwrap_or(0).to_string()
                    ]
                )
            ),
            None => t("loading…").into(),
        };
        let lines = wrap(&header, usize::from(area.width.max(1)));
        let h = (lines.len() as u16 + 1).min(area.height / 2);
        let [head, body] =
            Layout::vertical([Constraint::Length(h), Constraint::Min(1)]).areas(area);
        frame.render_widget(
            Paragraph::new(lines.into_iter().map(Line::from).collect::<Vec<_>>()),
            head,
        );
        draw_statuses(
            frame,
            body,
            &mut app.profile.posts,
            n!("No posts yet. Press R to refresh."),
            ctx,
            images,
        );
    }
}

pub(super) fn draw_viewer(
    frame: &mut Frame,
    area: Rect,
    files: &[Media],
    index: usize,
    generation: u32,
    images: &mut Images,
    th: &Theme,
) {
    frame.render_widget(Clear, area);
    frame.render_widget(Block::new().style(th.base()), area);
    let Some(file) = files.get(index) else {
        return;
    };
    let [picture, caption] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).areas(area);
    let mut state = String::new();
    if !images.shows() {
        state = t("this terminal cannot show pictures or video; d saves files, o opens the post")
            .into();
    } else if let Some(url) = file.source() {
        if file.kind == "image" {
            images.stop_video();
            images.draw_first(
                frame,
                picture,
                &[url, file.preview_url.as_deref().unwrap_or("")],
            );
        } else {
            use terminal_kit::media::player::State;
            state = match images.draw_video(frame, picture, url, generation) {
                State::Loading => t("loading the video…").into(),
                State::Playing => t("playing (no sound)").into(),
                State::Ended => t("ended  r plays it again").into(),
                State::Warning(why) => format!(
                    "{}: {}",
                    t("cannot play this video; d saves it, o opens it"),
                    drawable(&why)
                ),
            };
            if !images.video_shown()
                && let Some(url) = &file.preview_url
            {
                images.draw(frame, picture, url);
            }
        }
    } else {
        state = t("the file has no address").into();
    }
    let text = format!(
        "{}/{} · {}\n{}\n{}",
        index + 1,
        files.len(),
        file.kind,
        file.description.as_deref().unwrap_or(t("(no alt text)")),
        state
    );
    frame.render_widget(wrapped(&text, caption.width), caption);
}
