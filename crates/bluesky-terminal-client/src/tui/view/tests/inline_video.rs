use super::*;
use ratatui::buffer::Buffer;
use ratatui_image::picker::ProtocolType;
use ratatui_image::protocol::Protocol;
use std::cell::RefCell;
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};
use terminal_kit::media::player::{Pacer, Video};

thread_local! {
    static EVENTS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

struct Clip {
    url: String,
    generation: u32,
    frame: Box<Protocol>,
}

impl Video for Clip {
    fn is(&self, url: &str, generation: u32) -> bool {
        self.url == url && self.generation == generation
    }

    fn resize(&self, _: (u16, u16)) {}

    fn poll(&mut self) -> bool {
        false
    }

    fn frame(&self) -> Option<&Protocol> {
        Some(&self.frame)
    }

    fn state(&self) -> &State {
        &State::Playing
    }
}

impl Drop for Clip {
    fn drop(&mut self) {
        EVENTS.with_borrow_mut(|events| events.push(format!("stop {}", self.url)));
    }
}

fn start(picker: Picker, url: &str, size: (u16, u16), generation: u32) -> Box<dyn Video> {
    EVENTS.with_borrow_mut(|events| events.push(format!("start {url}")));
    let (tx, rx) = mpsc::channel();
    let size = Mutex::new(size);
    // The real encoder, including the video IDs used by kitty. Every new
    // player starts at frame zero, just as a decoder starting the clip does.
    let mut pacer = Pacer::new(&picker, &size, &tx);
    let picture = image::RgbImage::from_pixel(40, 40, image::Rgb([200, 20, 30]));
    assert!(pacer.show(Some(picture), Some(0)).unwrap());
    Box::new(Clip {
        url: url.into(),
        generation,
        frame: rx.recv().unwrap(),
    })
}

/// Observe the cells ratatui will actually emit, rather than the image
/// cache or TestBackend's screen (neither interprets graphics commands).
fn emitted(before: &Buffer, after: &Buffer, area: Rect) -> String {
    before
        .diff(after)
        .into_iter()
        .filter(|(x, y, _)| area.contains((*x, *y).into()))
        .map(|(_, _, cell)| cell.symbol())
        .collect()
}

#[rstest::rstest]
#[case(ProtocolType::Kitty, "\x1b_Gq=2,i=")]
#[case(ProtocolType::Sixel, "\x1bP")]
#[case(ProtocolType::Iterm2, "\x1b]1337;File=")]
fn stopping_inline_video_restores_thumbnails_in_the_next_frame(
    #[case] protocol: ProtocolType,
    #[case] transmission: &str,
) {
    EVENTS.with_borrow_mut(Vec::clear);
    let dir = tempfile::tempdir().unwrap();
    let paths = [dir.path().join("a.png"), dir.path().join("b.png")];
    for (path, shade) in paths.iter().zip([40, 80]) {
        image::RgbImage::from_pixel(40, 40, image::Rgb([shade; 3]))
            .save(path)
            .unwrap();
    }
    #[allow(
        deprecated,
        reason = "a deterministic cell size without querying a terminal"
    )]
    let mut picker = Picker::from_fontsize((10, 20).into());
    picker.set_protocol_type(protocol);
    let mut images = Images::new(picker, None).unwrap();
    images.set_player(start);
    let mut term = Terminal::new(TestBackend::new(4, 5)).unwrap();
    let boxes = [Rect::new(0, 0, 4, 2), Rect::new(0, 3, 4, 2)];
    let urls = paths.each_ref().map(|p| format!("file://{}", p.display()));

    // Register local files, while the cache still contains placeholders.
    term.draw(|f| {
        images.begin_frame(f.area().as_size(), Style::new());
        for (path, area) in paths.iter().zip(boxes) {
            images.draw_file(f, area, path);
        }
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        images.poll();
        for url in &urls {
            images.prefetch(url, 4, 2);
        }
        if !images.loading() {
            break;
        }
        assert!(Instant::now() < deadline, "thumbnail encoding timed out");
        std::thread::sleep(Duration::from_millis(2));
    }

    let mut draw = |selected: Option<usize>| {
        term.draw(|f| {
            images.begin_frame(f.area().as_size(), Style::new());
            for (i, (url, area)) in urls.iter().zip(boxes).enumerate() {
                draw_post_media(
                    f,
                    area,
                    std::slice::from_ref(url),
                    (selected == Some(i)).then_some(url.as_str()),
                    &mut images,
                );
            }
            // view::draw_screen stops after drawing the unselected posts.
            if selected.is_none() {
                images.stop_video();
            }
        })
        .unwrap()
        .buffer
        .clone()
    };
    // The first thumbnail becomes ready while its video is already ready
    // to cover it: its initial transmission must not be consumed unseen.
    let first = draw(Some(0));
    let switched = draw(Some(1));
    assert!(
        emitted(&first, &switched, boxes[0]).contains(transmission),
        "{protocol:?}: the first thumbnail must be transmitted on leaving its post"
    );
    assert!(emitted(&first, &switched, boxes[1]).contains(transmission));
    let back = draw(Some(0));
    assert!(emitted(&switched, &back, boxes[0]).contains(transmission));
    // Kitty can reuse B's already transmitted thumbnail; the other two
    // protocols must send its pixels again.
    let restored = emitted(&switched, &back, boxes[1]);
    assert!(restored.contains(if protocol == ProtocolType::Kitty {
        "\u{10eeee}"
    } else {
        transmission
    }));
    let stopped = draw(None);
    assert!(!emitted(&back, &stopped, boxes[0]).is_empty());
    let idle = draw(None);
    assert!(emitted(&stopped, &idle, boxes[0]).is_empty());
    assert!(emitted(&stopped, &idle, boxes[1]).is_empty());
    assert!(!images.video_shown());
    let events = EVENTS.with_borrow(Clone::clone);
    assert_eq!(events.iter().filter(|e| e.starts_with("start ")).count(), 3);
    for url in &urls {
        let count = if *url == urls[0] { 2 } else { 1 };
        assert_eq!(
            events
                .iter()
                .filter(|e| **e == format!("start {url}"))
                .count(),
            count
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| **e == format!("stop {url}"))
                .count(),
            count
        );
    }
}
