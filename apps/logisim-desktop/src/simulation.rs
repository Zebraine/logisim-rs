use logisim_core::{Circuit, Kind, Radix};
use logisim_sim::{Bit, Problem, Simulator, Value};
use logisim_ui::i18n::tr;
use logisim_ui::{
    InputValue, SignalTone, SignalView, SimulationAction, SimulationIssue, SimulationView,
};

#[cfg(test)]
pub fn apply(sim: &mut Simulator, action: SimulationAction) {
    match action {
        SimulationAction::SetRegisterFileWord { id, address, value } => {
            sim.set_register_file_word(id, address, value)
        }
        SimulationAction::PressButton { id } => sim.set_button(id, true),
        SimulationAction::ReleaseButtons => sim.release_buttons(),
        SimulationAction::SetJoystick { id, offset } => sim.set_joystick(id, offset),
        SimulationAction::ReleaseJoysticks => sim.release_joysticks(),
        SimulationAction::TypeCharacter { id, value } => sim.type_character(id, value),
        SimulationAction::ToggleEnabled => sim.set_enabled(!sim.enabled()),
        SimulationAction::Step if !sim.enabled() => {
            sim.step();
        }
        SimulationAction::Step => {}
        SimulationAction::Reset => sim.reset(),
        SimulationAction::Tick => {
            sim.tick();
        }
        SimulationAction::PokeClock { id } => sim.poke_clock(id),
        SimulationAction::ToggleStored { id } => sim.toggle_stored(id),
        SimulationAction::ToggleTicks | SimulationAction::SetTickRate(_) => {}
        SimulationAction::ToggleInput { id, bit } => sim.toggle_input(id, bit),
        SimulationAction::SetInput { id, value } => sim.set_input(id, value),
        SimulationAction::SetInputUnknown { id } => sim.set_input_unknown(id),
        SimulationAction::FillMemory {
            id,
            start,
            end,
            value,
        } => sim.fill_memory(id, start, end, value),
    }
}

pub fn present(circuit: &Circuit, sim: &Simulator) -> SimulationView {
    let mut view = SimulationView {
        enabled: sim.enabled(),
        pending: sim.pending_count(),
        steps: sim.steps(),
        limited: sim.limited(),
        ..Default::default()
    };
    for wire in &circuit.wires {
        if let Some(value) = sim.wire_value(wire.id) {
            view.wires
                .insert(wire.id, signal(value, sim.wire_mismatch(wire.id)));
        }
    }
    for c in &circuit.components {
        for index in 0..c.ports().len() {
            if let Some(value) = sim.port_value(c.id, index) {
                let mut presented = signal(value, sim.port_mismatch(c.id, index));
                if c.kind == Kind::Probe {
                    presented.text = probe_text(value, c.radix);
                }
                view.ports.insert((c.id, index), presented);
            }
        }
        if c.kind == Kind::Clock {
            view.clocks.insert(c.id);
        }
        if let Some(words) = sim.register_file(c.id) {
            view.register_files
                .insert(c.id, words.iter().map(|v| signal(*v, false)).collect());
        }
        if let Some(contents) = sim.memory_contents(c.id) {
            view.memories.insert(c.id, contents.clone());
        }
        if let Some(value) = sim.stored_value(c.id) {
            view.storage.insert(c.id, signal(value, false));
        }
        if let Some(rgb) = sim.rgb_color(c.id) {
            view.rgb.insert(c.id, rgb);
        }
        if let Some(cells) = sim.matrix_cells(c.id) {
            view.matrices.insert(
                c.id,
                cells
                    .into_iter()
                    .map(|bit| match bit {
                        Bit::One => Some(true),
                        Bit::Zero => Some(false),
                        _ => None,
                    })
                    .collect(),
            );
        }
        if let Some(buffer) = sim.keyboard_buffer(c.id) {
            view.keyboards.insert(c.id, buffer);
        }
        if let Some(screen) = sim.tty(c.id) {
            view.ttys.insert(
                c.id,
                logisim_ui::TtyView {
                    lines: screen.lines.clone(),
                    row: screen.row,
                    column: screen.column,
                },
            );
        }
        if let Some(offset) = sim.joystick(c.id) {
            view.joysticks.insert(c.id, offset);
        }
        if let Some(lights) = sim.indicator(c.id) {
            view.indicators.insert(c.id, lights);
        }
        if c.kind == Kind::Button {
            view.buttons
                .insert(c.id, sim.input(c.id).is_some_and(|v| v.number() == Some(1)));
        }
        if c.kind != Kind::Button
            && let Some(value) = sim.input(c.id)
        {
            view.inputs.insert(c.id, input_value(value));
        }
    }
    view.has_clocks = sim.has_clocks();
    append_issues(&mut view, circuit, sim.diagnostics());
    view
}
fn append_issues(
    view: &mut SimulationView,
    circuit: &Circuit,
    diagnostics: Vec<logisim_sim::Diagnostic>,
) {
    for diagnostic in diagnostics {
        let message = match diagnostic.problem {
            Problem::WidthMismatch => "Incompatible bit widths",
            Problem::ErrorSignal => "Error signal: conflicting drivers / undefined input",
            Problem::PropagationLimit => "Propagation limit: check feedback, then Reset or Run",
        };
        let names: Vec<_> = diagnostic
            .components
            .iter()
            .take(3)
            .filter_map(|id| circuit.component(*id))
            .map(|c| {
                if c.label.is_empty() {
                    format!("{} #{}", tr(c.kind.name()), c.id)
                } else {
                    c.label.clone()
                }
            })
            .collect();
        view.issues.push(SimulationIssue {
            message: format!("{}\n{}", tr(message), names.join(", ")),
            targets: diagnostic
                .components
                .into_iter()
                .chain(diagnostic.wires)
                .collect(),
        });
    }
}
pub fn apply_project(
    sim: &mut logisim_sim::ProjectSimulator,
    path: &[logisim_core::Id],
    action: SimulationAction,
) {
    match action {
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
        _ => {}
    }
}
pub fn present_project(
    circuit: &Circuit,
    sim: &logisim_sim::ProjectSimulator,
    path: &[logisim_core::Id],
) -> SimulationView {
    let mut view = sim
        .view(path)
        .map(|s| present(circuit, s))
        .unwrap_or_default();
    view.enabled = sim.enabled();
    view.pending = sim.pending_count();
    view.steps = sim.steps();
    view.limited = sim.limited();
    view.has_clocks = sim.has_clocks();
    view.ticks = sim.ticks();
    if !path.is_empty() {
        view.inputs.retain(|id, _| {
            circuit
                .component(*id)
                .is_some_and(|c| matches!(c.kind, Kind::Switch | Kind::DipSwitch))
        });
    }
    view.issues.clear();
    append_issues(&mut view, circuit, sim.diagnostics(path));
    if let Some(error) = sim.error() {
        view.issues.push(SimulationIssue {
            message: tr(error).into(),
            targets: Default::default(),
        });
    }
    view
}

fn input_value(value: Value) -> InputValue {
    let mut input = InputValue {
        bits: value.width(),
        ..Default::default()
    };
    for bit in 0..value.width() {
        match value.bit(bit) {
            Bit::One => input.value |= 1 << bit,
            Bit::Unknown => input.unknown |= 1 << bit,
            Bit::Error => input.error |= 1 << bit,
            Bit::Zero => {}
        }
    }
    input
}

fn signal(value: Value, mismatch: bool) -> SignalView {
    let tone = if mismatch {
        SignalTone::WidthMismatch
    } else if value.contains(Bit::Error) {
        SignalTone::Error
    } else if value.width() == 0 || value.contains(Bit::Unknown) {
        SignalTone::Unknown
    } else if value.width() > 1 {
        SignalTone::Bus
    } else if value.number() == Some(1) {
        SignalTone::High
    } else {
        SignalTone::Low
    };
    let text = if value.width() == 0 {
        "—".into()
    } else if let Some(number) = value.number() {
        InputValue {
            bits: value.width(),
            value: number,
            ..Default::default()
        }
        .text()
    } else if value.contains(Bit::Error) {
        "E".into()
    } else {
        "X".into()
    };
    SignalView {
        text,
        binary: value.binary(),
        tone,
    }
}

fn probe_text(value: Value, radix: Radix) -> String {
    if value.width() == 0 {
        return "—".into();
    }
    if radix == Radix::Binary {
        return value.binary();
    }
    if let Some(n) = value.number() {
        return match radix {
            Radix::Octal => format!(
                "{n:0width$o}",
                width = usize::from(value.width().div_ceil(3))
            ),
            Radix::Hex => format!(
                "{n:0width$X}",
                width = usize::from(value.width().div_ceil(4))
            ),
            Radix::Signed => {
                let signed = if n & (1 << (value.width() - 1)) != 0 {
                    i64::from(n) - (1_i64 << value.width())
                } else {
                    i64::from(n)
                };
                signed.to_string()
            }
            Radix::Unsigned => n.to_string(),
            Radix::Binary => unreachable!(),
        };
    }
    if matches!(radix, Radix::Octal | Radix::Hex) {
        let size = if radix == Radix::Octal { 3 } else { 4 };
        return (0..value.width().div_ceil(size))
            .rev()
            .map(|group| {
                let bits: Vec<_> = (group * size..((group + 1) * size).min(value.width()))
                    .map(|i| value.bit(i))
                    .collect();
                if bits.contains(&Bit::Error) {
                    'E'
                } else if bits.contains(&Bit::Unknown) {
                    'X'
                } else {
                    let n = bits
                        .iter()
                        .enumerate()
                        .fold(0, |n, (i, b)| n | (u32::from(*b == Bit::One) << i));
                    char::from_digit(n, if radix == Radix::Octal { 8 } else { 16 })
                        .unwrap()
                        .to_ascii_uppercase()
                }
            })
            .collect();
    }
    if value.contains(Bit::Error) {
        "E".into()
    } else {
        "X".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use logisim_core::{Component, Kind, Point};
    #[test]
    fn nested_switches_are_pokable_but_parent_driven_pins_are_protected() {
        let mut p = logisim_core::Project::default();
        let root = p.main;
        let cell = p.add("Cell".into()).unwrap();
        let c = &mut p.circuit_mut(cell).unwrap().circuit;
        let pin = c.add_component(Component::new(Kind::Pin, Point::new(100, 100)));
        let switch = c.add_component(Component::new(Kind::Switch, Point::new(200, 100)));
        let dip = c.add_component(Component::new(Kind::DipSwitch, Point::new(200, 200)));
        let instance = p.instance(cell, Point::new(300, 100)).unwrap();
        let id = p.circuit_mut(root).unwrap().circuit.add_component(instance);
        let mut sim = logisim_sim::ProjectSimulator::default();
        sim.synchronize(&p, root);
        sim.advance(64);
        let before = sim.view(&[id]).unwrap().input(pin);
        apply_project(
            &mut sim,
            &[id],
            SimulationAction::SetInput { id: pin, value: 1 },
        );
        apply_project(
            &mut sim,
            &[id],
            SimulationAction::ToggleInput { id: pin, bit: 0 },
        );
        assert_eq!(sim.view(&[id]).unwrap().input(pin), before);
        apply_project(
            &mut sim,
            &[id],
            SimulationAction::ToggleInput { id: switch, bit: 0 },
        );
        apply_project(
            &mut sim,
            &[id],
            SimulationAction::SetInput { id: dip, value: 9 },
        );
        sim.advance(64);
        let view = present_project(&p.circuit(cell).unwrap().circuit, &sim, &[id]);
        assert!(!view.inputs.contains_key(&pin));
        assert_eq!(view.inputs[&switch].value, 1);
        assert_eq!(view.inputs[&switch].bits, 1);
        assert_eq!(view.inputs[&dip].value, 9);
        assert_eq!(view.inputs[&dip].bits, 4);
        assert_eq!(view.ports[&(dip, 3)].tone, SignalTone::High);
    }
    #[test]
    fn probe_formats_runtime_values_and_full_width_signed_numbers() {
        let value = Value::known(8, 0xa6);
        for (radix, expected) in [
            (Radix::Binary, "10100110"),
            (Radix::Octal, "246"),
            (Radix::Signed, "-90"),
            (Radix::Unsigned, "166"),
            (Radix::Hex, "A6"),
        ] {
            assert_eq!(probe_text(value, radix), expected);
        }
        assert_eq!(probe_text(Value::known(32, u32::MAX), Radix::Signed), "-1");
        assert_eq!(
            probe_text(Value::known(32, 0x8000_0000), Radix::Signed),
            "-2147483648"
        );
        assert_eq!(probe_text(Value::known(0, 0), Radix::Binary), "—");
    }
    #[test]
    fn tristate_input_presentation_does_not_turn_unknown_bits_into_zero() {
        let mut circuit = Circuit::default();
        let mut pin = Component::new(Kind::Pin, Point::ZERO);
        pin.bits = 8;
        pin.pin.three_state = true;
        let id = circuit.add_component(pin);
        let mut sim = Simulator::default();
        sim.synchronize(&circuit);
        sim.advance(8);
        assert_eq!(present(&circuit, &sim).inputs[&id].text(), "XX");
        apply(&mut sim, SimulationAction::SetInput { id, value: 0xab });
        apply(&mut sim, SimulationAction::ToggleInput { id, bit: 0 });
        let input = present(&circuit, &sim).inputs[&id];
        assert_eq!(input.text(), "AX");
        assert_eq!(input.bit_text(0), "X");
        assert_eq!(input.bit_text(1), "1");
        assert_eq!(input.bit_text(2), "0");
        apply(&mut sim, SimulationAction::SetInputUnknown { id });
        assert_eq!(present(&circuit, &sim).inputs[&id].unknown, 255);
    }
    #[test]
    fn host_commands_and_presentation_keep_input_and_net_values_distinct_while_paused() {
        let mut circuit = Circuit::default();
        let id = circuit.add_component(Component::new(Kind::Pin, Point::ZERO));
        let mut sim = Simulator::default();
        sim.synchronize(&circuit);
        sim.advance(8);
        apply(&mut sim, SimulationAction::ToggleEnabled);
        apply(&mut sim, SimulationAction::ToggleInput { id, bit: 0 });
        let view = present(&circuit, &sim);
        assert!(!view.enabled);
        assert_eq!(view.inputs[&id].value, 1);
        assert_eq!(view.ports[&(id, 0)].tone, SignalTone::Low);
        apply(&mut sim, SimulationAction::Step);
        assert_eq!(
            present(&circuit, &sim).ports[&(id, 0)].tone,
            SignalTone::High
        );
        apply(&mut sim, SimulationAction::Reset);
        assert_eq!(sim.input(id).unwrap().number(), Some(0));
        assert_eq!(circuit.component(id).unwrap().position, Point::ZERO);
    }
}
