//! Desktop host; editor presentation and navigation live in logisim-ui.
mod analysis;
mod background;
mod clocks;
mod files;
mod generation;
mod library_import;
mod memory_import;
mod plugins;
mod preferences;
mod recovery;
mod runtime_rates;
mod simulation;
mod storage;
#[cfg(test)]
mod tests;
mod window;

fn main() -> eframe::Result {
    let mut workspace = logisim_ui::Workspace::default();
    let preferences = preferences::Preferences::load(&mut workspace);
    let window = preferences.initial_window(&workspace);
    eframe::run_native(
        "Logisim rs — main · Untitled",
        window.options(),
        Box::new(move |cc| {
            logisim_ui::configure(&cc.egui_ctx);
            workspace.preferences().apply_appearance(&cc.egui_ctx);
            let files = files::Files::with_recovery(preferences.recovery_directory());
            let mut app = DesktopApp {
                analysis: Default::default(),
                generation: Default::default(),
                last_panels: workspace.layout(),
                workspace,
                preferences,
                window,
                initial_geometry_pending: true,
                simulator: simulation::worker::Worker::new({
                    let ctx = cc.egui_ctx.clone();
                    move || ctx.request_repaint()
                }),
                rates: Default::default(),
                files,
                window_title: String::new(),
            };
            logisim_ui::i18n::with_language(app.workspace.language(), || {
                app.files.startup(
                    std::env::args_os().nth(1).map(Into::into),
                    &mut app.workspace,
                    &mut app.simulator,
                );
            });
            Ok(Box::new(app))
        }),
    )
}

struct DesktopApp {
    analysis: analysis::Service,
    generation: generation::Service,
    preferences: preferences::Preferences,
    workspace: logisim_ui::Workspace,
    simulator: simulation::worker::Worker,
    rates: runtime_rates::RuntimeRates,
    files: files::Files,
    window_title: String,
    window: window::WindowState,
    initial_geometry_pending: bool,
    last_panels: logisim_ui::WorkspaceLayout,
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
        self.files.present_recovery(&mut self.workspace);
        self.workspace
            .set_document(self.files.present(&self.workspace));
        ui.add_enabled_ui(!self.files.busy(), |ui| {
            if self.files.capturing_image() {
                // Suppress interaction and tooltips while painting the original canvas colors.
                ui.set_opacity(1.);
            }
            self.workspace.show(ui);
        });
        for action in self.workspace.take_file_actions() {
            self.files
                .request(action, ui.ctx(), &mut self.workspace, &mut self.simulator);
        }
        self.files.show(
            ui.ctx(),
            &mut self.workspace,
            &mut self.simulator,
            &mut self.preferences,
        );
        self.sync_layout(ui);
        self.preferences.synchronize(&mut self.workspace);
        let context = ui.ctx().clone();
        self.analysis
            .update(&mut self.workspace, move || context.request_repaint());
        let context = ui.ctx().clone();
        self.generation
            .update(&mut self.workspace, move || context.request_repaint());
        let view = self.files.present(&self.workspace);
        if !self.files.allow_close {
            self.files.autosave.update(
                std::time::Instant::now(),
                self.workspace.preferences().files,
                view.dirty,
                &self.files.storage,
                self.workspace.project(),
            );
        }
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
            .synchronize(&mut self.workspace, self.files.suspends_simulation());
        for action in self.workspace.take_simulation_actions() {
            self.simulator.action(action);
        }
    }

    fn sync_layout(&mut self, ui: &eframe::egui::Ui) {
        let before_window = self.window;
        let reset = self.workspace.take_window_reset();
        if reset {
            self.window = window::WindowState::default();
        }
        if self.initial_geometry_pending && ui.ctx().cumulative_frame_nr() == 0 {
            // egui-winit sampled the initial monitor before applying the user's UI scale.
            ui.ctx().request_repaint();
        } else if self.initial_geometry_pending || reset {
            self.initial_geometry_pending = false;
            self.window = self.window.restore(ui.ctx());
        } else {
            let zoom = ui.ctx().zoom_factor();
            ui.input(|i| {
                self.window
                    .observe(i.viewport(), i.viewport_rect().size(), zoom)
            });
        }
        let panels = self.workspace.layout();
        let options = self.workspace.preferences().workspace;
        self.preferences.observe_layout(
            (reset || options.restore_window && before_window != self.window)
                .then_some(self.window),
            (reset || options.restore_panels && self.last_panels != panels).then_some(panels),
            std::time::Instant::now(),
        );
        self.last_panels = panels;
        if self.files.allow_close {
            self.preferences.flush_layout();
        }
    }
}
