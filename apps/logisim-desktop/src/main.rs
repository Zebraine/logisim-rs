//! Desktop host; editor presentation and navigation live in logisim-ui.
mod file_picker;
#[cfg(test)]
mod file_tests;
mod files;
mod simulation;
mod storage;

fn main() -> eframe::Result {
    eframe::run_native(
        "Logisim rs — main · Untitled",
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_inner_size([1200.0, 820.0])
                .with_min_inner_size([720.0, 480.0])
                .with_app_id("logisim-rs"),
            ..Default::default()
        },
        Box::new(|cc| {
            logisim_ui::configure(&cc.egui_ctx);
            let mut app = DesktopApp::default();
            if let Some(path) = std::env::args_os().nth(1) {
                app.files.load(
                    std::path::Path::new(&path),
                    &mut app.workspace,
                    &mut app.simulator,
                );
            }
            Ok(Box::new(app))
        }),
    )
}

#[derive(Default)]
struct DesktopApp {
    workspace: logisim_ui::Workspace,
    simulator: logisim_sim::Simulator,
    files: files::Files,
}

impl eframe::App for DesktopApp {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        self.simulator.synchronize(self.workspace.circuit());
        self.simulator.advance(64);
        self.workspace.set_simulation(simulation::present(
            self.workspace.circuit(),
            &self.simulator,
        ));
        if ui.input(|i| i.viewport().close_requested()) && !self.files.allow_close {
            ui.ctx()
                .send_viewport_cmd(eframe::egui::ViewportCommand::CancelClose);
            self.files.request(
                logisim_ui::FileAction::Close,
                ui.ctx(),
                &mut self.workspace,
                &mut self.simulator,
            );
        }
        self.workspace
            .set_document(self.files.present(self.workspace.circuit()));
        ui.add_enabled_ui(!self.files.busy(), |ui| self.workspace.show(ui));
        for action in self.workspace.take_file_actions() {
            self.files
                .request(action, ui.ctx(), &mut self.workspace, &mut self.simulator);
        }
        self.files
            .show(ui.ctx(), &mut self.workspace, &mut self.simulator);
        let view = self.files.present(self.workspace.circuit());
        ui.ctx()
            .send_viewport_cmd(eframe::egui::ViewportCommand::Title(format!(
                "Logisim rs — {} · {}{}{}",
                view.circuit_name,
                view.title,
                if view.dirty { " *" } else { "" },
                if view.partial { " (partial)" } else { "" }
            )));
        let edited = self.simulator.synchronize(self.workspace.circuit());
        let actions = self.workspace.take_simulation_actions();
        let acted = !actions.is_empty();
        for action in actions {
            simulation::apply(&mut self.simulator, action);
        }
        if edited || acted || (self.simulator.enabled() && self.simulator.pending_count() > 0) {
            ui.ctx().request_repaint();
        }
    }
}
