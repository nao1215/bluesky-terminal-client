//! Speculative threads and notification counts for the current account.
use super::*;
use crate::tui::worker::Thread;

const FRESH: Duration = Duration::from_secs(15);

#[derive(Default)]
pub(super) struct ReadAhead {
    dwell: terminal_kit::dwell::Dwell,
    reading: HashMap<u64, String>,
    kept: Vec<(String, Instant, Thread)>,
    notifications_at: Option<Instant>,
    seen: Option<String>,
    server_count: Option<usize>,
    marker_ready: bool,
    syncing: Option<u64>,
    marking: Option<(u64, String)>,
    failed_mark: Option<String>,
}

impl App {
    pub(super) fn notification_state(
        &mut self,
        seq: u64,
        result: Result<(Option<String>, Option<usize>)>,
    ) {
        if self.read_ahead.syncing != Some(seq) {
            return;
        }
        self.read_ahead.syncing = None;
        if let Ok((marker, count)) = result {
            self.read_ahead.marker_ready = true;
            self.read_ahead.seen = marker;
            self.read_ahead.server_count = count;
        }
        self.count_notifications();
    }

    pub(super) fn notifications_marked(&mut self, seq: u64, id: String, result: Result<()>) {
        if self.read_ahead.marking.as_ref().map(|(s, _)| *s) != Some(seq) {
            return;
        }
        self.read_ahead.marking = None;
        match result {
            Ok(()) => {
                self.read_ahead.seen = Some(id);
                self.read_ahead.server_count = None;
            }
            Err(e) => {
                self.read_ahead.failed_mark = Some(id);
                self.fail(e.message().to_string());
            }
        }
        self.count_notifications();
    }

    /// Background reads do not put a spinner on the selected timeline.
    pub(crate) fn poll_read_ahead(&mut self, now: Instant) -> Jobs {
        self.count_notifications();
        let mut jobs = Vec::new();
        if self.session.is_some()
            && self.notifications.waiting.is_none()
            && self
                .read_ahead
                .notifications_at
                .is_none_or(|at| now.saturating_duration_since(at) >= Duration::from_secs(60))
        {
            self.read_ahead.notifications_at = Some(now);
            jobs.extend(self.load_tab(Tab::Notifications, false));
            if self.read_ahead.syncing.is_none() && self.read_ahead.marking.is_none() {
                let job = self.job(Job::NotificationState);
                self.read_ahead.syncing = Some(job.0);
                self.read_ahead.failed_mark = None;
                jobs.push(job);
            }
        }
        if self.read_ahead.marker_ready
            && self.read_ahead.syncing.is_none()
            && self.read_ahead.marking.is_none()
            && self.tab == Tab::Notifications
            && self.overlay.is_none()
            && self.threads.is_empty()
            && let Some(id) = self
                .notifications
                .items
                .iter()
                .map(|n| &n.id)
                .max_by(|a, b| id_order(a, b))
                .cloned()
            && self
                .read_ahead
                .seen
                .as_ref()
                .is_none_or(|s| id_order(&id, s).is_gt())
            && self.read_ahead.failed_mark.as_ref() != Some(&id)
        {
            let job = self.job(Job::MarkNotifications { id: id.clone() });
            self.read_ahead.marking = Some((job.0, id));
            self.unread = 0;
            jobs.push(job);
        }
        let selected = (self.overlay.is_none()
            && !self.search.editing
            && self.threads.is_empty()
            && self.session.is_some()
            && self.tab != Tab::Chat)
            .then(|| self.selected().map(|s| s.id.clone()))
            .flatten();
        if !self
            .read_ahead
            .dwell
            .ready(selected.as_deref(), now, Duration::from_millis(400))
        {
            return jobs;
        }
        let Some(id) = selected else { return jobs };
        if self.read_ahead.reading.values().any(|s| s == &id)
            || self
                .read_ahead
                .kept
                .iter()
                .any(|(s, at, _)| s == &id && now.saturating_duration_since(*at) < FRESH)
        {
            return jobs;
        }
        let (seq, job) = self.job(Job::Thread { id: id.clone() });
        self.waiting.remove(&seq);
        self.read_ahead.reading.insert(seq, id);
        jobs.push((seq, job));
        jobs
    }

    pub(super) fn invalidate_read_ahead(&mut self) {
        self.read_ahead.kept.clear();
        for seq in self.read_ahead.reading.keys() {
            self.waiting.remove(seq);
            for th in &mut self.threads {
                if th.rows.waiting == Some(*seq) {
                    th.rows.waiting = None;
                }
            }
        }
        self.read_ahead.reading.clear();
        self.read_ahead.dwell = Default::default();
    }

    pub(super) fn read_ahead_answer(&mut self, seq: u64, ev: &Event) -> bool {
        let Some(id) = self.read_ahead.reading.remove(&seq) else {
            return false;
        };
        if let Event::Thread {
            result: Ok(thread), ..
        } = ev
        {
            self.read_ahead.kept.retain(|(s, _, _)| s != &id);
            if self.read_ahead.kept.len() == 8 {
                self.read_ahead.kept.remove(0);
            }
            self.read_ahead
                .kept
                .push((id, Instant::now(), *thread.clone()));
        }
        !self.waiting.contains(&seq)
    }

    pub(super) fn cached_thread(&mut self, id: &str) -> Option<(Thread, bool)> {
        let i = self.read_ahead.kept.iter().position(|(s, _, _)| s == id)?;
        let (_, at, thread) = self.read_ahead.kept.remove(i);
        Some((thread, at.elapsed() < FRESH))
    }

    pub(super) fn reading_thread(&self, id: &str) -> Option<u64> {
        self.read_ahead
            .reading
            .iter()
            .find_map(|(seq, s)| (s == id).then_some(*seq))
    }

    /// Only IDs newer than the newest notification viewed this run count.
    pub(super) fn count_notifications(&mut self) {
        let newest = self
            .notifications
            .items
            .iter()
            .map(|n| &n.id)
            .max_by(|a, b| id_order(a, b))
            .cloned();
        if !self.read_ahead.marker_ready
            && self.tab == Tab::Notifications
            && self.overlay.is_none()
            && self.threads.is_empty()
            && let Some(newest) = newest
            && self
                .read_ahead
                .seen
                .as_ref()
                .is_none_or(|seen| id_order(&newest, seen).is_gt())
        {
            self.read_ahead.seen = Some(newest);
        }
        if self.read_ahead.marking.is_some() {
            self.unread = 0;
            return;
        }
        if let Some(count) = self.read_ahead.server_count {
            self.unread = count;
            return;
        }
        self.unread = self
            .notifications
            .items
            .iter()
            .filter(|n| {
                self.read_ahead
                    .seen
                    .as_ref()
                    .is_none_or(|seen| id_order(&n.id, seen).is_gt())
            })
            .count();
    }
}

fn id_order(a: &str, b: &str) -> std::cmp::Ordering {
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}
