use super::*;
use crate::Simulator;
use logisim_core::{Circuit, Facing, Id, Point};
fn setup(kind: Ttl, facing: Facing, power: bool) -> (Circuit, Component, Vec<(u8, Id)>, Simulator) {
    let mut c = Component::new(Kind::Ttl(kind), Point::new(300, 300));
    c.facing = facing;
    c.ttl.power_pins = power;
    let mut circuit = Circuit::default();
    let mut inputs = vec![];
    for (i, p) in c.ports().into_iter().enumerate() {
        if !c.drives_port(i) {
            inputs.push((
                c.ttl_pins()[i],
                circuit.add_component(Component::new(Kind::Pin, p)),
            ));
        }
    }
    c.id = circuit.add_component(c.clone());
    let mut sim = Simulator::default();
    sim.synchronize(&circuit);
    (circuit, c, inputs, sim)
}
fn set(sim: &mut Simulator, inputs: &[(u8, Id)], pin: u8, v: Bit) {
    sim.external_input(
        inputs.iter().find(|p| p.0 == pin).unwrap().1,
        Value::filled(1, v),
    );
}
fn settle(sim: &mut Simulator) {
    sim.advance(128);
    assert_eq!(sim.pending_count(), 0);
    assert!(!sim.limited());
}
fn bit(s: &Simulator, c: &Component, pin: u8) -> Bit {
    s.port_value(c.id, c.ttl_index(pin).unwrap())
        .unwrap()
        .bit(0)
}
fn decode(c: char) -> Bit {
    match c {
        '0' => Bit::Zero,
        '1' => Bit::One,
        'X' => Bit::Unknown,
        _ => Bit::Error,
    }
}
#[test]
fn observed_four_state_matrices_all_gates_and_orientations() {
    for (kind, expected) in [
        (Ttl::Ic7400, "111110EE1EEE1EEE"),
        (Ttl::Ic7402, "10EE0000E0EEE0EE"),
        (Ttl::Ic7404, "11110000EEEEEEEE"),
        (Ttl::Ic7408, "000001EE0EEE0EEE"),
        (Ttl::Ic7432, "01EE1111E1EEE1EE"),
        (Ttl::Ic7486, "01EE10EEEEEEEEEE"),
        (Ttl::Ic747266, "10EE01EEEEEEEEEE"),
    ] {
        for facing in Facing::ALL {
            let (_, c, inputs, mut sim) = setup(kind, facing, false);
            // Each gate receives a different matrix cell, then walks through all cells.
            for step in 0..16 {
                for (g, &[a, b, _]) in kind.gates().iter().enumerate() {
                    let i = (step + g) % 16;
                    set(
                        &mut sim,
                        &inputs,
                        a,
                        decode("01XE".chars().nth(i / 4).unwrap()),
                    );
                    if a != b {
                        set(
                            &mut sim,
                            &inputs,
                            b,
                            decode("01XE".chars().nth(i % 4).unwrap()),
                        );
                    }
                }
                settle(&mut sim);
                for (g, &[_, _, out]) in kind.gates().iter().enumerate() {
                    assert_eq!(
                        bit(&sim, &c, out),
                        decode(expected.chars().nth((step + g) % 16).unwrap()),
                        "{kind:?} {facing:?} gate{g} step{step}"
                    );
                }
            }
        }
        for wired in [false, true] {
            let mut circuit = Circuit::default();
            let mut c = Component::new(Kind::Ttl(kind), Point::new(300, 300));
            if wired {
                for (i, p) in c.ports().into_iter().enumerate() {
                    if !c.drives_port(i) {
                        circuit.add_wire(p, p + Point::new(0, if p.y > 300 { 40 } else { -40 }));
                    }
                }
            }
            c.id = circuit.add_component(c.clone());
            let mut sim = Simulator::default();
            sim.synchronize(&circuit);
            settle(&mut sim);
            for &[_, _, o] in kind.gates() {
                assert_eq!(bit(&sim, &c, o), Bit::Error);
            }
        }
    }
}
#[test]
fn observed_supply_matrix_release_bus_conflicts_and_live_edits() {
    for kind in Ttl::ALL {
        let (mut circuit, mut c, inputs, mut sim) = setup(kind, Facing::East, true);
        let expected = match kind {
            Ttl::Ic7408 | Ttl::Ic7432 | Ttl::Ic7486 => Bit::Zero,
            _ => Bit::One,
        };
        for vcc in "01XE".chars() {
            for gnd in "01XE".chars() {
                set(&mut sim, &inputs, 14, decode(vcc));
                set(&mut sim, &inputs, 7, decode(gnd));
                settle(&mut sim);
                for &[_, _, o] in kind.gates() {
                    assert_eq!(
                        bit(&sim, &c, o),
                        if vcc == '1' && gnd == '0' {
                            expected
                        } else {
                            Bit::Unknown
                        }
                    );
                }
            }
        }
        let output = kind.gates()[0][2];
        let ext = circuit.add_component(Component::new(
            Kind::Pin,
            c.ports()[c.ttl_index(output).unwrap()],
        ));
        sim.synchronize(&circuit);
        for powered in [false, true] {
            for forced in [Bit::Zero, Bit::One] {
                set(
                    &mut sim,
                    &inputs,
                    14,
                    if powered { Bit::One } else { Bit::Zero },
                );
                set(&mut sim, &inputs, 7, Bit::Zero);
                sim.external_input(ext, Value::filled(1, forced));
                settle(&mut sim);
                assert_eq!(
                    bit(&sim, &c, output),
                    if powered && forced != expected {
                        Bit::Error
                    } else {
                        forced
                    }
                );
            }
        }
        // Display edits leave inputs intact; disabling explicit power recomputes outputs.
        set(&mut sim, &inputs, 14, Bit::Zero);
        settle(&mut sim);
        c.ttl.internal = true;
        circuit.replace_component(c.clone());
        sim.synchronize(&circuit);
        settle(&mut sim);
        assert_eq!(bit(&sim, &c, kind.gates()[1][2]), Bit::Unknown);
        c.ttl.power_pins = false;
        circuit.replace_component(c.clone());
        sim.synchronize(&circuit);
        settle(&mut sim);
        assert_eq!(bit(&sim, &c, kind.gates()[1][2]), expected);
    }
}
#[test]
fn ttl_subcircuit_instances_keep_inputs_and_outputs_independent() {
    use crate::ProjectSimulator;
    use logisim_core::Project;
    let mut child = Circuit::default();
    let mut c = Component::new(Kind::Ttl(Ttl::Ic7404), Point::new(100, 100));
    c.id = child.add_component(c.clone());
    let input = child.add_component(Component::new(
        Kind::Pin,
        c.ports()[c.ttl_index(1).unwrap()],
    ));
    let mut out = Component::new(Kind::Pin, c.ports()[c.ttl_index(2).unwrap()]);
    out.output = true;
    let output = child.add_component(out);
    let mut project = Project::single("main", Circuit::default());
    let def = project.add("chip".into()).unwrap();
    project.circuit_mut(def).unwrap().circuit = child;
    let root = project.main;
    let mut instances = vec![];
    for i in 0..2 {
        instances.push(
            project
                .circuit_mut(root)
                .unwrap()
                .circuit
                .add_component(Component::new(
                    Kind::Subcircuit(def),
                    Point::new(300, 100 + 200 * i),
                )),
        );
    }
    project.refresh_interfaces(false);
    for (i, &id) in instances.iter().enumerate() {
        let instance = project
            .circuit(root)
            .unwrap()
            .circuit
            .component(id)
            .unwrap();
        let port = instance
            .interface
            .ports
            .iter()
            .position(|p| p.pin == input)
            .unwrap();
        let mut constant = Component::new(Kind::Constant, instance.ports()[port]);
        constant.constant = i as u32;
        project
            .circuit_mut(root)
            .unwrap()
            .circuit
            .add_component(constant);
    }
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&project, root);
    sim.advance(128);
    assert!(sim.error().is_none());
    assert_eq!(sim.pending_count(), 0);
    assert!(!sim.limited());
    for (i, &id) in instances.iter().enumerate() {
        assert_eq!(
            sim.view(&[id]).unwrap().port_value(output, 0),
            Some(Value::known(1, 1 - i as u32))
        );
        let instance = project
            .circuit(root)
            .unwrap()
            .circuit
            .component(id)
            .unwrap();
        let port = instance
            .interface
            .ports
            .iter()
            .position(|p| p.pin == output)
            .unwrap();
        assert_eq!(
            sim.view(&[]).unwrap().port_value(id, port),
            Some(Value::known(1, 1 - i as u32))
        );
    }
}
