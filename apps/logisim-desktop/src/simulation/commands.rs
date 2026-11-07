//! Translate UI intents into the same hierarchical commands used by the desktop.
use logisim_ui::SimulationAction;

pub fn apply_project(
    sim: &mut logisim_sim::ProjectSimulator,
    path: &[logisim_core::Id],
    action: SimulationAction,
) {
    match action {
        SimulationAction::PluginAction {
            id,
            action,
            argument,
        } => sim.plugin_action(path, id, action, argument),
        SimulationAction::SetRegisterFileWord { id, address, value } => {
            sim.set_register_file_word(path, id, address, value)
        }
        SimulationAction::PressButton { id } => sim.set_button(path, id, true),
        SimulationAction::SetJoystick { id, offset } => sim.set_joystick(path, id, offset),
        SimulationAction::ReleaseJoysticks => sim.release_joysticks(),
        SimulationAction::TypeCharacter { id, value } => sim.type_character(path, id, value),
        SimulationAction::ReleaseButtons => sim.release_buttons(),
        SimulationAction::ToggleEnabled => sim.set_enabled(!sim.enabled()),
        SimulationAction::Step if !sim.enabled() => {
            sim.step();
        }
        SimulationAction::Reset => sim.reset(),
        SimulationAction::Tick => {
            sim.tick();
        }
        SimulationAction::PokeClock { id } => sim.poke_clock(path, id),
        SimulationAction::FillMemory {
            id,
            start,
            end,
            value,
        } => sim.fill_memory(path, id, start, end, value),
        SimulationAction::ToggleStored { id } => sim.toggle_stored(path, id),
        SimulationAction::ToggleInput { id, bit } => sim.toggle_input_at(path, id, bit),
        SimulationAction::SetInput { id, value } => sim.set_input_at(path, id, value),
        SimulationAction::SetInputUnknown { id } => sim.set_input_unknown_at(path, id),
        SimulationAction::Step
        | SimulationAction::ToggleTicks
        | SimulationAction::SetTickRate(_) => {}
    }
}
