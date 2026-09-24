//! A list of posts: the rows a post takes (header, text, pictures, quote, counts), and drawing them with the selection in sight.

use super::*;

/// Everything a post draws besides images, precomputed for a width.
pub(super) struct PostLines {
    /// The thread above a reply, one dim line per post.
    pub(super) context: Vec<Line<'static>>,
    pub(super) header: Line<'static>,
    pub(super) body: Vec<Line<'static>>,
    pub(super) images: Vec<String>,
    /// Rows the images take: enough for the tallest at its box width.
    pub(super) image_rows: u16,
    /// Whether an avatar is drawn beside it, which the row must be tall
    /// enough for.
    pub(super) avatar: bool,
    pub(super) stats: Line<'static>,
}

/// Width in cells of each image box when `n` images share `width` columns.
pub(super) fn image_box_width(width: u16, n: u16) -> u16 {
    if n == 0 || width < n {
        return 0;
    }
    ((width - (n - 1)) / n).min(IMAGE_MAX_W)
}

/// Rows an image of aspect `w:h` needs at `box_w` cells, given the cell size
/// in pixels, so a picture is drawn at its own shape instead of in a fixed
/// band with blank rows under it.
pub(super) fn image_rows(aspect: Option<(u32, u32)>, box_w: u16, cell: (u16, u16)) -> u16 {
    let Some((w, h)) = aspect else {
        return IMAGE_ROWS;
    };
    let (cw, ch) = (u64::from(cell.0.max(1)), u64::from(cell.1.max(1)));
    let px_h = u64::from(box_w) * cw * u64::from(h) / u64::from(w);
    let rows = px_h.div_ceil(ch);
    rows.clamp(u64::from(IMAGE_ROWS_MIN), u64::from(IMAGE_ROWS_MAX)) as u16
}

impl PostLines {
    /// `me` is the viewer's DID when the post should say whether its author
    /// is followed (a search, a thread); `None` where that goes without
    /// saying (the timeline is only followed accounts, a profile says it once).
    /// `cell` is the terminal's cell size in pixels, or `None` when it
    /// shows no pictures: then a line says what the post carries instead.
    fn new(post: &Post, width: u16, cell: Option<(u16, u16)>, me: Option<&str>, t: &Theme) -> Self {
        let width = usize::from(width.max(1));
        let record = post.record();
        let time = format_time(record.created_at.as_deref().unwrap_or(&post.indexed_at));
        let mut header = vec![
            Span::styled(post.author.name().to_string(), Style::new().bold()),
            Span::styled(format!(" @{}", post.author.handle), t.dim()),
        ];
        match me {
            Some(me) if me != post.author.did => {
                header.push(if post.author.following_uri().is_some() {
                    Span::styled(" ✓ following", t.accent())
                } else {
                    Span::styled(" not following", t.dim())
                })
            }
            _ => {}
        }
        header.push(Span::styled(format!(" · {time}"), t.dim()));
        if record.reply.is_some() {
            header.push(Span::styled(" ↩ reply", t.dim()));
        }
        let header = Line::from(header);
        let header = truncate_line(header, width);

        let mut body: Vec<Line<'static>> = wrap(&record.text, width)
            .into_iter()
            .map(Line::from)
            .collect();
        let mut images = Vec::new();
        let mut rows = 0;
        if let Some(embed) = &post.embed
            && cell.is_none()
        {
            // The media line names a video with its description.
            if !matches!(embed, Embed::Video { .. }) {
                body.extend(embed_lines(embed, width, t));
            }
            body.extend(media_line(embed, width, t));
        } else if let Some(embed) = &post.embed
            && let Some(cell) = cell
        {
            body.extend(embed_lines(embed, width, t));
            let shown: Vec<_> = embed.images().into_iter().take(4).collect();
            let box_w = image_box_width(width as u16, shown.len() as u16);
            rows = shown
                .iter()
                .map(|i| image_rows(i.aspect, box_w, cell))
                .max()
                .unwrap_or(0);
            images = shown.iter().map(|i| i.url.to_string()).collect();
        }
        let liked = post.like_uri().is_some();
        let heart = if liked { "♥" } else { "♡" };
        let heart_style = if liked { t.like() } else { t.dim() };
        let reposted = post.viewer.as_ref().is_some_and(|v| v.repost.is_some());
        let repost_style = if reposted { t.repost() } else { t.dim() };
        let stats = Line::from(vec![
            Span::styled(format!("{heart} {}", post.like_count), heart_style),
            Span::raw("   "),
            Span::styled(format!("⟳ {}", post.repost_count), repost_style),
            Span::styled(format!("   ↩ {}", post.reply_count), t.dim()),
        ]);
        let context = post
            .context
            .as_deref()
            .map(|c| context_lines(c, width, t))
            .unwrap_or_default();
        Self {
            context,
            header,
            body,
            images,
            image_rows: rows,
            avatar: cell.is_some(),
            stats,
        }
    }

    /// A row that stands in for a post that cannot be shown.
    fn placeholder(text: &'static str, t: &Theme) -> Self {
        Self {
            context: Vec::new(),
            header: Line::styled(text, t.dim()),
            body: Vec::new(),
            images: Vec::new(),
            image_rows: 0,
            avatar: false,
            stats: Line::default(),
        }
    }

    fn height(&self) -> u16 {
        let ctx = self.context.len() as u16;
        let content = ctx + 2 + self.body.len() as u16 + self.image_rows;
        let avatar = if self.avatar { ctx + AVATAR.1 } else { 0 };
        content.max(avatar) + 1
    }
}

/// The posts above a reply, each on one dim line: the root when the reply is
/// not directly under it, a gap when more of the thread sits in between, and
/// the parent.
pub(super) fn context_lines(c: &ReplyContext, width: usize, t: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if let Some(root) = &c.root {
        lines.push(ref_post_line(root, width, t));
        if c.gap {
            lines.push(Line::styled("┆ ⋮", t.dim()));
        }
    }
    lines.push(ref_post_line(&c.parent, width, t));
    lines
}

pub(super) fn ref_post_line(p: &RefPost, width: usize, t: &Theme) -> Line<'static> {
    let text = match p {
        RefPost::Post(post) => {
            let first = post.record().text.lines().next().unwrap_or("").to_string();
            format!("┆ {} @{}: {first}", post.author.name(), post.author.handle)
        }
        RefPost::NotFound { .. } => "┆ (post not found)".into(),
        RefPost::Blocked { .. } => "┆ (blocked post)".into(),
        RefPost::Other => "┆ (post not shown)".into(),
    };
    Line::styled(truncate(&text, width), t.dim())
}

/// `line` cut to `width` columns as one piece: whole grapheme clusters up
/// to the cut and one ellipsis there, in the style of the span it cuts.
pub(super) fn truncate_line(line: Line<'static>, width: usize) -> Line<'static> {
    let spans: Vec<(String, Style)> = line
        .spans
        .into_iter()
        .map(|s| (drawable(&s.content).into_owned(), s.style))
        .collect();
    if spans
        .iter()
        .map(|(t, _)| crate::tui::text::cells(t))
        .sum::<usize>()
        <= width
    {
        return Line::from(
            spans
                .into_iter()
                .map(|(t, s)| Span::styled(t, s))
                .collect::<Vec<_>>(),
        );
    }
    // One column is kept for the ellipsis.
    let mut left = width.saturating_sub(1);
    let mut out = Vec::new();
    for (text, style) in spans {
        let mut kept = String::new();
        for g in unicode_segmentation::UnicodeSegmentation::graphemes(text.as_str(), true) {
            let gw = g.width();
            if gw > left {
                if width > 0 {
                    kept.push('…');
                }
                out.push(Span::styled(kept, style));
                return Line::from(out);
            }
            kept.push_str(g);
            left -= gw;
        }
        out.push(Span::styled(kept, style));
    }
    Line::from(out)
}

/// What a post carries, for a terminal that cannot show it: the number of
/// pictures and their descriptions, or the video's.
pub(super) fn media_line(embed: &Embed, width: usize, t: &Theme) -> Option<Line<'static>> {
    let media = embed.media();
    let alts = |alt: &str| {
        let alt = alt.trim();
        (!alt.is_empty()).then(|| alt.to_string())
    };
    let text = match media.first()? {
        Media::Video { alt, .. } => match alts(alt) {
            Some(alt) => format!("▶ video: {alt}"),
            None => "▶ video".to_string(),
        },
        Media::Image { .. } => {
            let described: Vec<String> = media
                .iter()
                .filter_map(|m| match m {
                    Media::Image { alt, .. } => alts(alt),
                    Media::Video { .. } => None,
                })
                .collect();
            let n = media.len();
            let what = if n == 1 {
                "1 picture".to_string()
            } else {
                format!("{n} pictures")
            };
            if described.is_empty() {
                format!("▣ {what}")
            } else {
                format!("▣ {what}: {}", described.join(" · "))
            }
        }
    };
    Some(Line::styled(truncate(&text, width), t.dim()))
}

pub(super) fn embed_lines(embed: &Embed, width: usize, t: &Theme) -> Vec<Line<'static>> {
    let dim = t.dim();
    match embed {
        Embed::External { external } => {
            let title = if external.title.is_empty() {
                &external.uri
            } else {
                &external.title
            };
            vec![
                Line::styled(truncate(&format!("🔗 {title}"), width), t.accent()),
                Line::styled(truncate(&external.uri, width), dim),
            ]
        }
        // A quote beside a picture is a recordWithMedia, and says whose
        // post it quotes just as a plain quote does.
        Embed::Record { .. } | Embed::RecordWithMedia { .. } => embed
            .quoted()
            .map(|r| quote_line(r, width, t))
            .into_iter()
            .collect(),
        Embed::Video { .. } => vec![Line::styled("▶ video", dim)],
        Embed::Images { .. } | Embed::Other => Vec::new(),
    }
}

/// The one line a quote gets: who is quoted and the start of what they
/// wrote. A quote whose post cannot be shown says why, since a line that is
/// simply missing reads as a post that quotes nothing.
pub(super) fn quote_line(record: &serde_json::Value, width: usize, t: &Theme) -> Line<'static> {
    let kind = record
        .get("$type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let name = |p: &str| {
        record
            .pointer(p)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let text = match kind {
        "app.bsky.embed.record#viewNotFound" => "❝ quoted post not found".to_string(),
        "app.bsky.embed.record#viewBlocked" => {
            "❝ quoted post from an account you cannot see".to_string()
        }
        "app.bsky.embed.record#viewDetached" => "❝ quote removed by the post's author".to_string(),
        "app.bsky.feed.defs#generatorView" => format!("❝ feed: {}", name("/displayName")),
        "app.bsky.graph.defs#listView" => format!("❝ list: {}", name("/name")),
        "app.bsky.graph.defs#starterPackViewBasic" => {
            format!("❝ starter pack: {}", name("/record/name"))
        }
        _ => match record.pointer("/author/handle").and_then(|h| h.as_str()) {
            Some(handle) => {
                let said = name("/value/text");
                let first = said.lines().next().unwrap_or("");
                format!("❝ @{handle}: {first}")
            }
            None => "❝ quoted post cannot be shown".to_string(),
        },
    };
    Line::styled(truncate(&text, width), t.dim())
}

/// Width left for a row's text once its indentation is taken off.
pub(super) fn content_width<T: PostRow>(content: Rect, row: &T) -> u16 {
    content.width.saturating_sub(row.indent() * 2)
}

/// The content column of a list row, right of the marker and avatar.
pub(super) fn content_rect(area: Rect, avatars: bool) -> Rect {
    let left = if avatars {
        MARK_W + AVATAR.0 + 1
    } else {
        MARK_W + 1
    };
    Rect {
        x: area.x + left.min(area.width),
        width: area.width.saturating_sub(left),
        ..area
    }
}

pub(super) fn draw_marker(frame: &mut Frame, area: Rect, height: u16, selected: bool, t: &Theme) {
    if !selected {
        return;
    }
    let h = height.saturating_sub(1).min(area.height).max(1);
    let bar = vec![Line::from("▌"); usize::from(h)];
    frame.render_widget(
        Paragraph::new(bar).style(t.accent()),
        Rect {
            width: 1,
            height: h,
            ..area
        },
    );
}

/// Something drawn as one entry of a post list: a feed item, or a row of a
/// thread (indented, possibly a placeholder for a post that cannot be shown).
pub trait PostRow {
    fn post(&self) -> Option<&Post>;
    /// Reply depth, drawn as indentation with guide lines.
    fn indent(&self) -> u16 {
        0
    }
    /// What to show instead of a post that is not there.
    fn placeholder(&self) -> &'static str {
        "(post not shown)"
    }
}

impl PostRow for Post {
    fn post(&self) -> Option<&Post> {
        Some(self)
    }
}

impl PostRow for ThreadRow {
    fn post(&self) -> Option<&Post> {
        ThreadRow::post(self)
    }
    fn indent(&self) -> u16 {
        self.depth.min(MAX_INDENT)
    }
    fn placeholder(&self) -> &'static str {
        match self.kind {
            RowKind::Blocked(_) => "(blocked post)",
            _ => "(post not found)",
        }
    }
}

pub(super) fn row_lines<T: PostRow>(
    row: &T,
    width: u16,
    cell: Option<(u16, u16)>,
    me: Option<&str>,
    t: &Theme,
) -> PostLines {
    match row.post() {
        Some(post) => PostLines::new(post, width, cell, me, t),
        None => PostLines::placeholder(row.placeholder(), t),
    }
}

pub(super) fn draw_posts<T: PostRow>(
    frame: &mut Frame,
    area: Rect,
    list: &mut List<T>,
    images: &mut Images,
    empty: &str,
    me: Option<&str>,
    t: &Theme,
) {
    if !list.loaded {
        frame.render_widget(Paragraph::new(" loading…").style(t.dim()), area);
        return;
    }
    if list.items.is_empty() {
        empty_message(frame, area, list, empty, t);
        return;
    }
    let content = content_rect(area, images.shows());
    let cell = images.shows().then(|| images.cell_size());
    // Lay out only what this frame can need: the posts above the selection
    // that fit with it on screen, then as many as fill the screen. A long,
    // paged list costs no more than a short one.
    let mut lines: HashMap<usize, PostLines> = HashMap::new();
    let selected = list.selected.min(list.items.len() - 1);
    let items = &list.items;
    list.offset = scroll_offset(selected, list.offset, list.items.len(), area.height, |i| {
        lines
            .entry(i)
            .or_insert_with(|| row_lines(&items[i], content_width(content, &items[i]), cell, me, t))
            .height()
    });

    let mut y = area.y;
    let mut below = list.offset;
    for (i, item) in list.items.iter().enumerate().skip(list.offset) {
        if y >= area.bottom() {
            break;
        }
        let pl = lines
            .entry(i)
            .or_insert_with(|| row_lines(item, content_width(content, item), cell, me, t));
        let h = pl.height();
        let visible = (area.bottom() - y).min(h);
        let row = Rect {
            y,
            height: visible,
            ..area
        };
        let whole = visible == h;
        // A post cut off at the bottom draws no pictures, so it is the first
        // one to get ready.
        below = if whole { i + 1 } else { i };
        draw_marker(frame, row, h, i == list.selected, t);

        // Replies are indented, with a guide line for each level.
        let indent = item.indent() * 2;
        for level in 0..item.indent() {
            let guide = vec![Line::from("│"); usize::from(visible.saturating_sub(1).max(1))];
            let x = area.x + MARK_W + level * 2;
            frame.render_widget(
                Paragraph::new(guide).style(t.dim()),
                Rect {
                    x,
                    y,
                    width: 1,
                    height: visible,
                }
                .intersection(area),
            );
        }
        if let Some(url) = item.post().and_then(|p| p.author.avatar.as_ref())
            && whole
        {
            // Beside the post itself, below the thread lines above it.
            let avatar = Rect {
                x: area.x + MARK_W + indent,
                y: y + pl.context.len() as u16,
                width: AVATAR.0,
                height: AVATAR.1,
            };
            images.draw(frame, avatar, &small_avatar(url));
        }
        let content = Rect {
            x: content.x + indent.min(content.width),
            width: content.width.saturating_sub(indent),
            ..content
        };
        let c = Rect {
            y,
            height: visible,
            ..content
        };
        let mut text: Vec<Line> = pl.context.clone();
        text.push(pl.header.clone());
        text.extend(pl.body.iter().cloned());
        let body_rows = text.len() as u16;
        frame.render_widget(Paragraph::new(text), c);

        let mut next = y + body_rows;
        if !pl.images.is_empty() {
            let img_area = Rect {
                y: next,
                height: pl.image_rows,
                ..content
            };
            if whole {
                draw_image_row(frame, img_area, &pl.images, images);
            }
            next += pl.image_rows;
        }
        if next < area.bottom() {
            frame.render_widget(
                pl.stats.clone(),
                Rect {
                    y: next,
                    height: 1,
                    ..content
                },
            );
        }
        y += h;
    }
    for (i, item) in list.items.iter().enumerate().skip(below).take(PREFETCH) {
        let pl = lines
            .entry(i)
            .or_insert_with(|| row_lines(item, content_width(content, item), cell, me, t));
        if let Some(url) = item.post().and_then(|p| p.author.avatar.as_ref()) {
            images.prefetch(&small_avatar(url), AVATAR.0, AVATAR.1);
        }
        // The same box sizes draw_image_row gives them.
        let width = content.width.saturating_sub(item.indent() * 2);
        let each = image_box_width(width, pl.images.len() as u16);
        for url in &pl.images {
            images.prefetch(url, each, pl.image_rows);
        }
    }
    // Further on, only the downloads: the layout is not needed for them.
    for item in list.items.iter().skip(below + PREFETCH).take(WARM) {
        let Some(post) = item.post() else { continue };
        if let Some(url) = &post.author.avatar {
            images.warm(&small_avatar(url));
        }
        if let Some(embed) = &post.embed {
            for i in embed.images().into_iter().take(4) {
                images.warm(i.url);
            }
        }
    }
}

pub(super) fn draw_image_row(frame: &mut Frame, area: Rect, urls: &[String], images: &mut Images) {
    let each = image_box_width(area.width, urls.len() as u16);
    if each == 0 {
        return;
    }
    for (i, url) in urls.iter().enumerate() {
        let x = area.x + i as u16 * (each + 1);
        images.draw(
            frame,
            Rect {
                x,
                width: each,
                ..area
            },
            url,
        );
    }
}

pub(super) fn account_lines(p: &Profile, width: u16, t: &Theme) -> Vec<Line<'static>> {
    let width = usize::from(width.max(1));
    let mut head = vec![
        Span::styled(p.name().to_string(), Style::new().bold()),
        Span::styled(format!(" @{}", p.handle), t.dim()),
    ];
    if p.following_uri().is_some() {
        head.push(Span::styled("  ✓ following", t.accent()));
    }
    let desc = p
        .description
        .as_deref()
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    vec![
        truncate_line(Line::from(head), width),
        Line::styled(truncate(&desc, width), t.dim()),
    ]
}

pub(super) fn draw_accounts(
    frame: &mut Frame,
    area: Rect,
    list: &mut List<Profile>,
    images: &mut Images,
    t: &Theme,
) {
    let messages = (" searching…", " No accounts found.");
    draw_two_line_rows(
        frame,
        area,
        list,
        images,
        messages,
        t,
        |p| p.avatar.as_deref(),
        |p, w| account_lines(p, w, t),
    );
}

/// A list whose entries are an avatar beside two lines of text.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_two_line_rows<T>(
    frame: &mut Frame,
    area: Rect,
    list: &mut List<T>,
    images: &mut Images,
    (loading, empty): (&str, &str),
    t: &Theme,
    avatar: impl Fn(&T) -> Option<&str>,
    lines: impl Fn(&T, u16) -> Vec<Line<'static>>,
) {
    if !list.loaded {
        frame.render_widget(Paragraph::new(loading).style(t.dim()), area);
        return;
    }
    if list.items.is_empty() {
        empty_message(frame, area, list, empty, t);
        return;
    }
    const H: u16 = 3;
    list.offset = scroll_offset(
        list.selected,
        list.offset,
        list.items.len(),
        area.height,
        |_| H,
    );
    let content = content_rect(area, images.shows());
    let mut y = area.y;
    for (i, item) in list.items.iter().enumerate().skip(list.offset) {
        if y + H > area.bottom() {
            break;
        }
        let row = Rect {
            y,
            height: H,
            ..area
        };
        draw_marker(frame, row, H, i == list.selected, t);
        if let Some(url) = avatar(item) {
            let r = Rect {
                x: area.x + MARK_W,
                y,
                width: AVATAR.0,
                height: AVATAR.1,
            };
            images.draw(frame, r, &small_avatar(url));
        }
        frame.render_widget(
            Paragraph::new(lines(item, content.width)),
            Rect {
                y,
                height: 2,
                ..content
            },
        );
        y += H;
    }
    // The rows below the screen: their avatars are ready when scrolled to.
    let shown = usize::from(area.height / H);
    for (n, item) in list
        .items
        .iter()
        .skip(list.offset + shown)
        .take(WARM)
        .enumerate()
    {
        if let Some(url) = avatar(item) {
            let url = small_avatar(url);
            if n < PREFETCH * 2 {
                images.prefetch(&url, AVATAR.0, AVATAR.1);
            } else {
                images.warm(&url);
            }
        }
    }
}
