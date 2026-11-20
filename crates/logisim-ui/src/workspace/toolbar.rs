//! Workspace toolbar; document and simulation commands stay in their existing layers.
use crate::{
    Workspace,
    editor::Tool,
    i18n::tr,
    icons::{self, Icon},
};
use egui::{Align, Layout, Ui};
use logisim_core::{Gate, Kind};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ToolbarLayout {
    Wide,
    Wrapped,
    Narrow,
}

impl ToolbarLayout {
    pub(super) fn for_width(width: f32) -> Self {
        if width < 620. {
            Self::Narrow
        } else if width < 1100. {
            Self::Wrapped
        } else {
            Self::Wide
        }
    }
    pub(super) fn height(self, m: crate::theme::Metrics, telemetry: bool) -> f32 {
        match self {
            Self::Wide => m.toolbar_wide,
            Self::Narrow if telemetry => m.toolbar_wrapped + m.row,
            _ => m.toolbar_wrapped,
        }
    }
    pub(super) fn padding(self, m: crate::theme::Metrics) -> i8 {
        let rows = if self == Self::Wide { 1. } else { 2. };
        let gap = (rows - 1.) * f32::from(m.gap);
        ((self.height(m, false) - rows * m.row - gap) / 2.).floor() as i8
    }
}

impl Workspace {
    pub(super) fn toolbar(&mut self, ui: &mut Ui, layout: ToolbarLayout) {
        if self.editor.session.context.appearance_mode {
            self.appearance.toolbar(ui, &mut self.editor);
            return;
        }
        let narrow = layout == ToolbarLayout::Narrow;
        let compact = layout != ToolbarLayout::Wide;
        ui.horizontal(|ui| {
            self.edit_tools(ui);
            if narrow {
                return;
            }
            if compact {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    self.editor.simulation.status_chip(ui)
                });
            } else {
                ui.separator();
                self.editor
                    .simulation
                    .toolbar(ui, &self.preferences.shortcuts);
                self.runtime_status(ui, true);
            }
        });
        if compact {
            ui.horizontal(|ui| {
                self.editor
                    .simulation
                    .toolbar(ui, &self.preferences.shortcuts);
                if !narrow {
                    self.runtime_status(ui, false);
                } else if !self.preferences.show_telemetry {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        self.editor.simulation.status_chip(ui)
                    });
                }
            });
        }
        if narrow && self.preferences.show_telemetry {
            ui.horizontal(|ui| self.runtime_status(ui, true));
        }
    }

    fn runtime_status(&self, ui: &mut Ui, show_state: bool) {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if self.preferences.show_telemetry {
                self.editor.simulation.telemetry(ui);
            }
            if show_state {
                self.editor.simulation.status_chip(ui);
            }
        });
    }

    fn edit_tools(&mut self, ui: &mut Ui) {
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
            if matches!(icon, Icon::Pin | Icon::Not) {
                ui.separator();
            }
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
    }
}
