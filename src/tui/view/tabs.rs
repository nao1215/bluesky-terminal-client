//! The tabs: Timeline, Search, Notifications, Profile, Columns and Chat.

use super::*;

/// The Chat tab: the conversations, or the one open with its messages
/// (the newest at the bottom) and the box to write in.
pub(super) fn draw_chat(
    frame: &mut Frame,
    body: Rect,
    app: &mut App,
    images: &mut Images,
    t: &Theme,
) {
    let me = app
        .session
        .as_ref()
        .map(|s| s.did.clone())
        .unwrap_or_default();
    if let Some(why) = &app.chat.refused {
        frame.render_widget(
            Paragraph::new(format!(" {why}"))
                .style(t.error())
                .wrap(ratatui::widgets::Wrap { trim: true }),
            body,
        );
        return;
    }
    let Some(open) = &mut app.chat.open else {
        let messages = (
            " loading…",
            " No conversations yet. m on someone's profile starts one.",
        );
        let me2 = me.clone();
        draw_two_line_rows(
            frame,
            body,
            &mut app.chat.convos,
            images,
            messages,
            t,
            move |c| {
                c.members
                    .iter()
                    .find(|m| m.did != me2)
                    .and_then(|m| m.avatar.as_deref())
            },
            |c, w| convo_lines(c, &me, w, t),
        );
        return;
    };
    let [head, msgs, input] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(body);
    let width = usize::from(body.width.saturating_sub(2)).max(1);
    frame.render_widget(
        Paragraph::new(format!(
            " {}",
            truncate(&convo_title(&open.convo, &me), width)
        ))
        .style(t.accent().bold()),
        head,
    );
    let names: std::collections::HashMap<&str, String> = open
        .convo
        .members
        .iter()
        .map(|m| (m.did.as_str(), m.name().to_string()))
        .collect();
    let mut lines: Vec<Line> = Vec::new();
    if open.loading_older {
        lines.push(Line::styled(" loading earlier messages…", t.dim()));
    } else if open.older.is_none() && open.loaded {
        lines.push(Line::styled(" the start of the conversation", t.dim()));
    }
    for m in &open.messages {
        let who = if m.sender == me {
            "you".to_string()
        } else {
            names
                .get(m.sender.as_str())
                .cloned()
                .unwrap_or_else(|| m.sender.clone())
        };
        let style = if m.sender == me {
            t.accent()
        } else {
            t.base().bold()
        };
        lines.push(Line::from(vec![
            Span::raw(" "),
            Span::styled(truncate(&who, width.saturating_sub(20)), style),
            Span::styled(format!(" · {}", format_time(&m.sent_at)), t.dim()),
        ]));
        let text = if m.deleted {
            Line::styled("   (deleted)", t.dim())
        } else if m.system {
            Line::styled(format!("   {}", m.text), t.dim())
        } else {
            Line::raw("")
        };
        if m.deleted || m.system {
            lines.push(text);
        } else {
            for l in wrap(&m.text, width.saturating_sub(3).max(1)) {
                lines.push(Line::raw(format!("   {l}")));
            }
        }
    }
    if !open.loaded {
        lines.push(Line::styled(" loading…", t.dim()));
    } else if let Some(e) = &open.error {
        lines.push(Line::styled(format!(" {e}  (R to retry)"), t.error()));
    }
    // Scrolled `scroll` lines up from the newest, as far as there is.
    let h = usize::from(msgs.height);
    let most = lines.len().saturating_sub(h);
    open.scroll = open.scroll.min(most);
    let top = most - open.scroll;
    let shown: Vec<Line> = lines.into_iter().skip(top).take(h).collect();
    frame.render_widget(Paragraph::new(shown), msgs);
    if open.typing {
        frame.render_widget(Paragraph::new(" ›").style(t.accent()), input);
        let field = Rect {
            x: input.x + 3,
            width: input.width.saturating_sub(3),
            ..input
        };
        draw_single_input(frame, field, &open.input, true);
    } else {
        let draft = open.input.text();
        let text = if open.sending {
            " sending…".to_string()
        } else if draft.is_empty() {
            " i write a message".to_string()
        } else {
            format!(
                " i continue: {}",
                truncate(&draft, width.saturating_sub(13))
            )
        };
        frame.render_widget(Paragraph::new(text).style(t.dim()), input);
    }
}

/// Who a conversation is with: everyone in it but you.
pub(super) fn convo_title(c: &crate::api::types::Convo, me: &str) -> String {
    let others: Vec<String> = c
        .others(me)
        .iter()
        .map(|m| format!("{} @{}", m.name(), m.handle))
        .collect();
    if others.is_empty() {
        "(just you)".to_string()
    } else {
        others.join(", ")
    }
}

/// A conversation in the list: who, how many unread, the last message.
pub(super) fn convo_lines(
    c: &crate::api::types::Convo,
    me: &str,
    width: u16,
    t: &Theme,
) -> Vec<Line<'static>> {
    let w = usize::from(width).max(1);
    let mut head = vec![Span::styled(
        truncate(&convo_title(c, me), w.saturating_sub(12)),
        Style::new().bold(),
    )];
    if c.unread_count > 0 && !c.muted {
        head.push(Span::styled(
            format!("  {} new", c.unread_count),
            t.accent().bold(),
        ));
    }
    if c.muted {
        head.push(Span::styled("  muted", t.dim()));
    }
    let last = match &c.last_message {
        Some(m) if m.deleted => "(deleted)".to_string(),
        Some(m) => {
            let who = if m.sender == me { "you: " } else { "" };
            format!("{who}{}", m.text.lines().next().unwrap_or(""))
        }
        None => String::new(),
    };
    vec![Line::from(head), Line::styled(truncate(&last, w), t.dim())]
}

/// Narrowest a column is drawn; fewer columns show on a narrow screen.
pub(super) const COLUMN_MIN_W: u16 = 36;

/// The Columns tab: the columns side by side, as many as fit, the focused
/// one among them, each with its title above its list; how many are off
/// screen on either side is said in the titles of the outermost ones.
pub(super) fn draw_columns(
    frame: &mut Frame,
    body: Rect,
    app: &mut App,
    images: &mut Images,
    t: &Theme,
) {
    let n = app.columns.items.len();
    if n == 0 {
        frame.render_widget(
            Paragraph::new(" No columns yet. Press + to add one: the timeline, a feed, notifications, your posts, or a search.")
                .style(t.dim())
                .wrap(ratatui::widgets::Wrap { trim: true }),
            body,
        );
        return;
    }
    let fit = usize::from((body.width / COLUMN_MIN_W).max(1)).min(n);
    let focus = app.columns.focus.min(n - 1);
    let first = focus.saturating_sub(fit - 1).min(n - fit);
    let areas = Layout::horizontal(vec![Constraint::Ratio(1, fit as u32); fit]).split(body);
    let me = app.session.as_ref().map(|s| s.did.clone());
    for (slot, i) in (first..first + fit).enumerate() {
        let area = areas[slot];
        let [head, list] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
        let col = &mut app.columns.items[i];
        let mut title = col.source.title();
        if slot == 0 && first > 0 {
            title = format!("‹{first} {title}");
        }
        let right = n - (first + fit);
        if slot == fit - 1 && right > 0 {
            title = format!("{title} {right}›");
        }
        let style = if i == focus { t.selected() } else { t.dim() };
        let width = usize::from(head.width.saturating_sub(1));
        frame.render_widget(
            Paragraph::new(format!(" {}", truncate(&title, width))).style(style),
            head,
        );
        // A gap on the right keeps the columns apart.
        let list = Rect {
            width: list.width.saturating_sub(u16::from(slot + 1 < fit)),
            ..list
        };
        match &mut col.rows {
            Rows::Posts(l) => draw_posts(
                frame,
                list,
                l,
                images,
                "Nothing here yet.",
                me.as_deref(),
                t,
            ),
            Rows::Notifications(l) => draw_notification_list(frame, list, l, images, t),
        }
    }
}

/// The Timeline tab: the following timeline or a pinned feed, with a row
/// naming them all when there are feeds to choose from.
pub(super) fn draw_timeline(
    frame: &mut Frame,
    body: Rect,
    app: &mut App,
    images: &mut Images,
    t: &Theme,
) {
    let area = if app.feeds.is_empty() {
        body
    } else {
        let [bar, rest] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(body);
        let names: Vec<&str> = std::iter::once("Following")
            .chain(app.feeds.iter().map(|f| f.info.name.as_str()))
            .collect();
        frame.render_widget(feed_bar(&names, app.feed, usize::from(bar.width), t), bar);
        rest
    };
    let empty = if app.feed == 0 {
        "No posts from accounts you follow yet. Press R to refresh."
    } else {
        "No posts in this feed yet. Press R to refresh."
    };
    draw_posts(frame, area, app.feed_list(), images, empty, None, t);
}

/// The feeds to choose from, the shown one highlighted, with `[ ] feeds`
/// at the end. When they do not all fit, the row starts as far left as it
/// can with the shown one in sight, and an ellipsis marks each side where
/// feeds are left out.
pub(super) fn feed_bar(names: &[&str], shown: usize, width: usize, t: &Theme) -> Line<'static> {
    const HINT: &str = "  [ ] feeds";
    let room = width.saturating_sub(HINT.width() + 4);
    // A name is cut to fit what room there is, so the shown one always does.
    let max = room.saturating_sub(3).clamp(4, 24);
    let labels: Vec<String> = names.iter().map(|n| truncate(n, max)).collect();
    // " name " and the space after it.
    let cost = |i: usize| labels[i].width() + 3;
    let span = |from: usize, to: usize| (from..=to).map(cost).sum::<usize>();
    let from = (0..=shown)
        .find(|&f| span(f, shown) <= room)
        .unwrap_or(shown);
    let mut to = shown;
    while to + 1 < labels.len() && span(from, to + 1) <= room {
        to += 1;
    }
    let mut spans = vec![Span::raw(if from > 0 { "…" } else { " " })];
    for (i, label) in labels.iter().enumerate().take(to + 1).skip(from) {
        let style = if i == shown { t.selected() } else { t.dim() };
        spans.push(Span::styled(format!(" {label} "), style));
        spans.push(Span::raw(" "));
    }
    if to + 1 < labels.len() {
        spans.push(Span::raw("…"));
    }
    spans.push(Span::styled(HINT, t.dim()));
    truncate_line(Line::from(spans), width)
}

/// A thread over the current tab: a title row, then the posts, the opened
/// one among them where the selection starts.
pub(super) fn draw_thread(
    frame: &mut Frame,
    area: Rect,
    th: &mut ThreadView,
    images: &mut Images,
    me: Option<&str>,
    t: &Theme,
) {
    let [title, list] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
    frame.render_widget(
        Line::from(vec![
            Span::styled(" Thread ", t.selected()),
            Span::styled("  esc back", t.dim()),
        ]),
        title,
    );
    if let Some(e) = &th.error {
        frame.render_widget(
            Paragraph::new(format!(" could not load the thread: {e}  (R to retry)"))
                .style(t.error())
                .wrap(ratatui::widgets::Wrap { trim: true }),
            list,
        );
        return;
    }
    draw_posts(
        frame,
        list,
        &mut th.list,
        images,
        "The thread is empty.",
        me,
        t,
    );
}

/// What a notification says its author did.
pub(super) fn notif_action(reason: &str) -> String {
    match reason {
        "like" => "liked your post".into(),
        "repost" => "reposted your post".into(),
        "follow" => "followed you".into(),
        "mention" => "mentioned you".into(),
        "reply" => "replied to you".into(),
        "quote" => "quoted your post".into(),
        "like-via-repost" => "liked your repost".into(),
        "repost-via-repost" => "reposted your repost".into(),
        "starterpack-joined" => "joined through your starter pack".into(),
        "verified" => "verified you".into(),
        "unverified" => "removed your verification".into(),
        "subscribed-post" => "posted".into(),
        other => format!("({other})"),
    }
}

pub(super) fn notif_lines(item: &NotifItem, width: u16, t: &Theme) -> Vec<Line<'static>> {
    let width = usize::from(width.max(1));
    let n = &item.n;
    let mut head = Vec::new();
    if item.fresh {
        head.push(Span::styled("● ", t.accent()));
    }
    head.push(Span::styled(
        n.author.name().to_string(),
        Style::new().bold(),
    ));
    head.push(Span::styled(format!(" @{}", n.author.handle), t.dim()));
    head.push(Span::raw(format!(" {}", notif_action(&n.reason))));
    head.push(Span::styled(
        format!(" · {}", format_time(&n.indexed_at)),
        t.dim(),
    ));
    // The post itself for a reply or mention, the viewer's own post (dim)
    // for a like or repost; nothing for a follow.
    let text = |p: &Post| p.record().text.lines().next().unwrap_or("").to_string();
    let second = match (&item.post, &item.subject) {
        (Some(p), _) => Line::from(truncate(&text(p), width)),
        (None, Some(p)) => Line::styled(truncate(&text(p), width), t.dim()),
        (None, None) => Line::default(),
    };
    vec![truncate_line(Line::from(head), width), second]
}

pub(super) fn draw_notifications(
    frame: &mut Frame,
    area: Rect,
    app: &mut App,
    images: &mut Images,
) {
    let t = app.theme;
    draw_notification_list(frame, area, &mut app.notifications, images, &t);
}

pub(super) fn draw_notification_list(
    frame: &mut Frame,
    area: Rect,
    list: &mut List<NotifItem>,
    images: &mut Images,
    t: &Theme,
) {
    let messages = (" loading…", " No notifications yet.");
    draw_two_line_rows(
        frame,
        area,
        list,
        images,
        messages,
        t,
        |i| i.n.author.avatar.as_deref(),
        |i, w| notif_lines(i, w, t),
    );
}

pub(super) fn draw_search(frame: &mut Frame, area: Rect, app: &mut App, images: &mut Images) {
    let t = &app.theme.clone();
    let [bar, input, gap, results] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas(area);
    let mode = app.search.mode;
    let tab = |m: SearchMode, label: &'static str| {
        if m == mode {
            Span::styled(format!(" {label} "), t.selected())
        } else {
            Span::styled(format!(" {label} "), t.dim())
        }
    };
    frame.render_widget(
        Line::from(vec![
            Span::raw(" "),
            tab(SearchMode::Posts, "Posts"),
            Span::raw(" "),
            tab(SearchMode::Accounts, "Accounts"),
        ]),
        bar,
    );
    let prompt = " search: ";
    let field = Rect {
        x: input.x + prompt.width() as u16,
        width: input.width.saturating_sub(prompt.width() as u16 + 1),
        ..input
    };
    // A focused box looks different from one that is only showing the last
    // query: reversed prompt and a cursor, against a dim prompt and a hint.
    let prompt_style = if app.search.editing {
        t.selected()
    } else {
        t.dim()
    };
    frame.render_widget(Paragraph::new(prompt).style(prompt_style), input);
    match (app.search.input.is_empty(), app.search.editing) {
        (true, false) => {
            frame.render_widget(Paragraph::new("press / or i to type").style(t.dim()), field)
        }
        (true, true) => {
            frame.render_widget(Paragraph::new("type, then enter").style(t.dim()), field);
            frame.set_cursor_position(Position::new(field.x, field.y));
        }
        _ => draw_single_input(frame, field, &app.search.input, app.search.editing),
    }
    let _ = gap;
    match mode {
        SearchMode::Posts => {
            if app.search.posts.loaded || !app.search.posts.items.is_empty() || app.pending > 0 {
                let me = app.session.as_ref().map(|s| s.did.as_str());
                draw_posts(
                    frame,
                    results,
                    &mut app.search.posts,
                    images,
                    "No posts found.",
                    me,
                    t,
                );
            }
        }
        SearchMode::Accounts => {
            if app.search.actors.loaded || app.pending > 0 {
                draw_accounts(frame, results, &mut app.search.actors, images, t);
            }
        }
    }
}

pub(super) fn draw_profile(frame: &mut Frame, area: Rect, app: &mut App, images: &mut Images) {
    let t = &app.theme.clone();
    let Some(p) = app.profile.profile.clone() else {
        let msg = match &app.profile.error {
            Some(e) => Paragraph::new(format!(" could not load the profile: {e}  (R to retry)"))
                .style(t.error()),
            None => Paragraph::new(" loading…").style(t.dim()),
        };
        frame.render_widget(msg.wrap(ratatui::widgets::Wrap { trim: true }), area);
        return;
    };
    let big = if images.shows() { BIG_AVATAR } else { (0, 0) };
    let text_x = if images.shows() {
        MARK_W + big.0 + 2
    } else {
        MARK_W + 1
    };
    let text_w = area.width.saturating_sub(text_x);
    let desc: Vec<String> = wrap(
        p.description.as_deref().unwrap_or(""),
        usize::from(text_w.max(1)),
    )
    .into_iter()
    .take(4)
    .collect();
    let mut lines = vec![
        Line::styled(p.name().to_string(), Style::new().bold()),
        Line::styled(format!("@{}", p.handle), t.dim()),
        Line::from(vec![
            Span::styled(
                format!("{}", p.followers_count.unwrap_or(0)),
                Style::new().bold(),
            ),
            Span::raw(" followers  "),
            Span::styled(
                format!("{}", p.follows_count.unwrap_or(0)),
                Style::new().bold(),
            ),
            Span::raw(" following  "),
            Span::styled(
                format!("{}", p.posts_count.unwrap_or(0)),
                Style::new().bold(),
            ),
            Span::raw(" posts"),
        ]),
    ];
    let own = app.profile.actor.is_none();
    if !own {
        let viewer = p.viewer.clone().unwrap_or_default();
        let mut rel = vec![if viewer.following.is_some() {
            Span::styled("✓ following", t.accent().bold())
        } else {
            Span::styled("not following", t.dim())
        }];
        if viewer.followed_by.is_some() {
            rel.push(Span::styled("  · follows you", t.dim()));
        }
        rel.push(Span::styled("  (f to toggle)", t.dim()));
        lines.push(Line::from(rel));
    } else {
        // Buttons that say the keys exist: bsky does not read the mouse, so
        // they are pressed with the key they name.
        let button = |key: &'static str, what: &'static str| {
            [
                Span::styled("[ ", t.accent()),
                Span::styled(key, t.accent().bold()),
                Span::styled(format!(" {what} ]"), t.accent()),
            ]
        };
        let mut row = vec![Span::styled("this is you  ", t.dim())];
        row.extend(button("e", "Edit profile"));
        row.push(Span::raw("  "));
        row.extend(button("s", "Settings"));
        lines.push(Line::from(row));
    }
    lines.extend(desc.into_iter().map(Line::from));
    let head_h = (lines.len() as u16).max(big.1) + 1;
    let [head, feed] =
        Layout::vertical([Constraint::Length(head_h), Constraint::Min(1)]).areas(area);
    if let Some(url) = &p.avatar {
        let a = Rect {
            x: head.x + MARK_W,
            y: head.y,
            width: BIG_AVATAR.0,
            height: BIG_AVATAR.1,
        };
        images.draw(frame, a.intersection(head), url);
    }
    frame.render_widget(
        Paragraph::new(lines),
        Rect {
            x: head.x + text_x.min(head.width),
            width: text_w,
            height: head_h - 1,
            ..head
        },
    );
    draw_posts(
        frame,
        feed,
        &mut app.profile.posts,
        images,
        "No posts yet.",
        None,
        t,
    );
}
