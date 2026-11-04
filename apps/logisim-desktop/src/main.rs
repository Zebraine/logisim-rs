//! Desktop host; editor presentation and navigation live in logisim-ui.

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
}

impl eframe::App for DesktopApp {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        self.workspace.show(ui);
    }
}
