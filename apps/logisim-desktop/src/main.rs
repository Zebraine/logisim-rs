//! Desktop host; editor presentation and navigation live in logisim-ui.
mod clocks;
mod file_picker;
#[cfg(test)]
mod file_tests;
mod files;
mod library_import;
mod memory_import;
mod plugins;
mod preferences;
mod runtime_rates;
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
                simulator: simulation::worker::Worker::new({
                    let ctx = cc.egui_ctx.clone();
                    move || ctx.request_repaint()
                }),
                rates: Default::default(),
                files: Default::default(),
                window_title: String::new(),
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
    simulator: simulation::worker::Worker,
    rates: runtime_rates::RuntimeRates,
    files: files::Files,
    window_title: String,
}

impl eframe::App for DesktopApp {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        logisim_ui::i18n::with_language(self.workspace.language(), || self.show(ui));
    }
}
impl DesktopApp {
    fn show(&mut self, ui: &mut eframe::egui::Ui) {
        use simulation::Host;
        self.rates
            .record_ticks(self.simulator.receive(&mut self.workspace));
        self.rates.present(
            std::time::Instant::now(),
            ui.ctx().cumulative_frame_nr(),
            self.workspace.simulation_mut(),
        );
        ui.ctx()
            .request_repaint_after(runtime_rates::REFRESH_INTERVAL);
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
        let title = format!(
            "Logisim rs — {} · {}{}{}",
            view.circuit_name,
            view.display_title(),
            if view.dirty { " *" } else { "" },
            if view.partial {
                logisim_ui::i18n::tr(" (partial)")
            } else {
                ""
            }
        );
        if self.window_title != title {
            ui.ctx()
                .send_viewport_cmd(eframe::egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
        self.simulator.set_plugins(self.files.plugins.clone());
        self.simulator
            .synchronize(&mut self.workspace, self.files.busy());
        for action in self.workspace.take_simulation_actions() {
            self.simulator.action(action);
        }
    }
}
