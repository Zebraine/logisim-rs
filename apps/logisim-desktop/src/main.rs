//! Desktop host; editor presentation and navigation live in logisim-ui.
mod simulation;

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
            Ok(Box::new(DesktopApp::default()))
        }),
    )
}

#[derive(Default)]
struct DesktopApp {
    workspace: logisim_ui::Workspace,
    simulator: logisim_sim::Simulator,
}

impl eframe::App for DesktopApp {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        self.simulator.synchronize(self.workspace.circuit());
        self.simulator.advance(64);
        self.workspace.set_simulation(simulation::present(
            self.workspace.circuit(),
            &self.simulator,
        ));
        self.workspace.show(ui);
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
