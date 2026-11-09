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
mod tests {
    use super::*;
    use logisim_core::generation::Analysis;
    use std::{
        sync::Barrier,
        time::{Duration, Instant},
    };
    fn workspace() -> Workspace {
        let mut w = Workspace::default();
        let mut a = Analysis::default();
        a.add_output("Y".into()).unwrap();
        w.replace_circuit(a.generate().unwrap());
        w.request_analysis();
        let (serial, _) = w.take_analysis_request().unwrap();
        w.receive_analysis(serial, Ok(a));
        w
    }
    fn wait(service: &mut Service, w: &mut Workspace) {
        let start = Instant::now();
        while service.job.is_some() || service.waiting.is_some() {
            assert!(start.elapsed() < Duration::from_secs(10));
            service.update(w, || {});
            thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn worker_delivers_readonly_and_replaced_document_cancels_without_joining() {
        let mut w = workspace();
        let before = w.project().clone();
        let mut service = Service::default();
        service.update(&mut w, || {});
        assert!(service.job.is_some());
        wait(&mut service, &mut w);
        assert_eq!(w.project(), &before);
        let mut w = workspace();
        let task = w.take_generation_request().unwrap();
        let request = Request {
            task: task.clone(),
            root: w.active_circuit(),
            project: Arc::new(w.project().clone()),
        };
        let barrier = Arc::new(Barrier::new(2));
        let worker_barrier = barrier.clone();
        service.job = Some(Job {
            request,
            cancelled: Arc::new(AtomicBool::new(false)),
            thread: thread::spawn(move || {
                worker_barrier.wait();
                task.execute(|| false)
            }),
        });
        w.replace_project(Project::default());
        service.update(&mut w, || {});
        assert!(
            service
                .job
                .as_ref()
                .unwrap()
                .cancelled
                .load(Ordering::Relaxed)
        );
        barrier.wait();
        wait(&mut service, &mut w);
        assert_eq!(w.project(), &Project::default());
        let mut w = workspace();
        service.update(&mut w, || {});
        drop(service); // Cancels and joins an outstanding task.
    }
}
