//! One FIFO mailbox and one owner for each project's runtime. Only IDs cross back
//! to the router alongside finished frames; simulation state stays on its thread.
use super::tool_frame;
use crate::{Error, Request, Service, new_session_id, unknown_session};
use serde_json::Value;
use std::{
    collections::{HashMap, VecDeque},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::mpsc::{self, Sender},
    thread::{self, JoinHandle},
};

fn stopped() -> Error {
    Error::new("worker_stopped", "The project session worker stopped.")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Opening,
    Ready,
    Closing,
}

struct Job {
    id: Value,
    request: crate::Result<Request>,
}

struct Worker {
    jobs: Sender<Job>,
    thread: JoinHandle<()>,
    state: State,
    // Includes calls received after a queued close, so their errors follow its reply.
    pending: VecDeque<Value>,
}

impl Worker {
    fn join(self) {
        drop(self.jobs);
        let _ = self.thread.join();
    }
}

pub(super) struct Completion {
    session: String,
    outcome: Outcome,
}

enum Outcome {
    Reply(Vec<u8>),
    Last(Vec<u8>),
    Stopped,
}

pub(super) struct Router {
    service: Service,
    workers: HashMap<String, Worker>,
}

impl Router {
    pub fn new(service: Service) -> Self {
        Self {
            service,
            workers: HashMap::new(),
        }
    }

    fn open(
        &mut self,
        id: &Value,
        path: std::path::PathBuf,
        mut execute: impl FnMut(&mut Service, Request) -> crate::Result<Value> + Send + 'static,
        notify: impl Fn(Completion) + Send + 'static,
    ) -> crate::Result<()> {
        let key = new_session_id(&self.workers)?;
        let session = key.clone();
        let open_id = id.clone();
        let root = self.service.root.clone();
        let (jobs, mailbox) = mpsc::channel::<Job>();
        let thread = thread::Builder::new()
            .name(format!("session-{}", &key[..8]))
            .spawn(move || {
                let report = |outcome| {
                    notify(Completion {
                        session: session.clone(),
                        outcome,
                    })
                };
                let run = || {
                    // The worker exclusively owns this one document. Use the CLI's
                    // dispatcher for root checks, saves, edits and simulation.
                    let mut service = Service {
                        root,
                        ..Service::default()
                    };
                    let opened = service.open_session(session.clone(), &path);
                    let ready = opened.is_ok();
                    let frame = tool_frame(&open_id, opened);
                    if !ready {
                        return Some(frame);
                    }
                    report(Outcome::Reply(frame));
                    for job in mailbox {
                        let closing = matches!(job.request, Ok(Request::ProjectClose { .. }));
                        let result = job
                            .request
                            .and_then(|request| execute(&mut service, request));
                        let frame = tool_frame(&job.id, result);
                        if closing {
                            return Some(frame);
                        }
                        report(Outcome::Reply(frame));
                    }
                    None
                };
                match catch_unwind(AssertUnwindSafe(run)) {
                    Ok(Some(last)) => report(Outcome::Last(last)),
                    Err(_) => report(Outcome::Stopped),
                    Ok(None) => {} // Router shutdown disconnected the mailbox.
                }
            })
            .map_err(|e| Error::new("worker_stopped", e))?;
        self.workers.insert(
            key,
            Worker {
                jobs,
                thread,
                state: State::Opening,
                pending: VecDeque::from([id.clone()]),
            },
        );
        Ok(())
    }

    /// An immediate frame or one outstanding reply delivered via notify.
    pub fn submit(
        &mut self,
        id: &Value,
        request: crate::Result<Request>,
        session_hint: Option<&str>,
        notify: impl Fn(Completion) + Send + 'static,
    ) -> Option<Vec<u8>> {
        if let Ok(Request::ProjectOpen { path }) = request {
            return self
                .open(id, path, Service::call, notify)
                .err()
                .map(|error| tool_frame(id, Err(error)));
        }
        // Even a rejected tool argument follows earlier commands for that session.
        let key = match &request {
            Ok(request) => request.session(),
            Err(_) => session_hint,
        };
        let Some(worker) = key
            .and_then(|key| self.workers.get_mut(key))
            .filter(|w| w.state != State::Opening)
        else {
            // The connection dispatcher has no sessions. It retains the CLI's
            // validation order for unknown sessions and handles global tools.
            return Some(tool_frame(id, request.and_then(|r| self.service.call(r))));
        };
        worker.pending.push_back(id.clone());
        if worker.state != State::Closing {
            if matches!(request, Ok(Request::ProjectClose { .. })) {
                worker.state = State::Closing;
            }
            // A disconnected receiver has already posted a terminal completion.
            // finish() accounts for every pending ID, including this one.
            let _ = worker.jobs.send(Job {
                id: id.clone(),
                request,
            });
        }
        None
    }

    pub fn finish(&mut self, done: Completion) -> Vec<Vec<u8>> {
        if let Outcome::Reply(frame) = done.outcome {
            let worker = self
                .workers
                .get_mut(&done.session)
                .expect("registered worker");
            worker.pending.pop_front();
            if worker.state == State::Opening {
                worker.state = State::Ready;
            }
            return vec![frame];
        }
        let mut worker = self.workers.remove(&done.session).expect("terminal worker");
        let mut frames = Vec::with_capacity(worker.pending.len());
        let error = if let Outcome::Last(frame) = done.outcome {
            worker.pending.pop_front();
            frames.push(frame);
            unknown_session
        } else {
            stopped
        };
        frames.extend(
            worker
                .pending
                .drain(..)
                .map(|id| tool_frame(&id, Err(error()))),
        );
        worker.join();
        frames
    }
}
impl Drop for Router {
    fn drop(&mut self) {
        for (_, worker) in self.workers.drain() {
            worker.join();
        }
    }
}

#[cfg(test)]
mod tests;
