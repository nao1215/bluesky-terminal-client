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
            wrapped(&format!(" {}", i18n::t(why)), body.width).style(t.error()),
            body,
        );
        return;
    }
    let Some(open) = &mut app.chat.open else {
        let messages = (
            n!("loading…"),
            n!("No conversations yet. m on someone's profile starts one."),
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
        lines.push(Line::styled(
            format!(" {}", i18n::t("loading earlier messages…")),
            t.dim(),
        ));
    } else if open.older.is_none() && open.loaded {
        lines.push(Line::styled(
            format!(" {}", i18n::t("the start of the conversation")),
            t.dim(),
        ));
    }
    for m in &open.messages {
        let who = if m.sender == me {
            i18n::t("you").to_string()
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
            Line::styled(format!("   {}", i18n::t("(deleted)")), t.dim())
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
        lines.push(Line::styled(format!(" {}", i18n::t("loading…")), t.dim()));
    } else if let Some(e) = &open.error {
        lines.push(Line::styled(
            format!(" {e}  {}", i18n::t("(R to retry)")),
            t.error(),
        ));
    }
    // Scrolled `scroll` lines up from the newest, as far as there is.
    let h = usize::from(msgs.height);
    let most = lines.len().saturating_sub(h);
    open.scroll = open.scroll.min(most);
    open.at_top = open.scroll >= most;
    let top = most - open.scroll;
    let shown: Vec<Line> = lines.into_iter().skip(top).take(h).collect();
    // A short conversation sits just above the input line, where the next
    // message goes, rather than under the title with a gap below it.
    let used = u16::try_from(shown.len()).unwrap_or(msgs.height);
    let msgs = Rect {
        y: msgs.y + msgs.height.saturating_sub(used),
        height: used.min(msgs.height),
        ..msgs
    };
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
            format!(" {}", i18n::t("sending…"))
        } else if draft.is_empty() {
            format!(" {}", i18n::t("i write a message"))
        } else {
            let head = format!(" {} ", i18n::t("i continue:"));
            let room = width.saturating_sub(crate::tui::text::cells(&head));
            format!("{head}{}", truncate(&draft, room))
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
        i18n::t("(just you)").to_string()
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
            format!("  {}", i18n::tf("{} new", &[&c.unread_count.to_string()])),
            t.accent().bold(),
        ));
    }
    if c.muted {
        head.push(Span::styled(format!("  {}", i18n::t("muted")), t.dim()));
    }
    let last = match &c.last_message {
        Some(m) if m.deleted => i18n::t("(deleted)").to_string(),
        Some(m) => {
            let first = m.text.lines().next().unwrap_or("");
            if m.sender == me {
                i18n::tf("you: {}", &[first])
            } else {
                first.to_string()
            }
        }
        None => String::new(),
    };
    vec![Line::from(head), Line::styled(truncate(&last, w), t.dim())]
}

/// Narrowest a column is drawn; fewer columns show on a narrow screen.
pub(super) const COLUMN_MIN_W: u16 = 36;

/// The Timeline tab with columns: side by side, as many as fit, the focused
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
    // Without columns the Timeline tab shows the timeline instead.
    if n == 0 {
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
        // The count of columns off to the right is kept whole: the title
        // before it is what is shortened.
        let right = n - (first + fit);
        let after = if slot == fit - 1 && right > 0 {
            format!(" {right}›")
        } else {
            String::new()
        };
        let style = if i == focus { t.selected() } else { t.dim() };
        let width = usize::from(head.width.saturating_sub(1))
            .saturating_sub(crate::tui::text::cells(&after));
        frame.render_widget(
            Paragraph::new(format!(" {}{after}", truncate(&title, width))).style(style),
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
                n!("Nothing here yet."),
                me.as_deref(),
                t,
            ),
            Rows::Notifications(l) => draw_notification_list(frame, list, l, images, t),
        }
    }
}

/// The Timeline tab without columns: the following timeline. The pinned
/// feeds are shown in columns, added with `+`.
pub(super) fn draw_timeline(
    frame: &mut Frame,
    body: Rect,
    app: &mut App,
    images: &mut Images,
    t: &Theme,
) {
    draw_posts(
        frame,
        body,
        &mut app.timeline,
        images,
        n!("No posts from accounts you follow yet. Press R to refresh."),
        None,
        t,
    );
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
            Span::styled(format!(" {} ", i18n::t("Thread")), t.selected()),
            Span::styled(format!("  esc {}", i18n::t("back")), t.dim()),
        ]),
        title,
    );
    if let Some(e) = &th.error {
        frame.render_widget(
            wrapped(
                &format!(
                    " {}  {}",
                    i18n::tf("could not load the thread: {}", &[&e.clone()]),
                    i18n::t("(R to retry)")
                ),
                list.width,
            )
            .style(t.error()),
            list,
        );
        return;
    }
    draw_posts(
        frame,
        list,
        &mut th.list,
        images,
        n!("The thread is empty."),
        me,
        t,
    );
}

/// What a notification says its author did.
pub(super) fn notif_action(reason: &str) -> String {
    let what = match reason {
        "like" => i18n::t("liked your post"),
        "repost" => i18n::t("reposted your post"),
        "follow" => i18n::t("followed you"),
        "mention" => i18n::t("mentioned you"),
        "reply" => i18n::t("replied to you"),
        "quote" => i18n::t("quoted your post"),
        "like-via-repost" => i18n::t("liked your repost"),
        "repost-via-repost" => i18n::t("reposted your repost"),
        "starterpack-joined" => i18n::t("joined through your starter pack"),
        "verified" => i18n::t("verified you"),
        "unverified" => i18n::t("removed your verification"),
        "subscribed-post" => i18n::t("made a new post"),
        other => return format!("({other})"),
    };
    what.to_string()
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
    let messages = (n!("loading…"), n!("No notifications yet."));
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
    let [bar, input, _, results] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas(area);
    let mode = app.search.mode;
    let tab = |m: SearchMode, label: &'static str| {
        let label = i18n::t(label);
        if m == mode {
            Span::styled(format!(" {label} "), t.selected())
        } else {
            Span::styled(format!(" {label} "), t.dim())
        }
    };
    frame.render_widget(
        Line::from(vec![
            Span::raw(" "),
            tab(SearchMode::Posts, n!("Posts")),
            Span::raw(" "),
            tab(SearchMode::Accounts, n!("Accounts")),
        ]),
        bar,
    );
    let prompt = format!(" {} ", i18n::t("search:"));
    let prompt_w = crate::tui::text::cells(&prompt) as u16;
    let field = Rect {
        x: input.x + prompt_w,
        width: input.width.saturating_sub(prompt_w + 1),
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
        (true, false) => frame.render_widget(
            Paragraph::new(i18n::t("press / or i to type")).style(t.dim()),
            field,
        ),
        (true, true) => {
            frame.render_widget(
                Paragraph::new(i18n::t("type, then enter")).style(t.dim()),
                field,
            );
            frame.set_cursor_position(Position::new(field.x, field.y));
        }
        _ => draw_single_input(frame, field, &app.search.input, app.search.editing),
    }
    match mode {
        SearchMode::Posts => {
            if app.search.posts.loaded || !app.search.posts.items.is_empty() || app.pending > 0 {
                let me = app.session.as_ref().map(|s| s.did.as_str());
                draw_posts(
                    frame,
                    results,
                    &mut app.search.posts,
                    images,
                    n!("No posts found."),
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
            Some(e) => wrapped(
                &format!(
                    " {}  {}",
                    i18n::tf("could not load the profile: {}", &[e]),
                    i18n::t("(R to retry)")
                ),
                area.width,
            )
            .style(t.error()),
            None => wrapped(&format!(" {}", i18n::t("loading…")), area.width).style(t.dim()),
        };
        frame.render_widget(msg, area);
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
            Span::raw(format!(" {}  ", i18n::t("followers"))),
            Span::styled(
                format!("{}", p.follows_count.unwrap_or(0)),
                Style::new().bold(),
            ),
            Span::raw(format!(" {}  ", i18n::t("following"))),
            Span::styled(
                format!("{}", p.posts_count.unwrap_or(0)),
                Style::new().bold(),
            ),
            Span::raw(format!(" {}", i18n::t("posts"))),
        ]),
    ];
    let own = app.profile.actor.is_none();
    if !own {
        let viewer = p.viewer.clone().unwrap_or_default();
        let mut rel = vec![if viewer.following.is_some() {
            Span::styled(i18n::t("✓ following"), t.accent().bold())
        } else {
            Span::styled(i18n::t("not following"), t.dim())
        }];
        if viewer.followed_by.is_some() {
            rel.push(Span::styled(
                format!("  · {}", i18n::t("follows you")),
                t.dim(),
            ));
        }
        match &viewer.muted_by_list {
            Some(list) => rel.push(Span::styled(
                format!(
                    "  · {}",
                    i18n::tf("muted by the list {}", &[&truncate(&list.name, 24)])
                ),
                t.error(),
            )),
            None if viewer.muted => {
                rel.push(Span::styled(format!("  · {}", i18n::t("muted")), t.error()));
            }
            None => {}
        }
        if viewer.blocking.is_some() {
            rel.push(Span::styled(
                format!("  · {}", i18n::t("blocked")),
                t.error(),
            ));
        }
        rel.push(Span::styled(
            format!("  {}", i18n::t("(f to toggle)")),
            t.dim(),
        ));
        lines.push(Line::from(rel));
    } else {
        // Buttons that say the keys exist: bsky does not read the mouse, so
        // they are pressed with the key they name.
        let button = |key: &'static str, what: &'static str| {
            [
                Span::styled("[ ", t.accent()),
                Span::styled(key, t.accent().bold()),
                Span::styled(format!(" {} ]", i18n::t(what)), t.accent()),
            ]
        };
        let mut row = vec![Span::styled(
            format!("{}  ", i18n::t("this is you")),
            t.dim(),
        )];
        row.extend(button("e", n!("Edit profile")));
        row.push(Span::raw("  "));
        row.extend(button("s", n!("Settings")));
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
        n!("No posts yet."),
        None,
        t,
    );
}
