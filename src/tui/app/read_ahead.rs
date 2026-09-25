//! Threads read ahead: when the selection rests on a post, its thread is
//! read before `v` asks for it, so `v` shows it at once instead of after a
//! round trip to the server.

use std::time::{Duration, Instant};

use super::{App, ThreadView, thread_rows};
use crate::api::types::ThreadNode;
use crate::tui::worker::Job;

/// How long the selection rests on a post before its thread is read: longer
/// than a held j stays on one, so scrolling past posts reads nothing.
pub(super) const REST: Duration = Duration::from_millis(400);
/// A thread read ahead this recently is shown as it is; an older one is
/// shown while it is read again.
pub(super) const FRESH: Duration = Duration::from_secs(15);
/// Threads kept read ahead.
const KEPT: usize = 8;

/// A thread read ahead.
#[derive(Debug, Clone)]
pub(super) struct ReadThread {
    uri: String,
    /// The number of the job that read it.
    seq: u64,
    pub(super) at: Instant,
    node: ThreadNode,
}

/// Where reading ahead is.
#[derive(Debug, Clone, Default)]
pub(super) struct ReadAhead {
    /// The post the selection is on, since when, and whether its thread has
    /// been asked for while it rests there.
    resting: Option<(String, Instant, bool)>,
    /// The threads read ahead, the newest last.
    pub(super) threads: Vec<ReadThread>,
    /// A thread read before this job may not show a write sent since.
    wrote_at: u64,
}

impl ReadAhead {
    /// Note that the job numbered `seq` writes: what was read before it
    /// may not show it.
    pub(super) fn wrote(&mut self, seq: u64) {
        self.wrote_at = seq;
        self.threads.clear();
    }
}

impl App {
    /// Read ahead the thread of the post the selection has rested on for
    /// [`REST`], once while it rests there. The job is not counted as
    /// pending: the screen does not wait for it.
    pub fn poll_read_ahead(&mut self, now: Instant) -> Vec<Job> {
        let shown = if self.overlay.is_none() && self.login.is_none() {
            self.shown_post().or_else(|| self.subject_shown())
        } else {
            None
        };
        let Some(uri) = shown.map(|p| p.uri.clone()) else {
            self.read_ahead.resting = None;
            return Vec::new();
        };
        let open = self.threads.last().is_some_and(|t| t.uri == uri);
        match &mut self.read_ahead.resting {
            Some((on, since, asked)) if *on == uri => {
                if *asked || open || now.saturating_duration_since(*since) < REST {
                    return Vec::new();
                }
                *asked = true;
                vec![Job::ReadAhead(uri)]
            }
            _ => {
                self.read_ahead.resting = Some((uri, now, false));
                Vec::new()
            }
        }
    }

    /// Keep a thread read ahead, unless a write was sent after it was asked
    /// for or a login made it another account's.
    pub(super) fn read_ahead(&mut self, uri: String, node: ThreadNode) {
        let Some(seq) = self.answering else { return };
        if seq < self.read_ahead.wrote_at || seq < self.account_since {
            return;
        }
        let kept = &mut self.read_ahead.threads;
        kept.retain(|t| t.uri != uri);
        if kept.len() == KEPT {
            kept.remove(0);
        }
        kept.push(ReadThread {
            uri,
            seq,
            at: Instant::now(),
            node,
        });
    }

    /// Fill `view` from the thread read ahead for its post, if there is one:
    /// loaded when it is fresh, else shown while it is read again. Returns
    /// whether it still has to be read.
    pub(super) fn fill_from_read_ahead(&mut self, view: &mut ThreadView) -> bool {
        let kept = &mut self.read_ahead.threads;
        let Some(i) = kept.iter().position(|t| {
            t.uri == view.uri && t.seq >= self.account_since && t.seq >= self.read_ahead.wrote_at
        }) else {
            return true;
        };
        let t = kept.remove(i);
        let (rows, focus) = thread_rows::flatten(t.node);
        view.list.items = rows;
        view.list.selected = focus;
        view.list.loaded = t.at.elapsed() < FRESH;
        !view.list.loaded
    }
}
