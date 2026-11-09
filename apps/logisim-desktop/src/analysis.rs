//! At most one isolated analysis worker and one replacement request.
//! Cancellation never joins a running job on a GUI frame; shutdown always joins.
use logisim_core::{
    Id, Project,
    generation::{Analysis, Error},
};
use logisim_ui::Workspace;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

struct Request {
    serial: u64,
    root: Id,
    project: Arc<Project>,
}
struct Job {
    request: Request,
    cancelled: Arc<AtomicBool>,
    thread: JoinHandle<Result<Analysis, Error>>,
}
#[derive(Default)]
pub(crate) struct Service {
    job: Option<Job>,
    waiting: Option<Request>,
}
impl Service {
    pub fn update(&mut self, workspace: &mut Workspace, repaint: impl FnOnce() + Send + 'static) {
        if let Some((serial, root)) = workspace.take_analysis_request() {
            self.waiting = Some(Request {
                serial,
                root,
                project: Arc::new(workspace.project().clone()),
            });
        }
        if let Some(job) = &self.job
            && (!workspace.analysis_pending(job.request.serial) || self.waiting.is_some())
        {
            job.cancelled.store(true, Ordering::Relaxed);
        }
        if self.job.as_ref().is_some_and(|j| j.thread.is_finished()) {
            let job = self.job.take().unwrap();
            let result = job
                .thread
                .join()
                .unwrap_or_else(|_| Err(error("Circuit analysis worker stopped.")));
            if !job.cancelled.load(Ordering::Relaxed) {
                let current = workspace.project() == job.request.project.as_ref();
                workspace.receive_analysis(
                    job.request.serial,
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
            if !workspace.analysis_pending(request.serial) {
                return;
            }
            let project = request.project.clone();
            let root = request.root;
            let cancelled = Arc::new(AtomicBool::new(false));
            let flag = cancelled.clone();
            match thread::Builder::new()
                .name("logisim-analysis".into())
                .spawn(move || {
                    let result = logisim_sim::analysis::analyze_cancellable(&project, root, || {
                        flag.load(Ordering::Relaxed)
                    });
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
                Err(e) => workspace.receive_analysis(
                    request.serial,
                    Err(Error {
                        message: "Could not start circuit analysis:",
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
