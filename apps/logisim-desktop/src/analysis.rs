//! Circuit analysis requests and snapshot validation for an explicit target.
use crate::background::LatestJob;
use logisim_core::{
    Id, Project,
    generation::{Analysis, Error},
};
use logisim_ui::Workspace;
use std::sync::Arc;

#[derive(Clone)]
struct Request {
    serial: u64,
    root: Id,
    project: Arc<Project>,
}
#[derive(Default)]
pub(crate) struct Service {
    jobs: LatestJob<Request, Result<Analysis, Error>>,
}
impl Service {
    pub fn update(&mut self, workspace: &mut Workspace, repaint: impl FnOnce() + Send + 'static) {
        if let Some((serial, root)) = workspace.take_analysis_request() {
            self.jobs.submit(Request {
                serial,
                root,
                project: Arc::new(workspace.project().clone()),
            });
        }
        if let Some((request, result)) = self
            .jobs
            .poll(|request| workspace.analysis_pending(request.serial))
        {
            let result = result.unwrap_or_else(|_| Err(error("Circuit analysis worker stopped.")));
            let current = workspace.project() == request.project.as_ref();
            workspace.receive_analysis(
                request.serial,
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
            "logisim-analysis",
            |request| workspace.analysis_pending(request.serial),
            |request, cancellation| {
                logisim_sim::analysis::analyze_cancellable(&request.project, request.root, || {
                    cancellation.is_cancelled()
                })
            },
            repaint,
        ) {
            workspace.receive_analysis(
                request.serial,
                Err(Error {
                    message: "Could not start circuit analysis:",
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
