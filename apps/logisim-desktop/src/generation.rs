//! Expression and circuit generation requests bound to a document snapshot.
use crate::background::LatestJob;
use logisim_core::{Id, Project, generation::Error};
use logisim_ui::{
    Workspace,
    generation::{Reply, Request as Task},
};
use std::sync::Arc;

#[derive(Clone)]
struct Request {
    task: Task,
    root: Id,
    project: Arc<Project>,
}
#[derive(Default)]
pub(crate) struct Service {
    jobs: LatestJob<Request, Result<Reply, Error>>,
}
impl Service {
    pub fn update(&mut self, workspace: &mut Workspace, repaint: impl FnOnce() + Send + 'static) {
        if let Some(task) = workspace.take_generation_request() {
            self.jobs.submit(Request {
                task,
                root: workspace.active_circuit(),
                project: Arc::new(workspace.project().clone()),
            });
        }
        if let Some((request, result)) = self
            .jobs
            .poll(|request| workspace.generation_pending(request.task.serial))
        {
            let result = result.unwrap_or_else(|_| Err(error("Computation worker stopped.")));
            let current = workspace.active_circuit() == request.root
                && workspace.project() == request.project.as_ref();
            workspace.receive_generation(
                request.task.serial,
                if current {
                    result
                } else {
                    Err(error(
                        "The circuit changed during analysis. Please analyze it again.",
                    ))
                },
            );
        }
        if let Some((request, error)) = self.jobs.start(
            "logisim-generation",
            |request| workspace.generation_pending(request.task.serial),
            |request, cancellation| request.task.execute(|| cancellation.is_cancelled()),
            repaint,
        ) {
            workspace.receive_generation(
                request.task.serial,
                Err(Error {
                    message: "Could not start computation:",
                    detail: error.to_string(),
                }),
            );
        }
    }
}
fn error(message: &'static str) -> Error {
    Error {
        message,
        detail: String::new(),
    }
}
#[cfg(test)]
mod tests;
