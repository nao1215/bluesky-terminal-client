use super::*;

#[test]
fn hints_wrap_whole_onto_more_rows_when_narrow() {
    let t = THEMES[0];
    let hints: Vec<keys::Hint> = vec![
        ("?", "help"),
        ("j k", "move"),
        ("enter", "profile"),
        ("q", "quit"),
    ];
    let text = |lines: &[Line]| -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    };
    assert_eq!(
        text(&hint_lines(&hints, 80, &t)),
        [" ? help  j k move  enter profile  q quit"]
    );
    assert_eq!(
        text(&hint_lines(&hints, 20, &t)),
        [" ? help  j k move", " enter profile", " q quit"]
    );
}

/// The hints are one row, whatever the width: a narrow screen keeps the
/// ones that fit, whole, starting with the help key and the list of
/// what can be done here, which lead to all the others.
#[test]
fn the_hint_row_is_one_line_and_starts_with_the_keys_that_lead_on() {
    let (mut app, _) = App::new(Some(session()), "x");
    app.handle_event(Event::Timeline(Ok(posts(3).into())));
    for width in [MIN_W, 40, 50, 80, 120] {
        let screen = render(&mut app, width, 24);
        let rows: Vec<&str> = screen.lines().filter(|l| l.contains("? help")).collect();
        assert_eq!(rows.len(), 1, "{width}:\n{screen}");
        assert!(
            rows[0].trim_start().starts_with("? help"),
            "{width}: {rows:?}"
        );
        if width >= 40 {
            assert!(rows[0].contains(". actions"), "{width}: {rows:?}");
        }
    }
    // On a wide screen every hint of the view is on that one row.
    let screen = render(&mut app, 120, 24);
    for (key, what) in keys::hints(&app) {
        assert!(
            screen.contains(&format!("{key} {what}")),
            "{key} {what}:\n{screen}"
        );
    }
}

// A header is cut as one line: one ellipsis where it stops, even when
// a wide character leaves a single column, and one whenever anything
// after the cut is dropped.
#[rstest::rstest]
#[case(&["日本語", " @alice"], 4, "日…")]
#[case(&["日本語", " @alice"], 5, "日本…")]
#[case(&["Alice", " @alice"], 5, "Alic…")]
#[case(&["Alice", " @alice"], 12, "Alice @alice")]
#[case(&["👨‍👩‍👧👨‍👩‍👧", " @a"], 3, "👨‍👩‍👧…")]
#[case(&["🇯🇵 name", " · 2026"], 8, "🇯🇵 name…")]
fn a_cut_header_ends_in_one_ellipsis(
    #[case] spans: &[&str],
    #[case] width: usize,
    #[case] want: &str,
) {
    let line = Line::from(
        spans
            .iter()
            .map(|s| Span::raw(s.to_string()))
            .collect::<Vec<_>>(),
    );
    let cut = truncate_line(line, width);
    let text: String = cut.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(text, want);
    assert!(text.width() <= width, "{text:?} is wider than {width}");
}

#[test]
fn a_long_path_keeps_its_end() {
    assert_eq!(truncate_start("/home/me/pics", 20), "/home/me/pics");
    assert_eq!(truncate_start("/home/me/pictures/trips", 10), "…res/trips");
    // Wide characters count as two columns.
    assert_eq!(truncate_start("/ホーム/写真", 6), "…/写真");
    assert_eq!(truncate_start("/ホーム/写真", 5), "…写真");
}

#[test]
fn a_long_path_is_not_cut_inside_a_grapheme_cluster() {
    assert_eq!(truncate_start("/pics/👍🏽👍🏽", 3), "…👍🏽");
    assert_eq!(truncate_start("/pics/👨‍👩‍👧", 3), "…👨‍👩‍👧");
    assert_eq!(truncate_start("/pics/🇯🇵🇯🇵", 3), "…🇯🇵");
}

/// Time to draw one frame of the timeline while `j` walks down 200 posts,
/// the cost a held key pays per step. Prints the median and the slowest.
#[test]
#[ignore = "measurement"]
#[cfg(not(coverage))]
fn frame_time() {
    let mut runs = Vec::new();
    for _ in 0..5 {
        let (mut app, _) = App::new(Some(session()), "x");
        app.handle_event(Event::Timeline(Ok(heavy_posts(200).into())));
        let mut images = Images::new(Picker::halfblocks(), None);
        let mut term = Terminal::new(TestBackend::new(120, 50)).unwrap();
        let mut times = Vec::new();
        for _ in 0..500 {
            app.handle_key(crossterm::event::KeyEvent::from(
                crossterm::event::KeyCode::Char(if times.len() % 250 < 199 { 'j' } else { 'k' }),
            ));
            let start = std::time::Instant::now();
            term.draw(|f| draw(f, &mut app, &mut images)).unwrap();
            times.push(start.elapsed());
        }
        times.sort();
        runs.push((times[times.len() / 2], times[times.len() - 1]));
    }
    for (median, max) in runs {
        println!("frame: median {median:?}, max {max:?}");
    }
}
