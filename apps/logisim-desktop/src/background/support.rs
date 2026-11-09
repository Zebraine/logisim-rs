use super::{Cancellation, LatestJob};
use std::{
    sync::mpsc::{self, Sender},
    thread,
    time::{Duration, Instant},
};

pub(crate) const TIMEOUT: Duration = Duration::from_secs(10);

/// Releases the worker even when an assertion fails, so service Drop can join.
pub(crate) struct Blocked {
    release: Sender<()>,
    pub cancellation: Cancellation,
}

impl Drop for Blocked {
    fn drop(&mut self) {
        let _ = self.release.send(());
    }
}

pub(crate) fn block<R: Clone + Send + 'static, T: Send + 'static>(
    jobs: &mut LatestJob<R, T>,
    request: R,
    execute: impl FnOnce(R) -> T + Send + 'static,
) -> Blocked {
    let (release, wait) = mpsc::channel();
    let (started, flag) = mpsc::channel();
    jobs.submit(request);
    assert!(
        jobs.start(
            "test-background-blocked",
            |_| true,
            move |request, cancellation| {
                started.send(cancellation).unwrap();
                wait.recv_timeout(TIMEOUT).expect("worker was not released");
                execute(request)
            },
            || {},
        )
        .is_none()
    );
    Blocked {
        release,
        cancellation: flag.recv_timeout(TIMEOUT).unwrap(),
    }
}

pub(crate) fn wait_until(mut ready: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ready() {
        assert!(start.elapsed() < TIMEOUT, "background worker timed out");
        thread::sleep(Duration::from_millis(1));
    }
}
