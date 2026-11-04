use logisim_core::Circuit;
use logisim_sim::{Bit, Problem, Simulator, Value};
use logisim_ui::{
    InputValue, SignalTone, SignalView, SimulationAction, SimulationIssue, SimulationView,
};

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
                view.ports
                    .insert((c.id, index), signal(value, sim.port_mismatch(c.id, index)));
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
    for diagnostic in sim.diagnostics() {
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
                    format!("{} #{}", c.kind.name(), c.id)
                } else {
                    c.label.clone()
                }
            })
            .collect();
        view.issues.push(SimulationIssue {
            message: format!("{message}\n{}", names.join(", ")),
            targets: diagnostic
                .components
                .into_iter()
                .chain(diagnostic.wires)
                .collect(),
        });
    }
    view
}
fn signal(value: Value, mismatch: bool) -> SignalView {
    let tone = if mismatch {
        SignalTone::WidthMismatch
    } else if value.contains(Bit::Error) {
        SignalTone::Error
    } else if value.contains(Bit::Unknown) {
        SignalTone::Unknown
    } else if value.width() > 1 {
        SignalTone::Bus
    } else if value.number() == Some(1) {
        SignalTone::High
    } else {
        SignalTone::Low
    };
    let text = if let Some(number) = value.number() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use logisim_core::{Component, Kind, Point};
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
