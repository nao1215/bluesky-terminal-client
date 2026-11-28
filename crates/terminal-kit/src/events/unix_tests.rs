//! A private PTY and subprocess isolate crossterm's global reader and signals.

use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fs::File;
use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd};
use std::process::Command;

#[test]
fn a_key_ready_with_a_resize_is_delivered_without_another_key() {
    const CHILD: &str = "TERMINAL_KIT_RESIZE_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let test = "events::unix_tests::a_key_ready_with_a_resize_is_delivered_without_another_key";
        for order in ["resize-first", "key-first"] {
            let output = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", test, "--nocapture"])
                .env(CHILD, order)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "PTY child ({order}) failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    }

    // SAFETY: this subprocess must not query the caller's controlling terminal.
    assert!(unsafe { libc::setsid() } >= 0);
    // SAFETY: dup returns new owned descriptors, wrapped exactly once.
    let (stdin, stdout) = unsafe {
        let stdin = libc::dup(0);
        let stdout = libc::dup(1);
        assert!(stdin >= 0 && stdout >= 0);
        (File::from_raw_fd(stdin), File::from_raw_fd(stdout))
    };
    let (mut master, mut slave) = (-1, -1);
    let size = libc::winsize {
        ws_row: 16,
        ws_col: 40,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: valid output pointers, no optional name or termios, valid size.
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null(),
                &size,
            )
        },
        0
    );
    // SAFETY: openpty returned two owned descriptors; each is wrapped once.
    let (mut master, slave) = unsafe { (File::from_raw_fd(master), File::from_raw_fd(slave)) };
    // SAFETY: only this test's subprocess changes its standard descriptors.
    // stdout is also the PTY so crossterm can obtain its size without /dev/tty.
    assert_eq!(unsafe { libc::dup2(slave.as_raw_fd(), 0) }, 0);
    assert_eq!(unsafe { libc::dup2(slave.as_raw_fd(), 1) }, 1);
    crossterm::terminal::enable_raw_mode().unwrap();
    let mut input = Input::new().unwrap();

    // Both descriptors are ready in the same poll, in either order. No sleeps or
    // further input to accidentally wake a reader that lost the key's edge.
    let mut queue_keys = || {
        master.write_all(b"jk").unwrap();
        // Wait for the line discipline to make the bytes readable without
        // consuming them or letting crossterm take either readiness event.
        let mut fd = libc::pollfd {
            fd: 0,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd, referring to the open PTY slave.
        assert_eq!(unsafe { libc::poll(&mut fd, 1, 1000) }, 1);
        assert_ne!(fd.revents & libc::POLLIN, 0);
    };
    let key_first = std::env::var(CHILD).unwrap() == "key-first";
    if key_first {
        queue_keys();
    }
    // SAFETY: Input::new has installed crossterm's SIGWINCH handler.
    assert_eq!(unsafe { libc::raise(libc::SIGWINCH) }, 0);
    if !key_first {
        queue_keys();
    }
    let events: Vec<_> = (0..3)
        .filter_map(|_| input.next(Duration::ZERO).unwrap())
        .collect();
    crossterm::terminal::disable_raw_mode().unwrap();
    // SAFETY: restore the saved descriptors before the harness prints results
    // and before closing the PTY master would make stdout return EIO.
    assert_eq!(unsafe { libc::dup2(stdin.as_raw_fd(), 0) }, 0);
    assert_eq!(unsafe { libc::dup2(stdout.as_raw_fd(), 1) }, 1);
    assert!(
        events.contains(&Event::Key(KeyEvent::new(
            KeyCode::Char('j'),
            KeyModifiers::NONE,
        ))),
        "key was lost: {events:?}"
    );
    assert!(
        events.contains(&Event::Key(KeyEvent::new(
            KeyCode::Char('k'),
            KeyModifiers::NONE,
        ))),
        "buffered key was lost: {events:?}"
    );
    assert!(
        events.contains(&Event::Resize(40, 16)),
        "resize was lost: {events:?}"
    );
}
