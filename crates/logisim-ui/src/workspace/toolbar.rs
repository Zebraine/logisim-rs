//! Workspace toolbar; document and simulation commands stay in their existing layers.
use crate::{
    Workspace,
    editor::Tool,
    i18n::tr,
    icons::{self, Icon},
};
use egui::{RichText, Ui};
use logisim_core::{Gate, Kind};

impl Workspace {
    pub(super) fn toolbar(&mut self, ui: &mut Ui) {
        if self.editor.session.context.appearance_mode {
            self.appearance.toolbar(ui, &mut self.editor);
            return;
        }
        ui.horizontal_wrapped(|ui| {
            for (icon, tool, hint) in [
                (
                    Icon::Poke,
                    Tool::Poke,
                    "Poke · P · toggle inputs / inspect signals",
                ),
                (Icon::Select, Tool::Select, "Select · V"),
                (
                    Icon::Wire,
                    Tool::Wire,
                    "Wire · W · drag or click two points",
                ),
                (Icon::Text, Tool::Place(Kind::Text), "Text · T"),
                (Icon::Pin, Tool::Place(Kind::Pin), "Pin"),
                (Icon::Tunnel, Tool::Place(Kind::Tunnel), "Tunnel · U"),
                (Icon::Not, Tool::Place(Kind::Gate(Gate::Not)), "NOT Gate"),
                (Icon::And, Tool::Place(Kind::Gate(Gate::And)), "AND Gate"),
                (Icon::Or, Tool::Place(Kind::Gate(Gate::Or)), "OR Gate"),
            ] {
                if icons::button(ui, icon, self.editor.tool == tool, true, hint).clicked() {
                    self.editor.set_tool(tool);
                }
            }
            ui.separator();
            if ui
                .add_enabled(self.editor.can_rotate(), egui::Button::new(tr("Rotate")))
                .on_hover_text(tr("Rotate clockwise · R"))
                .clicked()
            {
                self.editor.rotate();
            }
            ui.separator();
            self.editor.simulation.toolbar(ui);
            if ui.available_width() > 180.0 {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(self.editor.simulation.status())
                            .size(11.0)
                            .weak(),
                    );
                });
            }
        });
    }
}
