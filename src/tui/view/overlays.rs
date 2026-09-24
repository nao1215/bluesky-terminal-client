//! The windows drawn over a tab: the viewer, the actions list, the settings, the account list, the theme picker, the help and the login form.

use super::*;

/// A post's pictures (or video) full screen, the one at `index` shown at its
/// own shape, centered, with its number and alt text under it.
pub(super) fn draw_viewer(
    frame: &mut Frame,
    area: Rect,
    media: &[Media],
    index: usize,
    replay: u32,
    images: &mut Images,
    t: &Theme,
) {
    frame.render_widget(Clear, area);
    frame.render_widget(Block::new().style(t.base()), area);
    let Some(item) = media.get(index) else {
        return;
    };
    let [pic, caption] = Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(area);
    let (url, alt, aspect) = match item {
        Media::Image {
            url, alt, aspect, ..
        } => (Some(url.as_str()), alt, *aspect),
        Media::Video {
            thumbnail,
            alt,
            aspect,
            ..
        } => (thumbnail.as_deref(), alt, *aspect),
    };
    let shape = url.and_then(|u| images.dims(u)).or(aspect);
    let r = match shape {
        Some(d) => fit(pic, d, images.cell_size()),
        None => pic,
    };
    let mut head = vec![Span::styled(
        format!(" {}/{}", index + 1, media.len()),
        t.accent().bold(),
    )];
    match item {
        Media::Video { playlist, .. } => {
            // The thumbnail until the first picture arrives, and if the
            // video cannot be played at all.
            let state = images.draw_video(frame, r, playlist, replay);
            if !images.video_shown()
                && let Some(url) = url
            {
                images.draw(frame, r, url);
            }
            let (text, style) = match state {
                State::Loading => (format!("  {}", i18n::t("loading the video…")), t.dim()),
                State::Playing => (format!("  ▶ {}", i18n::t("playing (no sound)")), t.accent()),
                State::Ended => (
                    format!("  ■ {}", i18n::t("ended  r plays it again")),
                    t.dim(),
                ),
                State::Warning(why) => (
                    format!(
                        "  ⚠ {}",
                        i18n::tf("{}; showing its thumbnail", &[i18n::t(&why)])
                    ),
                    t.error(),
                ),
            };
            head.push(Span::styled(text, style));
        }
        Media::Image { url, thumb, .. } => {
            // The thumbnail at once, the full size as soon as it is here.
            images.draw_first(frame, r, &[url.as_str(), thumb.as_str()]);
        }
    }
    let alt = if alt.is_empty() {
        Line::styled(format!(" {}", i18n::t("(no alt text)")), t.dim())
    } else {
        Line::from(format!(
            " {}",
            truncate(
                &crate::tui::text::one_line(alt),
                usize::from(caption.width.saturating_sub(2))
            )
        ))
    };
    frame.render_widget(Paragraph::new(vec![Line::from(head), alt]), caption);
}

/// The list `.` opens: every key of this view that acts on the selected
/// post, with what it would do now beside it.
pub(super) fn draw_actions(
    frame: &mut Frame,
    area: Rect,
    entries: &[keys::Hint],
    selected: usize,
    t: &Theme,
) {
    let inner = popup(
        frame,
        area,
        ACTIONS_W,
        entries.len() as u16 + 2,
        n!("Actions"),
        t,
    );
    let lines: Vec<Line> = entries
        .iter()
        .enumerate()
        .map(|(i, (key, what))| {
            let marker = if i == selected { "▶ " } else { "  " };
            let key = Span::styled(format!("{marker}{key:<7}"), t.accent().bold());
            let what = i18n::t(what);
            let what = if i == selected {
                Span::styled(what, t.base().bold())
            } else {
                Span::styled(what, t.dim())
            };
            Line::from(vec![key, what])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

/// The settings screen: a row per setting, `name  value`, and under the
/// list what the selected one's value comes from or what Enter does, or the
/// line its new value is typed in.
pub(super) fn draw_settings(
    frame: &mut Frame,
    area: Rect,
    rows: &[SettingRow],
    selected: usize,
    typing: Option<&TextInput>,
    t: &Theme,
) {
    const NAME_W: usize = 16;
    let inner = popup(
        frame,
        area,
        SETTINGS_W,
        rows.len() as u16 + 5,
        n!("Settings"),
        t,
    );
    let width = usize::from(inner.width);
    // The note of the selected row, whole, below the list: two rows when
    // there is room, one on a short screen.
    let note_h = match inner.height {
        0..=2 => 0,
        3..=5 => 1,
        _ => 2,
    };
    let list_h = usize::from(inner.height.saturating_sub(note_h + u16::from(note_h > 0)));
    let top = (selected + 1).saturating_sub(list_h);
    let mut lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .skip(top)
        .take(list_h)
        .map(|(i, r)| {
            let marker = if i == selected { "▶ " } else { "  " };
            // Padded by the cells it takes: a name in Japanese is twice as
            // wide as its characters.
            let name = truncate(i18n::t(r.name), NAME_W);
            let pad = " ".repeat(NAME_W.saturating_sub(crate::tui::text::cells(&name)));
            let head = format!("{marker}{name}{pad} ");
            let room = width.saturating_sub(head.width());
            // A path is cut at its start: its end names the folder.
            let value = truncate_start(i18n::t(&r.value), room);
            let style = if i == selected {
                t.base().bold()
            } else {
                t.dim()
            };
            Line::from(vec![
                Span::styled(head, t.accent().bold()),
                Span::styled(value, style),
            ])
        })
        .collect();
    if note_h > 0
        && let Some(input) = typing
    {
        lines.push(Line::raw(""));
        if note_h > 1 {
            lines.push(Line::styled(
                format!(
                    " {}",
                    i18n::t("enter keeps it, empty is the default, esc cancels")
                ),
                t.dim(),
            ));
        }
        frame.render_widget(Paragraph::new(lines), inner);
        let field = Rect {
            x: inner.x + 1,
            y: inner.bottom().saturating_sub(1),
            width: inner.width.saturating_sub(2),
            height: 1,
        };
        draw_single_input(frame, field, input, true);
        return;
    }
    if note_h > 0
        && let Some(r) = rows.get(selected)
    {
        lines.push(Line::raw(""));
        let note: Vec<String> = wrap(i18n::t(&r.note), width.saturating_sub(1).max(1))
            .into_iter()
            .take(usize::from(note_h))
            .collect();
        lines.extend(
            note.into_iter()
                .map(|l| Line::styled(format!(" {l}"), t.dim())),
        );
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

/// The list `+` opens on the Timeline tab, or the search being typed for a
/// search column.
pub(super) fn draw_add_column(
    frame: &mut Frame,
    area: Rect,
    titles: &[String],
    selected: usize,
    query: Option<&TextInput>,
    t: &Theme,
) {
    let inner = popup(
        frame,
        area,
        ACTIONS_W,
        titles.len() as u16 + 4,
        n!("Add a column"),
        t,
    );
    let width = usize::from(inner.width);
    if let Some(input) = query {
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(format!(" {}", i18n::t("Search posts for:")), t.dim()),
                Line::raw(""),
            ]),
            inner,
        );
        let field = Rect {
            x: inner.x + 1,
            y: inner.y + 1,
            width: inner.width.saturating_sub(2),
            height: 1,
        };
        draw_single_input(frame, field, input, true);
        return;
    }
    let lines: Vec<Line> = titles
        .iter()
        .enumerate()
        .map(|(i, title)| {
            let marker = if i == selected { "▶ " } else { "  " };
            let style = if i == selected {
                t.base().bold()
            } else {
                t.base()
            };
            Line::from(vec![
                Span::styled(marker, t.accent().bold()),
                Span::styled(truncate(title, width.saturating_sub(2)), style),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

/// The account list `A` opens: every logged-in account, the one in use
/// marked, and what the keys do.
pub(super) fn draw_account_list(
    frame: &mut Frame,
    area: Rect,
    accounts: &[crate::tui::app::Account],
    selected: usize,
    me: Option<&str>,
    t: &Theme,
) {
    let inner = popup(
        frame,
        area,
        ACTIONS_W,
        accounts.len().max(1) as u16 + 4,
        n!("Accounts"),
        t,
    );
    let width = usize::from(inner.width);
    let mut lines: Vec<Line> = accounts
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let marker = if i == selected { "▶ " } else { "  " };
            let used = if me == Some(a.did.as_str()) {
                format!("  {}", i18n::t("in use"))
            } else {
                String::new()
            };
            let name = truncate(
                &format!("@{}", a.handle),
                width.saturating_sub(marker.width() + crate::tui::text::cells(&used)),
            );
            let style = if i == selected {
                t.base().bold()
            } else {
                t.base()
            };
            Line::from(vec![
                Span::styled(marker, t.accent().bold()),
                Span::styled(name, style),
                Span::styled(used, t.accent()),
            ])
        })
        .collect();
    if accounts.is_empty() {
        lines.push(Line::styled(
            format!("  {}", i18n::t("no account yet")),
            t.dim(),
        ));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        truncate(
            &format!(" {}", i18n::t("a log in another  x log out")),
            width,
        ),
        t.dim(),
    ));
    frame.render_widget(Paragraph::new(lines), inner);
}

/// The languages the settings offer, each in its own name, the one in use
/// marked.
pub(super) fn draw_languages(
    frame: &mut Frame,
    area: Rect,
    selected: usize,
    now: i18n::Lang,
    t: &Theme,
) {
    let langs = i18n::Lang::ALL;
    let inner = popup(frame, area, 34, langs.len() as u16 + 2, n!("Language"), t);
    let lines: Vec<Line> = langs
        .iter()
        .enumerate()
        .map(|(i, lang)| {
            let marker = if i == selected { "▶ " } else { "  " };
            let style = if i == selected {
                t.base().bold()
            } else {
                t.base()
            };
            let mut row = vec![
                Span::styled(marker, t.accent().bold()),
                Span::styled(lang.name(), style),
            ];
            if *lang == now {
                row.push(Span::styled(format!("  {}", i18n::t("in use")), t.accent()));
            }
            Line::from(row)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

pub(super) fn draw_login(frame: &mut Frame, area: Rect, form: &LoginForm, t: &Theme) {
    let inner = popup(frame, area, 64, 17, n!("Log in to Bluesky"), t);
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(format!(
                " {}",
                i18n::t("Your Bluesky password works; an app password is safer.")
            )),
            Line::styled(
                format!(
                    " {}",
                    i18n::t("Settings → Privacy and security → App passwords")
                ),
                t.dim(),
            ),
            Line::styled(
                format!(
                    " {}",
                    i18n::t("bsky is an unofficial client, not made by Bluesky.")
                ),
                t.dim(),
            ),
        ]),
        rows[0],
    );
    for (i, label) in LoginForm::LABELS.iter().enumerate() {
        let focused = form.focus == i && !form.pending;
        let style = if focused {
            t.accent().bold()
        } else {
            Style::new()
        };
        frame.render_widget(
            Paragraph::new(format!(" {}", i18n::t(label))).style(style),
            rows[2 + i * 2],
        );
        let field = rows[3 + i * 2];
        let field = Rect {
            x: field.x + 3,
            width: field.width.saturating_sub(4),
            ..field
        };
        frame.render_widget(
            Paragraph::new("›").style(t.accent()),
            Rect {
                x: field.x - 2,
                width: 1,
                ..field
            },
        );
        draw_single_input(frame, field, &form.fields[i], focused);
    }
    // The message area takes whatever rows are left, so a server's reason,
    // which is the part the user needs, is never cut off by the wrap.
    let msg = if form.pending {
        Line::styled(format!(" {}", i18n::t("logging in…")), t.dim())
    } else if let Some(e) = &form.error {
        Line::styled(format!(" {}", i18n::t(e)), t.error())
    } else {
        Line::raw("")
    };
    frame.render_widget(
        Paragraph::new(msg).wrap(ratatui::widgets::Wrap { trim: true }),
        rows[9],
    );
    frame.render_widget(
        Paragraph::new(format!(
            " {}",
            if form.adding {
                i18n::t("enter next/submit  tab switch field  esc back")
            } else {
                i18n::t("enter next/submit  tab switch field  esc quit")
            }
        ))
        .style(t.dim()),
        rows[10],
    );
}

/// The theme picker: every theme's name with a strip of its colors, the
/// selected one marked. The rest of the screen already shows it, since
/// moving the selection applies it.
pub(super) fn draw_themes(frame: &mut Frame, area: Rect, selected: usize, t: &Theme) {
    let n = THEMES.len();
    // A window of a few themes that scrolls with the selection, keeping it in
    // the middle where it can; not a list from the top of the screen to the
    // bottom.
    let rows = usize::from(area.height.saturating_sub(6)).clamp(1, THEME_ROWS.min(n));
    let first = selected.saturating_sub(rows / 2).min(n - rows);
    let width = THEMES.iter().map(|t| t.name.len()).max().unwrap_or(0) + 1;
    let lines: Vec<Line> = THEMES
        .iter()
        .enumerate()
        .skip(first)
        .take(rows)
        .map(|(i, theme)| {
            let marker = if i == selected { "▶ " } else { "  " };
            let name = format!("{marker}{:<width$}", theme.name);
            let mut spans = vec![if i == selected {
                Span::styled(name, t.accent().bold())
            } else {
                Span::raw(name)
            }];
            // A swatch in the theme's own colors, whatever the current one is.
            for c in [theme.accent, theme.like, theme.repost, theme.ok, theme.dim] {
                spans.push(Span::styled("██", Style::new().fg(c).bg(theme.bg)));
            }
            Line::from(spans)
        })
        .collect();
    let w = (width + 2 + 10 + 4) as u16;
    let inner = popup(frame, area, w.max(34), rows as u16 + 4, n!("Theme"), t);
    let [body, foot] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
    frame.render_widget(Paragraph::new(lines), body);
    // Arrows on the frame say there is more above or below.
    let more = |show: bool, y: u16, mark: &'static str| {
        if show { Some((mark, y)) } else { None }
    };
    for (mark, y) in [
        more(first > 0, body.y.saturating_sub(1), "▲"),
        more(first + rows < n, foot.y + 1, "▼"),
    ]
    .into_iter()
    .flatten()
    {
        frame.render_widget(
            Paragraph::new(mark).style(t.accent()),
            Rect {
                x: body.right().saturating_sub(2),
                y,
                width: 1,
                height: 1,
            },
        );
    }
    frame.render_widget(
        Paragraph::new(format!(
            " {}/{n}  {}",
            selected + 1,
            i18n::t("enter apply  esc cancel")
        ))
        .style(t.dim()),
        foot,
    );
}

pub(super) fn draw_help(
    frame: &mut Frame,
    area: Rect,
    scroll: &mut u16,
    pictures: bool,
    t: &Theme,
) {
    // The popup is 64 wide where the screen allows it; knowing the width
    // here decides whether a description fits beside its keys.
    let inner_w = usize::from(HELP_W.min(area.width).saturating_sub(2));
    // A description beside the keys needs the key column and enough cells
    // after it to read a few words. On a narrower screen it goes under
    // them, wrapped, rather than being cut to nothing.
    let beside = inner_w >= HELP_KEYS + 20;
    let mut lines: Vec<Line> = Vec::new();
    for (i, (title, keys)) in keys::help(pictures).into_iter().enumerate() {
        if i > 0 {
            lines.push(Line::raw(""));
        }
        lines.push(Line::styled(format!(" {title}"), Style::new().bold()));
        for (k, d) in keys {
            if beside {
                // A description too long for its column wraps onto more
                // rows under itself instead of being cut off.
                let mut parts = wrap(d, inner_w - HELP_KEYS).into_iter();
                let first = parts.next().unwrap_or_default();
                lines.push(Line::from(vec![
                    Span::styled(format!("   {k:<15}"), t.accent().bold()),
                    Span::raw(first),
                ]));
                lines.extend(parts.map(|l| Line::raw(format!("{:HELP_KEYS$}{l}", ""))));
                continue;
            }
            lines.push(Line::from(vec![Span::styled(
                format!("   {k}"),
                t.accent().bold(),
            )]));
            lines.extend(
                wrap(d, inner_w.saturating_sub(5).max(1))
                    .into_iter()
                    .map(|l| Line::raw(format!("     {l}"))),
            );
        }
    }
    let inner = popup(frame, area, HELP_W, lines.len() as u16 + 3, n!("Keys"), t);
    let [body, foot] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
    // Clamp here, where the viewport is known, and write it back so scrolling
    // up after overshooting starts at once.
    let max = (lines.len() as u16).saturating_sub(body.height);
    *scroll = (*scroll).min(max);
    frame.render_widget(Paragraph::new(lines).scroll((*scroll, 0)), body);
    let more = if *scroll < max {
        format!("  {}", i18n::t("j/pgdn more"))
    } else {
        String::new()
    };
    frame.render_widget(
        Paragraph::new(format!(" {}{more}", i18n::t("esc/q/? close"))).style(t.dim()),
        foot,
    );
}
