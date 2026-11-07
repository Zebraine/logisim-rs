//! Desktop host; editor presentation and navigation live in logisim-ui.
mod clocks;
mod file_picker;
#[cfg(test)]
mod file_tests;
mod files;
mod memory_import;
mod preferences;
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
            let mut workspace = logisim_ui::Workspace::default();
            let preferences = preferences::Preferences::load(&mut workspace);
            let mut app = DesktopApp {
                workspace,
                preferences,
                simulator: Default::default(),
                clocks: Default::default(),
                files: Default::default(),
            };
            if let Some(path) = std::env::args_os().nth(1) {
                logisim_ui::i18n::with_language(app.workspace.language(), || {
                    app.files.load(
                        std::path::Path::new(&path),
                        &mut app.workspace,
                        &mut app.simulator,
                    );
                });
            }
            Ok(Box::new(app))
        }),
    )
}

struct DesktopApp {
    preferences: preferences::Preferences,
    workspace: logisim_ui::Workspace,
    simulator: logisim_sim::ProjectSimulator,
    clocks: clocks::Clocks,
    files: files::Files,
}

impl eframe::App for DesktopApp {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        logisim_ui::i18n::with_language(self.workspace.language(), || self.show(ui));
    }
}
impl DesktopApp {
    fn show(&mut self, ui: &mut eframe::egui::Ui) {
        self.simulator
            .synchronize(self.workspace.project(), self.workspace.simulation_root());
        if let Some(delay) = self
            .clocks
            .poll(std::time::Instant::now(), &mut self.simulator)
        {
            ui.ctx().request_repaint_after(delay);
        }
        self.simulator.advance(64);
        let mut simulation_view = simulation::present_project(
            self.workspace.circuit(),
            &self.simulator,
            self.workspace.simulation_path(),
        );
        self.clocks.present(&mut simulation_view);
        self.workspace.set_simulation(simulation_view);
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
            .set_document(self.files.present(&self.workspace));
        ui.add_enabled_ui(!self.files.busy(), |ui| self.workspace.show(ui));
        for action in self.workspace.take_file_actions() {
            self.files
                .request(action, ui.ctx(), &mut self.workspace, &mut self.simulator);
        }
        self.files
            .show(ui.ctx(), &mut self.workspace, &mut self.simulator);
        self.preferences.synchronize(&mut self.workspace);
        let view = self.files.present(&self.workspace);
        ui.ctx()
            .send_viewport_cmd(eframe::egui::ViewportCommand::Title(format!(
                "Logisim rs — {} · {}{}{}",
                view.circuit_name,
                view.display_title(),
                if view.dirty { " *" } else { "" },
                if view.partial {
                    logisim_ui::i18n::tr(" (partial)")
                } else {
                    ""
                }
            )));
        let edited = self
            .simulator
            .synchronize(self.workspace.project(), self.workspace.simulation_root());
        let actions = self.workspace.take_simulation_actions();
        let acted = !actions.is_empty();
        for action in actions {
            self.clocks.action(action);
            simulation::apply_project(
                &mut self.simulator,
                self.workspace.simulation_path(),
                action,
            );
        }
        if edited || acted || (self.simulator.enabled() && self.simulator.pending_count() > 0) {
            ui.ctx().request_repaint();
        }
    }
}
