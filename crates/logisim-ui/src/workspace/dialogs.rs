//! Workspace modal routing and background-input gating.
use crate::{Workspace, i18n::tr};
use egui::Context;

impl Workspace {
    pub(super) fn dialog_open(&self) -> bool {
        self.editor.memory_target.is_some()
            || self.analysis.open()
            || self.generator.open
            || self.editor.simulation.tick_rate_draft.is_some()
            || self.editor.pla_target.is_some()
            || self.about_open
            || self.settings_open
            || self.project_options_open
            || self.name_dialog.is_some()
            || self.remove_dialog
            || self.editor.project_error.is_some()
    }
    pub(super) fn show_dialogs(&mut self, ctx: &Context) {
        if self.about_open {
            let about = crate::dialog::Dialog::prompt("about-window");
            self.about_open = about.show(ctx, tr("About logisim-rs"), |dialog| {
                dialog.body(|ui| {
                    ui.label(tr("A cleanroom digital circuit workspace."));
                    ui.separator();
                    ui.label(tr("Component editing, wiring and canvas navigation are available."));
                    ui.label(tr(
                        "Combinational and sequential simulation is available. Open and save the supported .circ components.",
                    ));
                });
                dialog.footer("Close", |_| {});
            }) == crate::dialog::Dismiss::Open;
        }
        self.project_dialogs(ctx);
        self.analysis_dialog(ctx);
        self.settings(ctx);
        self.editor
            .simulation
            .frequency_dialog(ctx, &mut self.preferences.tick_hz);
        self.memory_editor
            .show(ctx, &mut self.editor, &mut self.file_actions);
        self.pla_editor.show(ctx, &mut self.editor);
        self.generator.show(ctx, self.preferences.truth_table);
    }
}
