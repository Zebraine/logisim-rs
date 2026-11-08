use super::values::probe_text;
use super::*;
use logisim_core::{Circuit, Project, Radix};
use logisim_core::{Component, Kind, Point};
use logisim_sim::{ProjectSimulator, Value};
use logisim_ui::{SignalTone, SimulationAction};

#[test]
fn presentation_retains_each_display_family_and_prunes_removed_devices() {
    let kinds = [
        Kind::Led,
        Kind::SevenSegment,
        Kind::HexDisplay,
        Kind::RgbLed,
        Kind::DotMatrix,
        Kind::Keyboard,
        Kind::Tty,
        Kind::Joystick,
        Kind::Register,
        Kind::Ram,
    ];
    let mut project = Project::default();
    let circuit = &mut project.main_circuit_mut().circuit;
    let ids: Vec<_> = kinds
        .into_iter()
        .enumerate()
        .map(|(i, kind)| {
            circuit.add_component(Component::new(kind, Point::new(500 * i as i32, 100)))
        })
        .collect();
    let ordinary = circuit.add_component(Component::new(
        Kind::Gate(logisim_core::Gate::Not),
        Point::new(6000, 100),
    ));
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&project, project.main);
    sim.advance(128);
    let view = present_project(&project.main_circuit().circuit, &sim, &[]);
    assert_eq!(
        view.indicators.keys().copied().collect::<Vec<_>>(),
        ids[..3]
    );
    assert_eq!(view.rgb.keys().copied().collect::<Vec<_>>(), [ids[3]]);
    assert_eq!(view.matrices.keys().copied().collect::<Vec<_>>(), [ids[4]]);
    assert_eq!(view.keyboards.keys().copied().collect::<Vec<_>>(), [ids[5]]);
    assert_eq!(view.ttys.keys().copied().collect::<Vec<_>>(), [ids[6]]);
    assert_eq!(view.joysticks.keys().copied().collect::<Vec<_>>(), [ids[7]]);
    assert_eq!(view.storage.keys().copied().collect::<Vec<_>>(), [ids[8]]);
    assert_eq!(view.memories.keys().copied().collect::<Vec<_>>(), [ids[9]]);
    assert!(view.ports.contains_key(&(ordinary, 0)));
    project
        .main_circuit_mut()
        .circuit
        .remove(&ids.into_iter().collect());
    sim.synchronize(&project, project.main);
    sim.advance(128);
    let view = present_project(&project.main_circuit().circuit, &sim, &[]);
    assert!(view.indicators.is_empty());
    assert!(view.rgb.is_empty());
    assert!(view.matrices.is_empty());
    assert!(view.keyboards.is_empty());
    assert!(view.ttys.is_empty());
    assert!(view.joysticks.is_empty());
    assert!(view.storage.is_empty());
    assert!(view.memories.is_empty());
}

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
    let project = Project::single("main", circuit);
    let circuit = &project.main_circuit().circuit;
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&project, project.main);
    sim.advance(8);
    assert_eq!(present_project(circuit, &sim, &[]).inputs[&id].text(), "XX");
    apply_project(
        &mut sim,
        &[],
        SimulationAction::SetInput { id, value: 0xab },
    );
    apply_project(&mut sim, &[], SimulationAction::ToggleInput { id, bit: 0 });
    let input = present_project(circuit, &sim, &[]).inputs[&id];
    assert_eq!(input.text(), "AX");
    assert_eq!(input.bit_text(0), "X");
    assert_eq!(input.bit_text(1), "1");
    assert_eq!(input.bit_text(2), "0");
    apply_project(&mut sim, &[], SimulationAction::SetInputUnknown { id });
    assert_eq!(present_project(circuit, &sim, &[]).inputs[&id].unknown, 255);
}
#[test]
fn host_commands_and_presentation_keep_input_and_net_values_distinct_while_paused() {
    let mut circuit = Circuit::default();
    let id = circuit.add_component(Component::new(Kind::Pin, Point::ZERO));
    let project = Project::single("main", circuit);
    let circuit = &project.main_circuit().circuit;
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&project, project.main);
    sim.advance(8);
    apply_project(&mut sim, &[], SimulationAction::ToggleEnabled);
    apply_project(&mut sim, &[], SimulationAction::ToggleInput { id, bit: 0 });
    let view = present_project(circuit, &sim, &[]);
    assert!(!view.enabled);
    assert_eq!(view.inputs[&id].value, 1);
    assert_eq!(view.ports[&(id, 0)].tone, SignalTone::Low);
    apply_project(&mut sim, &[], SimulationAction::Step);
    assert_eq!(
        present_project(circuit, &sim, &[]).ports[&(id, 0)].tone,
        SignalTone::High
    );
    apply_project(&mut sim, &[], SimulationAction::Reset);
    assert_eq!(sim.input(id).unwrap().number(), Some(0));
    assert_eq!(circuit.component(id).unwrap().position, Point::ZERO);
}
