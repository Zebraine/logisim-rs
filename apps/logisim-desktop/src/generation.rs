//! At most one isolated analysis worker and one replacement request.
//! Cancellation never joins a running job on a GUI frame; shutdown always joins.
use logisim_core::{Id, Project, generation::Error};
use logisim_ui::{
    Workspace,
    generation::{Reply, Request as Task},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

struct Request {
    task: Task,
    root: Id,
    project: Arc<Project>,
}
struct Job {
    request: Request,
    cancelled: Arc<AtomicBool>,
    thread: JoinHandle<Result<Reply, Error>>,
}
#[derive(Default)]
pub(crate) struct Service {
    job: Option<Job>,
    waiting: Option<Request>,
}
impl Service {
    pub fn update(&mut self, workspace: &mut Workspace, repaint: impl FnOnce() + Send + 'static) {
        if let Some(task) = workspace.take_generation_request() {
            self.waiting = Some(Request {
                task,
                root: workspace.active_circuit(),
                project: Arc::new(workspace.project().clone()),
            });
        }
        if let Some(job) = &self.job
            && (!workspace.generation_pending(job.request.task.serial) || self.waiting.is_some())
        {
            job.cancelled.store(true, Ordering::Relaxed);
        }
        if self.job.as_ref().is_some_and(|j| j.thread.is_finished()) {
            let job = self.job.take().unwrap();
            let result = job
                .thread
                .join()
                .unwrap_or_else(|_| Err(error("Computation worker stopped.")));
            if !job.cancelled.load(Ordering::Relaxed) {
                let current = workspace.active_circuit() == job.request.root
                    && workspace.project() == job.request.project.as_ref();
                workspace.receive_generation(
                    job.request.task.serial,
                    if current {
                        result
                    } else {
                        Err(error(
                            "The circuit changed during analysis. Please analyze it again.",
                        ))
                    },
                );
            }
        }
        if self.job.is_none()
            && let Some(request) = self.waiting.take()
        {
            if !workspace.generation_pending(request.task.serial) {
                return;
            }
            let task = request.task.clone();
            let cancelled = Arc::new(AtomicBool::new(false));
            let flag = cancelled.clone();
            match thread::Builder::new()
                .name("logisim-generation".into())
                .spawn(move || {
                    let result = task.execute(|| flag.load(Ordering::Relaxed));
                    repaint();
                    result
                }) {
                Ok(thread) => {
                    self.job = Some(Job {
                        request,
                        cancelled,
                        thread,
                    })
                }
                Err(e) => workspace.receive_generation(
                    request.task.serial,
                    Err(Error {
                        message: "Could not start computation:",
                        detail: e.to_string(),
                    }),
                ),
            }
        }
    }
}
fn error(message: &'static str) -> Error {
    Error {
        message,
        detail: String::new(),
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() {
            job.cancelled.store(true, Ordering::Relaxed);
            let _ = job.thread.join();
        }
    }
}

#[cfg(test)]
mod tests;
