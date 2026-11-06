use logisim_core::{Circuit, Kind, Radix};
use logisim_sim::{Bit, Problem, Simulator, Value};
use logisim_ui::i18n::tr;
use logisim_ui::{
    InputValue, SignalTone, SignalView, SimulationAction, SimulationIssue, SimulationView,
};

#[cfg(test)]
pub fn apply(sim: &mut Simulator, action: SimulationAction) {
    match action {
        SimulationAction::ToggleEnabled => sim.set_enabled(!sim.enabled()),
        SimulationAction::Step if !sim.enabled() => {
            sim.step();
        }
        SimulationAction::Step => {}
        SimulationAction::Reset => sim.reset(),
        SimulationAction::ToggleInput { id, bit } => sim.toggle_input(id, bit),
        SimulationAction::SetInput { id, value } => sim.set_input(id, value),
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
        if let Some(value) = sim.input(c.id) {
            view.inputs.insert(
                c.id,
                InputValue {
                    bits: value.width(),
                    value: value.number().unwrap_or(0),
                },
            );
        }
    }
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
        SimulationAction::ToggleEnabled => sim.set_enabled(!sim.enabled()),
        SimulationAction::Step if !sim.enabled() => {
            sim.step();
        }
        SimulationAction::Reset => sim.reset(),
        SimulationAction::ToggleInput { id, bit } if path.is_empty() => sim.toggle_input(id, bit),
        SimulationAction::SetInput { id, value } if path.is_empty() => sim.set_input(id, value),
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
    if !path.is_empty() {
        view.inputs.clear();
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
