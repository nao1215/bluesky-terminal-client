//! Threads read ahead, as bsky's: when the selection rests on a note, its
//! thread is read before `v` asks for it, so `v` shows it at once instead
//! of after a round trip to the server.

use std::time::{Duration, Instant};

use super::*;
use crate::tui::worker::Thread;

/// How long the selection rests on a note before its thread is read:
/// longer than a held j stays on one, so scrolling past reads nothing.
pub(super) const REST: Duration = Duration::from_millis(400);
/// A thread read ahead this recently is shown as it is; an older one is
/// shown while it is read again.
pub(super) const FRESH: Duration = Duration::from_secs(15);
/// Threads kept read ahead.
const KEPT: usize = 8;

/// A thread read ahead.
#[derive(Debug, Clone)]
struct ReadThread {
    id: String,
    seq: u64,
    at: Instant,
    answer: std::result::Result<Thread, crate::error::Error>,
}

/// Where reading ahead is.
#[derive(Debug, Clone, Default)]
pub(crate) struct ReadAhead {
    /// The note the selection is on, since when, and whether its thread
    /// has been asked for while it rests there.
    resting: terminal_kit::dwell::Dwell,
    threads: Vec<ReadThread>,
    /// A thread read before this job may not show a write sent since.
    wrote_at: u64,
    /// Threads being read ahead now.
    reading: Vec<String>,
    /// Threads being read ahead that `v` opened meanwhile: the view waits
    /// for that answer, counted as pending, instead of asking again.
    adopted: Vec<String>,
}

impl ReadAhead {
    /// The job numbered `seq` writes: what was read before it may not
    /// show it.
    pub(crate) fn wrote(&mut self, seq: u64) {
        self.wrote_at = seq;
        self.threads.clear();
    }

    /// How many threads are kept read ahead.
    #[cfg(test)]
    pub(crate) fn kept(&self) -> usize {
        self.threads.len()
    }
}

impl App {
    /// Read ahead the thread of the note the selection has rested on for
    /// [`REST`], once while it rests there. The job is not counted as
    /// pending: the screen does not wait for it. Only on a list: the rows
    /// of a thread open are mostly that thread again.
    pub(crate) fn poll_read_ahead(&mut self, now: Instant) -> Jobs {
        let shown = if self.overlay.is_none()
            && !self.search.editing
            && self.threads.is_empty()
            && self.session.is_some()
        {
            self.shown_note().map(|n| n.id.clone())
        } else {
            None
        };
        if !self.read_ahead.resting.ready(shown.as_deref(), now, REST) {
            return Vec::new();
        }
        let Some(id) = shown else { return Vec::new() };
        let fresh = self
            .read_ahead
            .threads
            .iter()
            .any(|t| t.id == id && t.at.elapsed() < FRESH);
        if fresh || self.read_ahead.reading.contains(&id) {
            return Vec::new();
        }
        self.read_ahead.reading.push(id.clone());
        self.sent += 1;
        vec![(self.sent, Job::ReadAhead { id })]
    }

    /// A thread read ahead: kept, unless a write was sent after it was
    /// asked for or a login made it another account's; a view that waited
    /// for it takes it.
    pub(super) fn read_ahead_answer(
        &mut self,
        seq: u64,
        id: String,
        result: crate::error::Result<Box<Thread>>,
    ) -> Jobs {
        self.read_ahead.reading.retain(|r| *r != id);
        let adopted = self.read_ahead.adopted.contains(&id);
        self.read_ahead.adopted.retain(|a| *a != id);
        if adopted {
            self.pending = self.pending.saturating_sub(1);
        }
        if seq < self.read_ahead.wrote_at || seq < self.account_since {
            return Vec::new();
        }
        if adopted {
            // The view opened meanwhile is filled as if it had asked.
            if let Some(th) = self
                .threads
                .iter_mut()
                .find(|t| t.id == id && t.list.asked == 0)
            {
                match result {
                    Ok(t) => {
                        let (rows, at) = ThreadView::rows(*t);
                        th.list.set(rows);
                        th.list.cursor = None;
                        th.list.selected = at;
                    }
                    Err(e) => th.list.failed(said(&e)),
                }
            }
            return Vec::new();
        }
        let kept = &mut self.read_ahead.threads;
        kept.retain(|t| t.id != id);
        if kept.len() == KEPT {
            kept.remove(0);
        }
        kept.push(ReadThread {
            id,
            seq,
            at: Instant::now(),
            answer: result.map(|t| *t),
        });
        Vec::new()
    }

    /// Fill `view` from the thread read ahead for its note, if there is
    /// one: loaded when it is fresh, else shown while it is read again.
    /// Returns whether it still has to be read.
    pub(super) fn fill_from_read_ahead(&mut self, view: &mut ThreadView) -> bool {
        let (since, wrote) = (self.account_since, self.read_ahead.wrote_at);
        let kept = &mut self.read_ahead.threads;
        let Some(i) = kept
            .iter()
            .position(|t| t.id == view.id && t.seq >= since && t.seq >= wrote)
        else {
            if self.read_ahead.reading.contains(&view.id) {
                self.read_ahead.adopted.push(view.id.clone());
                self.pending += 1;
                view.list.begin();
                return false;
            }
            return true;
        };
        let t = kept.remove(i);
        let fresh = t.at.elapsed() < FRESH;
        match t.answer {
            Ok(thread) => {
                let (rows, at) = ThreadView::rows(thread);
                view.list.set(rows);
                view.list.cursor = None;
                view.list.selected = at;
                !fresh
            }
            // One that could not be read just now is read again.
            Err(_) => true,
        }
    }
}
