//! Workspace toolbar; document and simulation commands stay in their existing layers.
use crate::shortcuts::ShortcutCaptions as _;
use crate::{
    Workspace,
    editor::Tool,
    i18n::tr,
    icons::{self, Icon},
    shortcuts::Command,
};
use egui::{Align, Layout, Ui};
use logisim_core::{Gate, Kind};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ToolbarLayout {
    Wide,
    Compact,
    Narrow,
}

impl ToolbarLayout {
    pub(super) fn for_width(width: f32) -> Self {
        if width < 660. {
            Self::Narrow
        } else if width < 1080. {
            Self::Compact
        } else {
            Self::Wide
        }
    }
    fn wrapped(self, appearance: bool) -> bool {
        self == Self::Narrow || (appearance && self == Self::Compact)
    }
    pub(super) fn height(self, m: crate::theme::Metrics, appearance: bool) -> f32 {
        if self.wrapped(appearance) {
            m.toolbar_wrapped
        } else {
            m.toolbar_wide
        }
    }
    pub(super) fn padding(self, m: crate::theme::Metrics, appearance: bool) -> i8 {
        let rows = if self.wrapped(appearance) { 2. } else { 1. };
        ((self.height(m, appearance) - rows * m.row - (rows - 1.) * f32::from(m.gap)) / 2.).floor()
            as i8
    }
}

impl Workspace {
    pub(super) fn toolbar(&mut self, ui: &mut Ui, layout: ToolbarLayout) {
        if self.editor.session.context.appearance_mode {
            self.appearance.toolbar(ui, &mut self.editor);
            return;
        }
        ui.horizontal(|ui| {
            self.edit_tools(ui, layout != ToolbarLayout::Wide);
            if layout == ToolbarLayout::Narrow {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    self.editor.simulation.status_chip(ui)
                });
            } else {
                ui.separator();
                self.simulation_tools(ui, layout);
            }
        });
        if layout == ToolbarLayout::Narrow {
            ui.horizontal(|ui| self.simulation_tools(ui, layout));
        }
    }

    fn simulation_tools(&mut self, ui: &mut Ui, layout: ToolbarLayout) {
        let sim = &mut self.editor.simulation;
        ui.add_enabled_ui(!sim.view.awaiting_snapshot, |ui| {
            sim.toolbar(
                ui,
                &self.preferences.shortcuts,
                layout != ToolbarLayout::Wide,
            )
        });
        ui.separator();
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if self.preferences.show_telemetry && sim.clock_running() {
                sim.telemetry(ui, layout == ToolbarLayout::Wide);
            }
            if layout != ToolbarLayout::Narrow {
                sim.status_chip(ui);
            }
        });
    }

    fn edit_tools(&mut self, ui: &mut Ui, compact: bool) {
        let core = [
            (Icon::Poke, Tool::Poke),
            (Icon::Select, Tool::Select),
            (Icon::Wire, Tool::Wire),
            (Icon::Pin, Tool::Place(Kind::Pin)),
        ];
        let more = [
            (Icon::Text, Tool::Place(Kind::Text)),
            (Icon::Tunnel, Tool::Place(Kind::Tunnel)),
            (Icon::Not, Tool::Place(Kind::Gate(Gate::Not))),
            (Icon::And, Tool::Place(Kind::Gate(Gate::And))),
            (Icon::Or, Tool::Place(Kind::Gate(Gate::Or))),
        ];
        for (icon, tool) in core {
            self.edit_tool(ui, icon, tool, false);
        }
        if compact {
            let selected = more.iter().any(|(_, tool)| self.editor.tool == *tool);
            let response =
                icons::named_button(ui, Icon::More, selected, true, tr("More editing tools"));
            egui::Popup::menu(&response).show(|ui| {
                for (icon, tool) in more {
                    self.edit_tool(ui, icon, tool, true);
                }
            });
        } else {
            for (icon, tool) in more {
                self.edit_tool(ui, icon, tool, false);
            }
        }
        let hint =
            self.preferences
                .shortcuts
                .caption_with(Command::ToolRotate, "Rotate clockwise", ui);
        if icons::named_button(ui, Icon::Rotate, false, self.editor.can_rotate(), &hint).clicked() {
            self.editor.rotate();
        }
    }

    fn edit_tool(&mut self, ui: &mut Ui, icon: Icon, tool: Tool, in_menu: bool) {
        let bindings = &self.preferences.shortcuts;
        let name = match tool {
            Tool::Select => bindings.caption(Command::ToolSelect, ui),
            Tool::Poke => bindings.caption_with(
                Command::ToolPoke,
                "Poke · toggle inputs / inspect signals",
                ui,
            ),
            Tool::Wire => {
                bindings.caption_with(Command::ToolWire, "Wire · drag or click two points", ui)
            }
            Tool::Place(Kind::Text) => bindings.caption(Command::ToolText, ui),
            Tool::Place(Kind::Tunnel) => bindings.caption(Command::ToolTunnel, ui),
            Tool::Place(kind) => tr(kind.name()).into(),
        };
        let selected = self.editor.tool == tool;
        let response = if in_menu {
            ui.selectable_label(selected, &name)
        } else {
            icons::named_button(ui, icon, selected, true, &name)
        };
        if response.clicked() {
            self.editor.set_tool(tool);
            if in_menu {
                ui.close();
            }
        }
    }
}
