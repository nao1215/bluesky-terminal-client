//! The composer, its attachments, the file browser, and the profile editor.

use super::*;

pub(super) fn draw_compose(
    frame: &mut Frame,
    area: Rect,
    c: &Compose,
    images: &mut Images,
    t: &Theme,
) {
    let (title, quoted) = match (&c.reply, &c.quote) {
        (Some((_, handle, excerpt)), _) => (i18n::tf("Reply to @{}", &[handle]), Some(excerpt)),
        (None, Some((_, handle, excerpt))) => (i18n::tf("Quote @{}", &[handle]), Some(excerpt)),
        (None, None) => (i18n::t("New post").to_string(), None),
    };
    let n = c.media.len() as u16;
    // A row of thumbnails, then a line per picture for its alt text.
    let pics_h = if n > 0 { THUMB.1 + n } else { 0 };
    let inner = popup(frame, area, 72, 14 + pics_h, &title, t);
    let quote_h = if quoted.is_some() { 2 } else { 0 };
    // On a box too short for all of it, the pictures take everything left
    // after the quote, one row of text, and the footer. Asking for more
    // than the box has left them nothing at all, so what the post would
    // send was not on the screen anywhere.
    let pics_h = pics_h.min(inner.height.saturating_sub(quote_h + 2));
    let [quote, text, pics, foot] = Layout::vertical([
        Constraint::Length(quote_h),
        Constraint::Min(1),
        Constraint::Length(pics_h),
        Constraint::Length(1),
    ])
    .areas(inner);
    if let Some(excerpt) = quoted {
        frame.render_widget(
            Paragraph::new(truncate(&format!("❝ {excerpt}"), usize::from(quote.width)))
                .style(t.dim()),
            quote,
        );
    }
    let typing = !c.sending && c.browser.is_none();
    let text_area = Rect {
        x: text.x + 1,
        width: text.width.saturating_sub(2),
        ..text
    };
    draw_multi_input(frame, text_area, &c.input, typing && c.focus == 0);
    if n > 0 {
        draw_attachments(frame, pics, c, images, typing, t);
    }
    let text = c.input.text();
    let text = text.trim_end();
    let len = grapheme_len(text);
    let count_style = if post_length_problem(text).is_some() {
        t.error()
    } else {
        t.dim()
    };
    // Bytes are shown only once they are what limits the post.
    let bytes = if text.len() > MAX_POST_BYTES - 500 {
        format!(
            " {}",
            i18n::tf("{} bytes", &[&format!("{}/{MAX_POST_BYTES}", text.len())])
        )
    } else {
        String::new()
    };
    let action = i18n::t(if c.sending {
        n!("sending…")
    } else if n > 0 {
        n!("ctrl+s send  ctrl+o attach  tab alt text  ctrl+x remove  esc cancel")
    } else {
        n!("ctrl+s send  ctrl+o attach pictures or a video  esc cancel")
    });
    let count = format!(" {len}/{MAX_POST_GRAPHEMES}{bytes}  ");
    let room = usize::from(foot.width).saturating_sub(crate::tui::text::cells(&count));
    frame.render_widget(
        truncate_line(
            Line::from(vec![
                Span::styled(count, count_style),
                Span::styled(crate::tui::text::fit_hints(action, room), t.dim()),
            ]),
            usize::from(foot.width),
        ),
        foot,
    );
}

/// The composer's pictures: thumbnails in a row, then each one's number,
/// file name, and alt text.
pub(super) fn draw_attachments(
    frame: &mut Frame,
    area: Rect,
    c: &Compose,
    images: &mut Images,
    typing: bool,
    t: &Theme,
) {
    let n = c.media.len() as u16;
    // Room for a strip of thumbnails above a line per picture.
    let room = area.height >= THUMB.1 + n;
    // No thumbnails behind the browser (they would only cost encodes), but
    // their rows stay where they are, so the list does not jump when the
    // browser opens and closes.
    let thumbs = c.browser.is_none() && room;
    // Where there is no room the strip is not kept empty either: the names
    // start at the top of the area instead of below a strip that is not
    // drawn, which put them past the bottom of the box.
    let top = if room { THUMB.1 } else { 0 };
    let rows = area.height.saturating_sub(top);
    // Fewer rows than pictures: the last row counts the ones left out, so
    // the post never carries a picture the screen does not mention.
    let shown = if n > rows { rows.saturating_sub(1) } else { n };
    for (i, a) in c.media.iter().enumerate().take(usize::from(shown)) {
        let i16 = i as u16;
        let x = area.x + 1 + i16 * (THUMB.0 + 1);
        if thumbs && x + THUMB.0 <= area.right() {
            let r = Rect {
                x,
                y: area.y,
                width: THUMB.0,
                height: THUMB.1,
            };
            draw_media_box(frame, r, &a.path, a.info, images, t);
        }
        let row = Rect {
            y: area.y + top + i16,
            height: 1,
            ..area
        };
        if row.y >= area.bottom() {
            break;
        }
        let focused = typing && c.focus == i + 1;
        let name = a
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let what = match (a.info.animated_gif, a.info.seconds) {
            (true, _) => format!(" ({})", i18n::t("GIF, posted as a video")),
            (false, Some(s)) if a.info.kind == media::Kind::Video => {
                format!(" ({} {})", i18n::t("video"), format_seconds(s))
            }
            (false, None) if a.info.kind == media::Kind::Video => {
                format!(" ({})", i18n::t("video"))
            }
            _ => String::new(),
        };
        let label = format!(
            " {} {}{what}  {} ",
            i + 1,
            truncate(&name, 20),
            i18n::t("alt:")
        );
        let label_w = crate::tui::text::cells(&label) as u16;
        let style = if focused { t.accent().bold() } else { t.dim() };
        frame.render_widget(Paragraph::new(label).style(style), row);
        let field = Rect {
            x: row.x + label_w.min(row.width),
            width: row.width.saturating_sub(label_w + 1),
            ..row
        };
        if a.alt.is_empty() && !focused {
            frame.render_widget(
                Paragraph::new(i18n::t("(none; tab to describe it)")).style(t.dim()),
                field,
            );
        } else {
            draw_single_input(frame, field, &a.alt, focused);
        }
    }
    if n > shown {
        frame.render_widget(
            Paragraph::new(format!(
                " {}",
                i18n::tf("and {} more", &[&(n - shown).to_string()])
            ))
            .style(t.dim()),
            Rect {
                y: area.y + top + shown,
                height: 1,
                ..area
            },
        );
    }
}

/// A picture drawn in its box. A video attached from disk is not decoded
/// here, so its box says what it is and how long it runs; an animated GIF
/// shows its first frame.
pub(super) fn draw_media_box(
    frame: &mut Frame,
    area: Rect,
    path: &std::path::Path,
    info: media::Info,
    images: &mut Images,
    t: &Theme,
) {
    if info.kind == media::Kind::Picture || info.animated_gif {
        images.draw_file(frame, area, path);
        return;
    }
    let mut lines = vec![Line::styled(
        format!("▶ {}", i18n::t("video")),
        t.accent().bold(),
    )];
    if let Some(s) = info.seconds {
        lines.push(Line::styled(format_seconds(s), t.dim()));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(ratatui::layout::Alignment::Center)
            .block(Block::bordered().border_style(t.dim())),
        area,
    );
}

/// What a picture or video is, in one line: its kind, length, size.
pub(super) fn describe(i: media::Info, bytes: u64) -> String {
    let mut parts = Vec::new();
    if i.animated_gif {
        parts.push(i18n::t("animated GIF, posted as a video").to_string());
    } else if i.kind == media::Kind::Video {
        parts.push(i18n::t("video").to_string());
    }
    if let Some(s) = i.seconds {
        parts.push(format_seconds(s));
    }
    if let Some((w, h)) = i.dims {
        parts.push(format!("{w}×{h}"));
    }
    parts.push(human_bytes(bytes));
    parts.join(" · ")
}

/// The picture browser: the folder on top, its folders and pictures on the
/// left, the selected picture previewed on the right.
pub(super) fn draw_browser(
    frame: &mut Frame,
    area: Rect,
    b: &mut Browser,
    images: &mut Images,
    t: &Theme,
) {
    let title = match (b.videos, b.room) {
        _ if b.folders => i18n::t("Choose a folder").to_string(),
        (true, n) => i18n::tf("Attach pictures (up to {}) or a video", &[&n.to_string()]),
        (false, 1) => i18n::t("Choose a picture").to_string(),
        (false, n) => i18n::tf("Attach pictures (up to {})", &[&n.to_string()]),
    };
    let w = area.width.saturating_sub(4).min(110);
    let h = area.height.saturating_sub(2).min(34);
    let inner = popup(frame, area, w, h, &title, t);
    let [path_row, body, foot] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(inner);
    let dir = b.dir.display().to_string();
    frame.render_widget(
        Paragraph::new(format!(
            " {}",
            truncate_start(&dir, usize::from(path_row.width.saturating_sub(1)))
        ))
        .style(t.accent()),
        path_row,
    );
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(45), Constraint::Min(10)]).areas(body);
    let mut rows = left;
    if let Some(e) = &b.error {
        frame.render_widget(
            Paragraph::new(format!(" {e}")).style(t.error()),
            Rect { height: 1, ..left },
        );
        rows.y += 1;
        rows.height = rows.height.saturating_sub(1);
    }
    if !b.list.items.is_empty() {
        let selected = b.list.selected.min(b.list.items.len() - 1);
        b.list.offset = scroll_offset(
            selected,
            b.list.offset,
            b.list.items.len(),
            rows.height,
            |_| 1,
        );
    }
    let width = usize::from(rows.width);
    for (row, (i, e)) in b
        .list
        .items
        .iter()
        .enumerate()
        .skip(b.list.offset)
        .take(usize::from(rows.height))
        .enumerate()
    {
        let mark = if b.marked.contains(&e.path) {
            "✓ "
        } else {
            "  "
        };
        let (name, size) = match e.kind {
            EntryKind::Parent => ("../".to_string(), String::new()),
            EntryKind::Dir => (format!("{}/", e.name), String::new()),
            EntryKind::Media => (e.name.clone(), human_bytes(e.bytes)),
        };
        let size_w = crate::tui::text::cells(&size);
        let name = truncate(&name, width.saturating_sub(size_w + 4));
        let pad = width.saturating_sub(2 + crate::tui::text::cells(&name) + size_w + 1);
        let line = format!("{mark}{name}{}{size} ", " ".repeat(pad));
        let style = if i == b.list.selected {
            t.selected()
        } else if e.kind == EntryKind::Media {
            Style::new()
        } else {
            t.accent()
        };
        frame.render_widget(
            Paragraph::new(line).style(style),
            Rect {
                y: rows.y + row as u16,
                height: 1,
                ..rows
            },
        );
    }
    let preview = Rect {
        x: right.x + 1,
        width: right.width.saturating_sub(2),
        ..right
    };
    match b.current() {
        Some(e) if e.kind == EntryKind::Media => {
            let about = b.current_info();
            let info = about.map_or_else(|| human_bytes(e.bytes), |i| describe(i, e.bytes));
            let pic = Rect {
                height: preview.height.saturating_sub(3),
                ..preview
            };
            if let Some(i) = about {
                draw_media_box(frame, pic, &e.path, i, images, t);
            }
            frame.render_widget(
                Paragraph::new(vec![
                    Line::styled(
                        truncate(&e.name, usize::from(preview.width)),
                        Style::new().bold(),
                    ),
                    Line::styled(info, t.dim()),
                ]),
                Rect {
                    y: pic.bottom() + 1,
                    height: 2,
                    ..preview
                },
            );
        }
        Some(_) if b.folders => frame.render_widget(
            wrapped(
                i18n::t("enter opens the folder; space chooses the one you are in"),
                preview.width,
            )
            .style(t.dim()),
            preview,
        ),
        Some(_) => frame.render_widget(
            Paragraph::new(i18n::t("enter opens the folder")).style(t.dim()),
            preview,
        ),
        None if b.folders => frame.render_widget(
            wrapped(
                i18n::t("no folders here; space chooses this one"),
                preview.width,
            )
            .style(t.dim()),
            preview,
        ),
        None => frame.render_widget(
            Paragraph::new(i18n::t("no folders, pictures, or videos here")).style(t.dim()),
            preview,
        ),
    }
    let foot_line = match &b.note {
        Some(n) => Line::styled(format!(" {}", i18n::t(n)), t.error()),
        None if b.folders => Line::styled(
            format!(
                " {}",
                crate::tui::text::fit_hints(
                    i18n::t(
                        "enter open  space choose this folder  h up  . hidden  ~ home  esc cancel"
                    ),
                    usize::from(foot.width.saturating_sub(1))
                )
            ),
            t.dim(),
        ),
        None => {
            let marked = if b.marked.is_empty() {
                String::new()
            } else {
                format!(
                    "{}  ",
                    i18n::tf("{} marked", &[&b.marked.len().to_string()])
                )
            };
            let room = usize::from(foot.width).saturating_sub(1 + crate::tui::text::cells(&marked));
            Line::styled(
                format!(
                    " {marked}{}",
                    crate::tui::text::fit_hints(
                        i18n::t(
                            "enter open/choose  space mark  h up  . hidden  ~ home  esc cancel"
                        ),
                        room
                    )
                ),
                t.dim(),
            )
        }
    };
    frame.render_widget(truncate_line(foot_line, usize::from(foot.width)), foot);
}

pub(super) fn draw_edit_profile(frame: &mut Frame, area: Rect, e: &EditProfile, t: &Theme) {
    let inner = popup(frame, area, 72, 16, n!("Edit profile"), t);
    if e.loading {
        let loading = format!(" {}", i18n::t("loading…"));
        frame.render_widget(Paragraph::new(loading).style(t.dim()), inner);
        return;
    }
    // A box too short for all of it gives up rows in a fixed order: the
    // description's extra rows, the blank ones, then the optional avatar,
    // and never the name, a row of the description, or the keys. Left to
    // the layout, the rows were squeezed anywhere, and the description
    // being typed could get no row at all.
    let mut left = inner.height;
    // `whole`: all of `want` or nothing, for a name and its field.
    let mut take = |want: u16, whole: bool| {
        let got = if whole && want > left {
            0
        } else {
            want.min(left)
        };
        left -= got;
        got
    };
    let (name_h, desc_h, foot_h) = (take(2, false), take(2, false), take(1, false));
    let avatar_h = take(2, true);
    let (gap_h, gap2_h, more_h) = (take(1, false), take(1, false), take(3, false));
    let [l0, f0, _, l1, f1, _, l2, f2, foot] = Layout::vertical([
        Constraint::Length(name_h.min(1)),
        Constraint::Length(name_h.saturating_sub(1)),
        Constraint::Length(gap_h),
        Constraint::Length(desc_h.min(1)),
        Constraint::Length(desc_h.saturating_sub(1) + more_h),
        Constraint::Length(gap2_h),
        Constraint::Length(avatar_h / 2),
        Constraint::Length(avatar_h / 2),
        Constraint::Min(foot_h),
    ])
    .areas(inner);
    let fields = [(l0, f0), (l1, f1), (l2, f2)];
    for (i, (label, field)) in fields.into_iter().enumerate() {
        let focused = e.focus == i && !e.saving;
        let style = if focused {
            t.accent().bold()
        } else {
            Style::new()
        };
        frame.render_widget(
            Paragraph::new(format!(" {}", i18n::t(EditProfile::LABELS[i]))).style(style),
            label,
        );
        let field = Rect {
            x: field.x + 3,
            width: field.width.saturating_sub(4),
            ..field
        };
        if i == 1 {
            draw_multi_input(frame, field, &e.fields[i], focused);
        } else {
            draw_single_input(frame, field, &e.fields[i], focused);
        }
    }
    let action = format!(
        " {}",
        i18n::t(if e.saving {
            n!("saving…")
        } else {
            n!("tab next field  ctrl+o choose avatar  ctrl+s save  esc cancel")
        })
    );
    let action = crate::tui::text::fit_hints(&action, usize::from(foot.width));
    frame.render_widget(
        Paragraph::new(action).style(t.dim()),
        Rect { height: 1, ..foot },
    );
}
