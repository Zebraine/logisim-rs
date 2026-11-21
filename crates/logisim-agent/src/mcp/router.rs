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
    request: Request,
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
    frame: Vec<u8>,
    end: Option<End>,
}

#[derive(Clone, Copy)]
enum End {
    Closed,
    Failed,
}

fn deliver(
    session: &str,
    id: &Value,
    operation: State,
    call: impl FnOnce() -> crate::Result<Value>,
    notify: &impl Fn(Completion),
) -> bool {
    let response = catch_unwind(AssertUnwindSafe(|| {
        let result = call();
        let end = match operation {
            State::Opening if result.is_err() => Some(End::Failed),
            State::Closing => Some(End::Closed),
            _ => None,
        };
        (tool_frame(id, result), end)
    }));
    let (frame, end) =
        response.unwrap_or_else(|_| (tool_frame(id, Err(stopped())), Some(End::Failed)));
    notify(Completion {
        session: session.to_owned(),
        frame,
        end,
    });
    end.is_none()
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
                // The synchronous dispatcher holds exactly this worker's document.
                // It uses the same root checks, save, patch and sim paths as the CLI.
                let mut service = Service {
                    root,
                    ..Service::default()
                };
                if !deliver(
                    &session,
                    &open_id,
                    State::Opening,
                    || service.open_session(session.clone(), &path),
                    &notify,
                ) {
                    return;
                }
                for job in mailbox {
                    let operation = if matches!(job.request, Request::ProjectClose { .. }) {
                        State::Closing
                    } else {
                        State::Ready
                    };
                    if !deliver(
                        &session,
                        &job.id,
                        operation,
                        || service.call(job.request),
                        &notify,
                    ) {
                        break;
                    }
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
        request: Request,
        notify: impl Fn(Completion) + Send + 'static,
    ) -> Option<Vec<u8>> {
        if let Request::ProjectOpen { path } = request {
            return self
                .open(id, path, notify)
                .err()
                .map(|error| tool_frame(id, Err(error)));
        }
        let Some(key) = request.session() else {
            return Some(tool_frame(id, self.service.call(request)));
        };
        let Some(worker) = self
            .workers
            .get_mut(key)
            .filter(|w| w.state != State::Opening)
        else {
            return Some(tool_frame(id, Err(unknown_session())));
        };
        worker.pending.push_back(id.clone());
        if worker.state != State::Closing {
            if matches!(request, Request::ProjectClose { .. }) {
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
        let worker = self
            .workers
            .get_mut(&done.session)
            .expect("registered worker completion");
        worker.pending.pop_front();
        let mut frames = vec![done.frame];
        if let Some(end) = done.end {
            let mut worker = self
                .workers
                .remove(&done.session)
                .expect("completed worker");
            for id in worker.pending.drain(..) {
                let error = match end {
                    End::Closed => unknown_session(),
                    End::Failed => stopped(),
                };
                frames.push(tool_frame(&id, Err(error)));
            }
            worker.join();
        } else if worker.state == State::Opening {
            worker.state = State::Ready;
        }
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
