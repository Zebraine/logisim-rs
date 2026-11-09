//! Analysis presentation and host requests; evaluation belongs to logisim-sim.
use crate::{Workspace, i18n::tr};
use logisim_core::generation::{Analysis, Error};

#[derive(Default)]
pub(crate) struct AnalysisUi {
    serial: u64,
    requested: bool,
    open: bool,
    error: Option<Error>,
}
impl AnalysisUi {
    pub fn open(&self) -> bool {
        self.open
    }
    pub fn cancel(&mut self) {
        self.serial = self.serial.wrapping_add(1);
        self.open = false;
        self.requested = false;
        self.error = None;
    }
}
impl Workspace {
    pub fn request_analysis(&mut self) {
        self.finish_edit();
        self.analysis.cancel();
        self.analysis.open = true;
        if self.document.partial {
            self.analysis.error = Some(Error {
                message: "Circuit analysis is unavailable for an import with omitted content.",
                detail: String::new(),
            });
        } else {
            self.analysis.requested = true;
        }
    }
    pub fn take_analysis_request(&mut self) -> Option<u64> {
        std::mem::take(&mut self.analysis.requested).then_some(self.analysis.serial)
    }
    pub fn analysis_pending(&self, serial: u64) -> bool {
        self.analysis.open && self.analysis.error.is_none() && self.analysis.serial == serial
    }
    /// Only a still-open request may replace the generator draft. Failure preserves it.
    pub fn receive_analysis(&mut self, serial: u64, result: Result<Analysis, Error>) {
        if !self.analysis_pending(serial) {
            return;
        }
        let result = result.and_then(|analysis| {
            let target = logisim_core::generation::RebuildTarget::bind(
                self.project().circuit(self.active_circuit()).unwrap(),
                &analysis,
            )?;
            Ok((analysis, target))
        });
        match result {
            Ok((analysis, target)) => {
                self.analysis.cancel();
                self.generator.load_analysis(analysis);
                self.generator.target = Some(target);
                self.generator.replace_target = true;
                self.open_generator();
            }
            Err(error) => self.analysis.error = Some(error),
        }
    }
    pub(crate) fn analysis_dialog(&mut self, ctx: &egui::Context) {
        if !self.analysis.open {
            return;
        }
        let mut close = false;
        let response = egui::Modal::new(egui::Id::unique("circuit-analysis")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 80.0).clamp(280.0, 560.0));
            ui.heading(tr("Analyze Circuit"));
            if let Some(error) = &self.analysis.error {
                ui.colored_label(egui::Color32::from_rgb(185, 28, 28), tr(error.message));
                egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| { ui.label(&error.detail); });
                close = ui.button(tr("Close")).clicked();
            } else {
                ui.horizontal(|ui| { ui.spinner(); ui.label(tr("Analyzing circuit…")); });
                ui.weak(tr("Only stateless combinational circuits are supported. The current simulation is unchanged."));
                close = ui.button(tr("Cancel")).clicked();
            }
        });
        if close || response.should_close() {
            self.analysis.cancel();
        }
    }
}
