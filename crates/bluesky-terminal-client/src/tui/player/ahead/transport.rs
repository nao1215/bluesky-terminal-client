//! Cancellation of reads must not depend on shutdown waking a pending recv.
//! In particular, observing FIN at the server does not prove that a Windows
//! client's pending read has returned. Keep shutdown for prompt socket cleanup,
//! and bound each receive wait so the worker can check cancellation itself.
//! See Rust's `close_read_wakes_up` test:
//! <https://github.com/rust-lang/rust/blob/1.90.0/library/std/src/net/tcp/tests.rs#L513-L540>.

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use ureq::unversioned::transport::{self, Buffers, ConnectionDetails, NextTimeout, Transport};

const POLL: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub(super) struct Connector {
    inner: transport::DefaultConnector,
    cancelled: Arc<AtomicBool>,
}

impl Connector {
    pub(super) fn new(cancelled: Arc<AtomicBool>) -> Self {
        Self {
            inner: transport::DefaultConnector::default(),
            cancelled,
        }
    }
}

impl transport::Connector for Connector {
    type Out = ReadAhead;

    fn connect(
        &self,
        details: &ConnectionDetails,
        chained: Option<()>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        Ok(self
            .inner
            .connect(details, chained)?
            .map(|inner| ReadAhead {
                inner,
                cancelled: Arc::clone(&self.cancelled),
            }))
    }
}

#[derive(Debug)]
pub(super) struct ReadAhead {
    inner: Box<dyn Transport>,
    cancelled: Arc<AtomicBool>,
}

impl Transport for ReadAhead {
    fn buffers(&mut self) -> &mut dyn Buffers {
        self.inner.buffers()
    }

    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), ureq::Error> {
        self.inner.transmit_output(amount, timeout)
    }

    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, ureq::Error> {
        let started = Instant::now();
        self.read(timeout, || started.elapsed())
    }

    fn is_open(&mut self) -> bool {
        !self.cancelled.load(Ordering::Relaxed) && self.inner.is_open()
    }

    fn is_tls(&self) -> bool {
        self.inner.is_tls()
    }
}

impl ReadAhead {
    fn read(
        &mut self,
        timeout: NextTimeout,
        mut elapsed: impl FnMut() -> Duration,
    ) -> Result<bool, ureq::Error> {
        let budget = timeout.for_read();
        loop {
            if self.cancelled.load(Ordering::Relaxed) {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "read-ahead cancelled",
                )
                .into());
            }
            let left = budget.after.saturating_sub(elapsed());
            if left.is_zero() {
                return Err(ureq::Error::Timeout(budget.reason));
            }
            let slice = NextTimeout {
                after: left.min(POLL).into(),
                reason: if left > POLL {
                    ureq::Timeout::PerRead
                } else {
                    budget.reason
                },
                ..timeout
            };
            match self.inner.await_input(slice) {
                // A poll expiring is not a failed HTTP request. Preserve partial
                // headers/body/TLS records and the original request deadline.
                Err(ureq::Error::Timeout(ureq::Timeout::PerRead)) if left > POLL => {}
                result => return result,
            }
        }
    }
}

#[cfg(test)]
mod tests;
