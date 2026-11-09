use crate::generation::*;

pub(super) fn finish_jobs(w: &mut Workspace) {
    while let Some(request) = w.take_generation_request() {
        w.receive_generation(request.serial, request.execute(|| false));
    }
}
pub(super) fn click(ctx: &egui::Context, w: &mut Workspace, label: &str) {
    finish_jobs(w);
    crate::tests::support::widgets::click(ctx, w, label);
    finish_jobs(w);
}
