//! One worker and its latest replacement request. Cancellation is cooperative;
//! polling never joins a running worker, while shutdown always cancels and joins.
//! Request validity and document changes remain the caller's responsibility.
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

#[derive(Clone, Default)]
pub(crate) struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

struct Running<R, T> {
    request: R,
    cancellation: Cancellation,
    thread: JoinHandle<T>,
}

pub(crate) struct LatestJob<R, T> {
    running: Option<Running<R, T>>,
    waiting: Option<R>,
}

impl<R, T> Default for LatestJob<R, T> {
    fn default() -> Self {
        Self {
            running: None,
            waiting: None,
        }
    }
}

impl<R, T> LatestJob<R, T> {
    pub fn submit(&mut self, request: R) {
        self.waiting = Some(request);
        if let Some(running) = &self.running {
            running.cancellation.cancel();
        }
    }

    /// Cancel obsolete work, then deliver an uncancelled completion at most once.
    /// A replacement also suppresses results that finished before this poll.
    pub fn poll(&mut self, pending: impl FnOnce(&R) -> bool) -> Option<(R, thread::Result<T>)> {
        let running = self.running.as_ref()?;
        if !pending(&running.request) {
            running.cancellation.cancel();
        }
        if !running.thread.is_finished() {
            return None;
        }
        let running = self.running.take().unwrap();
        let result = running.thread.join();
        (!running.cancellation.is_cancelled()).then_some((running.request, result))
    }

    #[cfg(test)]
    pub fn is_idle(&self) -> bool {
        self.running.is_none() && self.waiting.is_none()
    }
}

impl<R: Clone + Send + 'static, T: Send + 'static> LatestJob<R, T> {
    /// Start the waiting request once its predecessor has been joined. Return
    /// the request and OS error only if spawning fails; invalid requests expire.
    pub fn start(
        &mut self,
        name: &str,
        pending: impl FnOnce(&R) -> bool,
        execute: impl FnOnce(R, Cancellation) -> T + Send + 'static,
        repaint: impl FnOnce() + Send + 'static,
    ) -> Option<(R, io::Error)> {
        if self.running.is_some() {
            return None;
        }
        let request = self.waiting.take()?;
        if !pending(&request) {
            return None;
        }
        let input = request.clone();
        let cancellation = Cancellation::default();
        let flag = cancellation.clone();
        match thread::Builder::new().name(name.into()).spawn(move || {
            let result = execute(input, flag);
            repaint();
            result
        }) {
            Ok(thread) => {
                self.running = Some(Running {
                    request,
                    cancellation,
                    thread,
                });
                None
            }
            Err(error) => Some((request, error)),
        }
    }
}

impl<R, T> Drop for LatestJob<R, T> {
    fn drop(&mut self) {
        if let Some(running) = self.running.take() {
            running.cancellation.cancel();
            let _ = running.thread.join();
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) mod support;
