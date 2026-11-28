use super::*;
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::thread;

/// A local server with explicit request and reply barriers; tests can hold
/// a request in flight without relying on sleeps to race the worker.
fn server() -> (String, Receiver<(String, std::net::TcpStream)>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = channel();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            let len = stream.read(&mut buf).unwrap_or(0);
            let path = String::from_utf8_lossy(&buf[..len])
                .split_whitespace()
                .nth(1)
                .unwrap_or("")
                .to_string();
            if tx.send((path, stream)).is_err() {
                return;
            }
        }
    });
    (base, rx)
}

fn reply(mut stream: std::net::TcpStream, body: &[u8]) {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .unwrap();
    stream.write_all(body).unwrap();
}

fn next(rx: &Receiver<(String, std::net::TcpStream)>, path: &str) -> std::net::TcpStream {
    let (actual, stream) = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(actual, path);
    stream
}

fn evicted(cache: &Cache) {
    // Wait for the worker's notification, with a deadline shorter than the
    // five-second HTTP timeout: timeout expiry cannot pass as cancellation.
    let (state, _) = cache
        .0
        .changed
        .wait_timeout_while(cache.0.lock(), Duration::from_secs(2), |state| {
            !state.entries.is_empty()
        })
        .unwrap();
    assert!(
        state.entries.is_empty(),
        "cancelled worker did not release its slot"
    );
}

const MASTER: &[u8] = b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\nv/media.m3u8\n";
const MEDIA: &[u8] = b"#EXTM3U\n#EXTINF:1,\nseg0.ts\n#EXT-X-ENDLIST\n";
const SEGMENT: &[u8] = include_bytes!("../../../../e2e/atago/testdata/hls/v/seg0.ts");

#[test]
fn playback_reuses_both_playlists_and_first_segment_even_while_reading_and_on_replay() {
    let (base, requests) = server();
    let url = format!("{base}/master.m3u8");
    let cache = Cache::new();
    cache.update(std::slice::from_ref(&url));
    let master = next(&requests, "/master.m3u8");
    let (ready_tx, ready_rx) = channel();
    thread::scope(|scope| {
        scope.spawn(|| {
            ready_tx
                .send(cache.get(&url, &AtomicBool::new(false)).unwrap())
                .unwrap()
        });
        assert!(ready_rx.recv_timeout(Duration::from_millis(30)).is_err());
        reply(master, MASTER);
        reply(next(&requests, "/v/media.m3u8"), MEDIA);
        reply(next(&requests, "/v/seg0.ts"), SEGMENT);
        let ready = ready_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        assert_eq!(ready.first.as_deref(), Some(SEGMENT));
        for _ in 0..2 {
            cache.update(std::slice::from_ref(&url));
            let cached = cache.get(&url, &AtomicBool::new(false)).unwrap().unwrap();
            assert!(
                Arc::ptr_eq(&ready, &cached),
                "replay borrows the same bytes"
            );
            let (tx, rx) = channel();
            assert_eq!(
                super::super::play_ready(
                    &ratatui_image::picker::Picker::halfblocks(),
                    &url,
                    &AtomicBool::new(false),
                    &Mutex::new((20, 10)),
                    &tx,
                    Some(cached),
                ),
                Ok(())
            );
            assert!(
                rx.try_iter().count() >= 8,
                "the cached segment actually decodes"
            );
            assert!(requests.try_recv().is_err(), "no second HTTP request");
        }
        let weak = Arc::downgrade(&ready);
        drop(ready);
        cache.update(&[]);
        assert!(cache.0.lock().entries.is_empty());
        assert!(
            weak.upgrade().is_none(),
            "eviction releases the cached bytes"
        );
    });
}

#[rstest::rstest]
#[case(false)]
#[case(true)]
fn cancelling_read_ahead_interrupts_the_socket_and_a_late_result_cannot_return(
    #[case] in_body: bool,
) {
    let (base, requests) = server();
    let url = format!("{base}/old.m3u8");
    let cache = Cache::new();
    cache.update(std::slice::from_ref(&url));
    let mut old = next(&requests, "/old.m3u8");
    if in_body {
        reply(old, MEDIA);
        old = next(&requests, "/seg0.ts");
        old.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\nx")
            .unwrap();
    }
    let job = Arc::clone(&cache.0.lock().entries[0].job);
    // Set before the client shuts the socket: macOS refuses SO_RCVTIMEO with
    // EINVAL on a socket its peer has already reset.
    old.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    // Hold either the response headers or the rest of the segment body.
    // Eviction must release the read without waiting for its HTTP timeout.
    cache.update(&[]);
    assert!(job.cancelled.load(Ordering::Relaxed));
    // Shutting down with response bytes still in flight can reset TCP,
    // rather than close it gracefully. Either must stop the server's read.
    match old.read(&mut [0; 1]) {
        Ok(0) => {}
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
            ) => {}
        other => panic!("cancelled socket is still open: {other:?}"),
    }
    evicted(&cache);
    assert!(cache.get(&url, &AtomicBool::new(false)).unwrap().is_none());
    assert!(requests.try_recv().is_err());
    // Returning to the same URL creates a new job, not the cancelled one.
    cache.update(std::slice::from_ref(&url));
    let new = next(&requests, "/old.m3u8");
    reply(new, MEDIA);
    reply(next(&requests, "/seg0.ts"), SEGMENT);
    assert!(cache.get(&url, &AtomicBool::new(false)).unwrap().is_some());
}

#[test]
fn slow_fragmented_headers_and_body_survive_receive_polls() {
    let (base, requests) = server();
    let url = format!("{base}/slow.m3u8");
    let cache = Cache::new();
    cache.update(std::slice::from_ref(&url));
    let mut master = next(&requests, "/slow.m3u8");
    let (done, finished) = channel();
    thread::scope(|scope| {
        scope.spawn(|| done.send(cache.get(&url, &AtomicBool::new(false))).unwrap());
        master.write_all(b"HTTP/1.1 200").unwrap();
        assert!(finished.recv_timeout(Duration::from_millis(300)).is_err());
        write!(
            master,
            " OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            MEDIA.len()
        )
        .unwrap();
        master.write_all(MEDIA).unwrap();
        let mut segment = next(&requests, "/seg0.ts");
        write!(
            segment,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
            SEGMENT.len()
        )
        .unwrap();
        segment.write_all(&SEGMENT[..1]).unwrap();
        assert!(finished.recv_timeout(Duration::from_millis(300)).is_err());
        segment.write_all(&SEGMENT[1..]).unwrap();
        let ready = finished
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(ready.first.as_deref(), Some(SEGMENT));
        assert_eq!(ready.media.as_bytes(), MEDIA);
        assert!(
            requests.try_recv().is_err(),
            "polls must not restart requests"
        );
    });
}

#[test]
fn count_and_bytes_are_bounded_and_oversized_bodies_are_not_cached() {
    let (base, requests) = server();
    let urls: Vec<_> = (0..20).map(|i| format!("{base}/{i}.m3u8")).collect();
    let cache = Cache::new();
    cache.update(&urls);
    let held: Vec<_> = (0..KEPT)
        .map(|_| requests.recv_timeout(Duration::from_secs(5)).unwrap())
        .collect();
    assert_eq!(cache.0.lock().wanted.len(), KEPT);
    assert_eq!(cache.0.lock().entries.len(), KEPT);
    assert!(requests.try_recv().is_err());
    // No Content-Length: the reader must enforce its own byte limit.
    let (path, mut oversized) = held.into_iter().next().unwrap();
    oversized
        .write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n")
        .unwrap();
    oversized
        .write_all(&vec![b'x'; PLAYLIST_BYTES + 1])
        .unwrap();
    drop(oversized);
    assert!(
        cache
            .get(&format!("{base}{path}"), &AtomicBool::new(false))
            .unwrap()
            .is_none()
    );
    cache.update(&[]);
    evicted(&cache);

    let url = format!("{base}/large.m3u8");
    cache.update(std::slice::from_ref(&url));
    reply(next(&requests, "/large.m3u8"), MEDIA);
    let mut segment = next(&requests, "/seg0.ts");
    write!(
        segment,
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
        SEGMENT_BYTES + 1
    )
    .unwrap();
    // Reject by the declared size, without reading the oversized body.
    let ready = cache.get(&url, &AtomicBool::new(false)).unwrap().unwrap();
    assert!(ready.first.is_none());
    assert_eq!(
        ready.media.as_bytes(),
        MEDIA,
        "playlists can still be reused"
    );
    cache.update(&[]);
    assert!(cache.0.lock().entries.is_empty());
}

#[test]
fn stopping_playback_does_not_wait_for_a_background_download() {
    let (base, requests) = server();
    let url = format!("{base}/master.m3u8");
    let cache = Cache::new();
    cache.update(std::slice::from_ref(&url));
    let _held = next(&requests, "/master.m3u8");
    let stop = AtomicBool::new(false);
    let (tx, rx) = channel();
    thread::scope(|scope| {
        scope.spawn(|| tx.send(cache.get(&url, &stop).is_err()).unwrap());
        assert!(rx.recv_timeout(Duration::from_millis(30)).is_err());
        stop.store(true, Ordering::Relaxed);
        assert!(rx.recv_timeout(Duration::from_secs(1)).unwrap());
    });
}
