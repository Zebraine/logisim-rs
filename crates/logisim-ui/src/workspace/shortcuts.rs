//! Route workspace commands once, after the canvas and text fields have resolved focus.
use crate::{
    FileAction, SimulationAction, Workspace,
    appearance::Command as Art,
    editor::{CLIPBOARD_MARKER, Editor, Tool},
    project::CircuitCommand,
    shortcuts::{Chord, Command, Group, Scope},
};
use egui::{Event, Response, Ui};
use logisim_core::Kind;

impl Workspace {
    pub(super) fn dispatch_shortcuts(&mut self, ui: &Ui, response: &Response) {
        if !ui.is_enabled()
            || self.dialog_open()
            || ui.ctx().any_popup_open()
            || egui::Popup::is_any_open(ui.ctx())
            || ui.ctx().text_edit_focused()
            || self.editor.simulation.plugin_keyboard.is_some()
            || ui.input(|i| !i.focused || i.events.iter().any(|e| matches!(e, Event::Ime(_))))
        {
            return;
        }
        let canvas_active =
            response.has_focus() || (response.hovered() && !ui.ctx().egui_wants_keyboard_input());
        let terminal = self.editor.simulation.keyboard_focus.is_some();
        let bindings = self.preferences.shortcuts;
        let mac = ui.ctx().os().is_mac();
        let mut clipboard_commands = Vec::new();
        // Execute in input order, outside the context lock. A command can switch
        // editors or open a dialog before the next event in the very same frame.
        let mut events = ui.input_mut(|input| std::mem::take(&mut input.events));
        events.retain(|event| {
            if self.dialog_open() {
                return true;
            }
            let scope = if self.editor.session.context.appearance_mode {
                Scope::Appearance
            } else {
                Scope::Canvas
            };
            use Command::*;
            let command = match event {
                Event::Key {
                    pressed: true,
                    key,
                    modifiers,
                    ..
                } => {
                    let chord = Chord::from_event(*key, *modifiers, mac);
                    let Some(command) = bindings.command(chord, scope, mac) else {
                        return true;
                    };
                    if terminal && (command.group() != Group::Simulation || modifiers.is_none()) {
                        return true;
                    }
                    command
                }
                Event::Copy if !terminal => {
                    if scope == Scope::Appearance {
                        AppearanceCopy
                    } else {
                        Copy
                    }
                }
                Event::Cut if !terminal => {
                    if scope == Scope::Appearance {
                        AppearanceCut
                    } else {
                        Cut
                    }
                }
                Event::Paste(_) if !terminal && scope == Scope::Appearance => AppearancePaste,
                Event::Paste(text) if !terminal && text == CLIPBOARD_MARKER => Paste,
                _ => return true,
            };
            if command.scope() != Scope::Global && !canvas_active {
                return true;
            }
            // Some backends deliver both the physical shortcut and its native
            // clipboard event. Publish/cut/paste once in the receiving editor.
            if matches!(
                command,
                Copy | Cut | Paste | AppearanceCopy | AppearanceCut | AppearancePaste
            ) {
                if clipboard_commands.contains(&command) {
                    return false;
                }
                clipboard_commands.push(command);
            }
            if self.command_enabled(command) {
                self.run_command(command);
            }
            false
        });
        ui.input_mut(|input| input.events = events);
    }
    pub(super) fn command_enabled(&self, c: Command) -> bool {
        use Command::*;
        let editor = &self.editor;
        match c {
            Undo | AppearanceUndo => editor.can_undo(),
            Redo | AppearanceRedo => editor.can_redo(),
            Cut | Copy | Duplicate => !editor.session.selection.is_empty(),
            Paste => editor.can_paste(),
            AppearanceCut | AppearanceCopy | AppearanceDuplicate | AppearanceRaise
            | AppearanceLower | AppearanceFront | AppearanceBack => {
                self.appearance.has_selected_shape()
            }
            AppearancePaste => !self.appearance.clipboard.is_empty(),
            AppearanceSelectAll => true,
            ToolRotate => editor.can_rotate(),
            ReturnParent => !editor.session.context.path.is_empty(),
            Step => !editor.simulation.view.enabled && !editor.simulation.view.limited,
            Tick => editor.simulation.can_tick(),
            AutomaticTicks => editor.simulation.view.has_clocks,
            New | Open | Save | SaveAs | Exit | SelectAll | AddCircuit | ToggleAppearance
            | ToolSelect | ToolPoke | ToolWire | ToolText | ToolTunnel | Toggle | Reset => true,
        }
    }
    pub(super) fn run_command(&mut self, c: Command) {
        use Command::*;
        match c {
            New => self.file_actions.push(FileAction::New),
            Open => self.file_actions.push(FileAction::Open),
            Save => self.file_actions.push(FileAction::Save),
            SaveAs => self.file_actions.push(FileAction::SaveAs),
            Exit => self.file_actions.push(FileAction::Close),
            AppearanceUndo => self.appearance.history(&mut self.editor, Editor::undo),
            AppearanceRedo => self.appearance.history(&mut self.editor, Editor::redo),
            AppearanceCut => self.appearance.command(&mut self.editor, Art::Cut),
            AppearanceCopy => self.appearance.command(&mut self.editor, Art::Copy),
            AppearancePaste => self.appearance.command(&mut self.editor, Art::Paste),
            AppearanceDuplicate => self.appearance.command(&mut self.editor, Art::Duplicate),
            AppearanceSelectAll => self.appearance.command(&mut self.editor, Art::All),
            AppearanceRaise => self.appearance.command(&mut self.editor, Art::Raise),
            AppearanceLower => self.appearance.command(&mut self.editor, Art::Lower),
            AppearanceFront => self.appearance.command(&mut self.editor, Art::Front),
            AppearanceBack => self.appearance.command(&mut self.editor, Art::Back),
            Undo => self.editor.undo(),
            Redo => self.editor.redo(),
            Copy => self.editor.copy(),
            Cut => self.editor.cut(),
            Paste => self.editor.paste(),
            Duplicate => self.editor.duplicate(),
            SelectAll => {
                let circuit = self.circuit();
                if !circuit.components.is_empty() || !circuit.wires.is_empty() {
                    self.editor.select_all();
                }
            }
            AddCircuit => self.circuit_command(CircuitCommand::Add),
            ReturnParent => self.editor.leave_instance(),
            ToolSelect => self.editor.set_tool(Tool::Select),
            ToolPoke => self.editor.set_tool(Tool::Poke),
            ToolWire => self.editor.set_tool(Tool::Wire),
            ToolText => self.editor.set_tool(Tool::Place(Kind::Text)),
            ToolTunnel => self.editor.set_tool(Tool::Place(Kind::Tunnel)),
            ToolRotate => self.editor.rotate(),
            ToggleAppearance => {
                self.finish_edit();
                self.editor.session.context.appearance_mode =
                    !self.editor.session.context.appearance_mode;
            }
            Toggle => self
                .editor
                .simulation
                .actions
                .push(SimulationAction::ToggleEnabled),
            Step => self.editor.simulation.actions.push(SimulationAction::Step),
            Reset => self.editor.simulation.actions.push(SimulationAction::Reset),
            Tick => self.editor.simulation.actions.push(SimulationAction::Tick),
            AutomaticTicks => self
                .editor
                .simulation
                .actions
                .push(SimulationAction::ToggleTicks),
        }
    }
}
