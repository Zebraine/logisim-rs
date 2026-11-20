//! Route workspace commands once, after the canvas and text fields have resolved focus.
use crate::{
    FileAction, SimulationAction, Workspace,
    editor::CLIPBOARD_MARKER,
    project::CircuitCommand,
    shortcuts::{Chord, Command, Group, Scope},
};
use egui::{Event, Response, Ui};

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
        let canvas = !self.editor.session.context.appearance_mode
            && (response.has_focus()
                || (response.hovered() && !ui.ctx().egui_wants_keyboard_input()));
        let terminal = self.editor.simulation.keyboard_focus.is_some();
        let bindings = self.preferences.shortcuts;
        let mac = ui.ctx().os().is_mac();
        let mut commands = Vec::new();
        ui.input_mut(|i| {
            i.events.retain(|event| {
                let command = match event {
                    Event::Key {
                        pressed: true,
                        key,
                        modifiers,
                        ..
                    } => {
                        let chord = Chord::from_event(*key, *modifiers, mac);
                        let Some(command) = bindings.command(chord, mac) else {
                            return true;
                        };
                        // The simulated keyboard receives text, while its modified
                        // simulation controls continue to operate the running circuit.
                        if terminal && (command.group() != Group::Simulation || modifiers.is_none())
                        {
                            return true;
                        }
                        command
                    }
                    Event::Copy if !terminal => Command::Copy,
                    Event::Cut if !terminal => Command::Cut,
                    Event::Paste(text) if !terminal && text == CLIPBOARD_MARKER => Command::Paste,
                    _ => return true,
                };
                if command.scope() == Scope::Canvas && !canvas {
                    return true;
                }
                // A platform clipboard event can accompany its key event. Other
                // actions, especially consecutive clock edges, stay ordered.
                if !matches!(command, Command::Cut | Command::Copy | Command::Paste)
                    || !commands.contains(&command)
                {
                    commands.push(command);
                }
                false
            })
        });
        for command in commands {
            if self.dialog_open() {
                break;
            }
            if self.command_enabled(command) {
                self.run_command(command);
            }
        }
    }
    pub(super) fn command_enabled(&self, c: Command) -> bool {
        use Command::*;
        let editor = &self.editor;
        match c {
            Undo => editor.can_undo(),
            Redo => editor.can_redo(),
            Cut | Copy | Duplicate => !editor.session.selection.is_empty(),
            Paste => editor.can_paste(),
            ReturnParent => !editor.session.context.path.is_empty(),
            Step => !editor.simulation.view.enabled && !editor.simulation.view.limited,
            Tick => editor.simulation.can_tick(),
            AutomaticTicks => editor.simulation.view.has_clocks,
            New | Open | Save | SaveAs | Exit | SelectAll | AddCircuit | ToggleAppearance
            | Toggle | Reset => true,
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
