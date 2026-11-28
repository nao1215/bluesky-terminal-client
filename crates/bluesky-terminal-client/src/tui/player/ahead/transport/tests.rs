use super::*;
use std::collections::VecDeque;
use std::sync::{Mutex, mpsc};
use std::thread;
use ureq::unversioned::transport::LazyBuffers;

#[derive(Debug)]
struct Pending {
    entered: mpsc::Sender<NextTimeout>,
    // No shutdown handle: cancellation cannot wake this read. Only its
    // timeout or a response from the server can do so.
    response: Mutex<mpsc::Receiver<()>>,
    buffers: LazyBuffers,
}

impl Transport for Pending {
    fn buffers(&mut self) -> &mut dyn Buffers {
        &mut self.buffers
    }

    fn transmit_output(&mut self, _: usize, _: NextTimeout) -> Result<(), ureq::Error> {
        unreachable!()
    }

    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, ureq::Error> {
        self.entered.send(timeout).unwrap();
        match self.response.lock().unwrap().recv_timeout(*timeout.after) {
            Ok(()) => Ok(true),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(ureq::Error::Timeout(timeout.reason)),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                panic!("server closed before cancellation")
            }
        }
    }

    fn is_open(&mut self) -> bool {
        true
    }
}

#[test]
fn cancellation_releases_a_receive_that_shutdown_does_not_wake() {
    let cancelled = Arc::new(AtomicBool::new(false));
    let (entered, receiving) = mpsc::channel();
    let (_server, response) = mpsc::channel();
    let mut transport = ReadAhead {
        inner: Box::new(Pending {
            entered,
            response: Mutex::new(response),
            buffers: LazyBuffers::new(1024, 1024),
        }),
        cancelled: Arc::clone(&cancelled),
    };
    let (done, finished) = mpsc::channel();
    let worker = thread::spawn(move || {
        done.send(transport.await_input(NextTimeout {
            after: Duration::from_secs(30).into(),
            ..NextTimeout::default()
        }))
        .unwrap();
    });
    let timeout = receiving.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(*timeout.after, POLL);
    cancelled.store(true, Ordering::Relaxed);
    let result = finished.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(
        matches!(result, Err(ureq::Error::Io(e)) if e.kind() == io::ErrorKind::ConnectionAborted)
    );
    worker.join().unwrap();
}

#[derive(Debug)]
struct Script {
    outcomes: VecDeque<Result<bool, ureq::Error>>,
    timeouts: Arc<Mutex<Vec<NextTimeout>>>,
    buffers: LazyBuffers,
}

impl Transport for Script {
    fn buffers(&mut self) -> &mut dyn Buffers {
        &mut self.buffers
    }

    fn transmit_output(&mut self, _: usize, _: NextTimeout) -> Result<(), ureq::Error> {
        unreachable!()
    }

    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, ureq::Error> {
        self.timeouts.lock().unwrap().push(timeout);
        self.outcomes.pop_front().expect("unexpected read")
    }

    fn is_open(&mut self) -> bool {
        true
    }

    fn is_tls(&self) -> bool {
        true
    }
}

fn poll_timeout() -> Result<bool, ureq::Error> {
    Err(ureq::Error::Timeout(ureq::Timeout::PerRead))
}

#[test]
fn polls_keep_partial_input_and_the_original_deadline() {
    let timeouts = Arc::new(Mutex::new(Vec::new()));
    let mut transport = ReadAhead {
        inner: Box::new(Script {
            outcomes: [poll_timeout(), poll_timeout(), Ok(true)].into(),
            timeouts: Arc::clone(&timeouts),
            buffers: LazyBuffers::new(1024, 1024),
        }),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let partial = b"HTTP/1.1 200";
    transport.buffers().input_append_buf()[..partial.len()].copy_from_slice(partial);
    transport.buffers().input_appended(partial.len());
    let timeout = NextTimeout {
        after: Duration::from_millis(250).into(),
        ..NextTimeout::default()
    };
    let mut elapsed = [0, 100, 200].into_iter();
    assert!(
        transport
            .read(timeout, || Duration::from_millis(elapsed.next().unwrap()))
            .unwrap()
    );
    assert_eq!(transport.buffers().input(), partial);
    assert!(transport.is_tls());
    let calls = timeouts.lock().unwrap();
    assert_eq!(
        calls
            .iter()
            .map(|t| t.after.as_millis())
            .collect::<Vec<_>>(),
        [100, 100, 50]
    );
    assert_eq!(calls[2].reason, ureq::Timeout::Global);
    drop(calls);
    assert!(matches!(
        transport.read(timeout, || Duration::from_millis(250)),
        Err(ureq::Error::Timeout(ureq::Timeout::Global))
    ));
    assert_eq!(
        timeouts.lock().unwrap().len(),
        3,
        "no read after the deadline"
    );
}

#[test]
fn real_timeouts_and_other_io_errors_are_not_retried() {
    for error in [
        ureq::Error::Timeout(ureq::Timeout::Global),
        ureq::Error::Timeout(ureq::Timeout::PerRead),
        ureq::Error::Io(io::ErrorKind::ConnectionReset.into()),
    ] {
        let timeouts = Arc::new(Mutex::new(Vec::new()));
        let mut transport = ReadAhead {
            inner: Box::new(Script {
                outcomes: [Err(error)].into(),
                timeouts: Arc::clone(&timeouts),
                buffers: LazyBuffers::new(1024, 1024),
            }),
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        let timeout = NextTimeout {
            after: Duration::from_secs(5).into(),
            per_read: Some(Duration::from_millis(50).into()),
            ..NextTimeout::default()
        };
        assert!(transport.read(timeout, || Duration::ZERO).is_err());
        let calls = timeouts.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].after.as_millis(), 50);
    }
}
