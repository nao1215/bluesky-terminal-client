//! Search results, profiles and the full-screen media viewer.
use super::*;

/// A single input line over the current list, near its top edge.
pub(super) fn search_box(frame: &mut Frame, area: Rect, app: &App, th: &Theme) {
    terminal_kit::view::draw_search_box(
        frame,
        area,
        &app.search.input,
        Some(app.search.accounts),
        crate::i18n::current(),
        th,
    );
}

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
        .chain(
            s.card
                .as_ref()
                .and_then(|c| c.image.as_deref())
                .filter(|u| !u.is_empty()),
        )
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
    if app.search_results() {
        let body = area;
        let empty = tf("No results for \"{}\"", &[&app.search.query]);
        if !app.search.result_accounts {
            draw_statuses(frame, body, &mut app.search.posts, &empty, ctx, images);
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
                        &empty
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
                    let mut urls = Vec::new();
                    let text = format!(
                        "{} {}\n{}",
                        emoji_text(&display_name(a), &a.emojis, images.shows(), &mut urls),
                        acct(a),
                        emoji_text(
                            &crate::html::to_text(&a.note),
                            &a.emojis,
                            images.shows(),
                            &mut urls
                        )
                    );
                    frame.render_widget(
                        wrapped(&text, row.width.saturating_sub(3)),
                        content_rect(row, 0),
                    );
                    terminal_kit::emoji::draw_emoji_cells(
                        frame,
                        content_rect(row, 0),
                        &urls,
                        images,
                    );
                }
            }
        }
    } else {
        if app.profile.account.is_none() && app.session.is_none() {
            draw_needs_login(frame, area, app.host(), &ctx.t);
            return;
        }
        let mut urls = Vec::new();
        let mut header = match &app.profile.account {
            Some(a) => format!(
                "{} {}\n{}\n{}",
                emoji_text(&display_name(a), &a.emojis, images.shows(), &mut urls),
                acct(a),
                emoji_text(
                    &crate::html::to_text(&a.note),
                    &a.emojis,
                    images.shows(),
                    &mut urls
                ),
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
        if let Some(r) = app
            .profile
            .account
            .as_ref()
            .and_then(|a| app.relationships.get(&a.id))
        {
            header.push('\n');
            header.push_str(&App::relationship_label(r));
        }
        let lines = wrap(&header, usize::from(area.width.max(1)));
        let h = (lines.len() as u16 + 1).min(area.height / 2);
        let [head, body] =
            Layout::vertical([Constraint::Length(h), Constraint::Min(1)]).areas(area);
        frame.render_widget(
            Paragraph::new(lines.into_iter().map(Line::from).collect::<Vec<_>>()),
            head,
        );
        terminal_kit::emoji::draw_emoji_cells(frame, head, &urls, images);
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
