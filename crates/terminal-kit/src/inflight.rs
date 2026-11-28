//! Work a client must not drop when the user quits: a write to the server
//! sent just before `q` is counted while it is on its way, and quitting
//! waits for it, but never longer than a limit.

use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Duration;

/// How many jobs are on their way. Clones count the same jobs: the UI
/// starts them and the worker threads finish them.
#[derive(Debug, Clone, Default)]
pub struct InFlight(Arc<(Mutex<usize>, Condvar)>);

impl InFlight {
    /// A job was sent.
    pub fn start(&self) {
        *self.0.0.lock().unwrap_or_else(PoisonError::into_inner) += 1;
    }

    /// A job sent with [`InFlight::start`] is done, whether it worked or
    /// not, or will never run.
    pub fn done(&self) {
        let (left, cv) = &*self.0;
        let mut left = left.lock().unwrap_or_else(PoisonError::into_inner);
        *left = left.saturating_sub(1);
        cv.notify_all();
    }

    /// Wait until every job started is done, for at most `limit`, so a
    /// server that never answers does not keep the client open. Returns at
    /// once when none is on its way.
    pub fn wait(&self, limit: Duration) {
        let (left, cv) = &*self.0;
        let left = left.lock().unwrap_or_else(PoisonError::into_inner);
        #[expect(
            clippy::let_underscore_must_use,
            reason = "quitting goes on either way; the limit is the whole point"
        )]
        let _ = cv
            .wait_timeout_while(left, limit, |n| *n > 0)
            .unwrap_or_else(PoisonError::into_inner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn nothing_on_its_way_is_not_waited_for() {
        let start = Instant::now();
        InFlight::default().wait(Duration::from_secs(5));
        assert!(start.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn a_job_on_another_thread_is_waited_for_until_it_is_done() {
        let jobs = InFlight::default();
        jobs.start();
        let worker = jobs.clone();
        let finished = Arc::new(Mutex::new(false));
        let mark = Arc::clone(&finished);
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            *mark.lock().unwrap() = true;
            worker.done();
        });
        jobs.wait(Duration::from_secs(5));
        assert!(*finished.lock().unwrap());
        t.join().unwrap();
    }

    #[test]
    fn a_job_that_never_ends_holds_for_the_limit_only() {
        let jobs = InFlight::default();
        jobs.start();
        let start = Instant::now();
        jobs.wait(Duration::from_millis(200));
        let took = start.elapsed();
        assert!(
            took >= Duration::from_millis(200) && took < Duration::from_secs(2),
            "{took:?}"
        );
    }

    #[test]
    fn done_more_often_than_started_does_not_go_below_none() {
        let jobs = InFlight::default();
        jobs.done();
        jobs.start();
        jobs.done();
        let start = Instant::now();
        jobs.wait(Duration::from_secs(5));
        assert!(start.elapsed() < Duration::from_millis(100));
    }
}
