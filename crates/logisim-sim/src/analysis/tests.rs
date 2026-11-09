use super::*;
use logisim_core::{Circuit, Facing, FlipFlop, Gate, Latch, Plexer, Point, generation::GateBasis};

fn specification() -> Analysis {
    let mut a = Analysis::default();
    for name in ["A", "B", "Unused"] {
        a.add_input(name.into()).unwrap();
    }
    for (i, (name, text)) in [("Sum", "A ^ B"), ("Carry", "A & B"), ("One", "1")]
        .into_iter()
        .enumerate()
    {
        a.add_output(name.into()).unwrap();
        a.apply_expression(i, text).unwrap();
    }
    a
}
fn same_table(a: &Analysis, b: &Analysis) {
    assert_eq!(a.inputs(), b.inputs());
    for (a, b) in a.outputs().iter().zip(b.outputs()) {
        assert_eq!(a.name(), b.name());
        assert_eq!(a.values(), b.values());
    }
    assert_eq!(a.outputs().len(), b.outputs().len());
}
fn generated() -> Project {
    Project::single("logic", specification().generate().unwrap())
}
fn wrap(p: &mut Project, child: Id, name: &str) -> Id {
    let root = p.add(name.into()).unwrap();
    let instance = p.instance(child, Point::new(400, 400)).unwrap();
    let circuit = &mut p.circuit_mut(root).unwrap().circuit;
    for (port, pos) in instance.interface.ports.iter().zip(instance.ports()) {
        let mut pin = Component::new(Kind::Pin, pos);
        pin.output = port.output;
        pin.label = port.label.clone();
        pin.bits = port.bits;
        if pin.output {
            pin.facing = Facing::West;
        }
        circuit.add_component(pin);
    }
    circuit.add_component(instance);
    root
}

#[test]
fn generated_logic_analyzes_regenerates_and_keeps_document_and_runtime_unchanged() {
    let expected = specification();
    for basis in [GateBasis::Standard, GateBasis::Nand] {
        let p = Project::single("logic", expected.generate_with_basis(basis).unwrap());
        let before = p.clone();
        let mut live = ProjectSimulator::default();
        live.synchronize(&p, p.main);
        let pin = p
            .main_circuit()
            .circuit
            .components
            .iter()
            .find(|p| p.label == "A")
            .unwrap()
            .id;
        live.set_input(pin, 1);
        live.settle_compiled().unwrap();
        live.set_enabled(false);
        let state = (
            live.input(pin),
            live.steps(),
            live.ticks(),
            live.pending_count(),
        );
        let a = analyze(&p, p.main).unwrap();
        same_table(&a, &expected);
        same_table(
            &analyze(
                &Project::single("rebuilt", a.generate_with_basis(GateBasis::Nand).unwrap()),
                p.main,
            )
            .unwrap(),
            &expected,
        );
        assert_eq!(p, before);
        assert!(!live.enabled());
        assert_eq!(
            (
                live.input(pin),
                live.steps(),
                live.ticks(),
                live.pending_count()
            ),
            state
        );
    }
}

#[test]
fn recursively_accepts_pure_definitions_and_rejects_disconnected_state_in_them() {
    let mut p = generated();
    let child = p.main;
    let middle = wrap(&mut p, child, "middle");
    let root = wrap(&mut p, middle, "top");
    same_table(&analyze(&p, root).unwrap(), &specification());
    let unrelated = p.add("unrelated".into()).unwrap();
    p.circuit_mut(unrelated)
        .unwrap()
        .circuit
        .add_component(Component::new(Kind::Register, Point::ZERO));
    same_table(&analyze(&p, root).unwrap(), &specification());
    p.circuit_mut(child)
        .unwrap()
        .circuit
        .add_component(Component::new(Kind::Register, Point::ZERO));
    assert!(analyze(&p, root).unwrap_err().message.contains("stateful"));
}

#[test]
fn rejects_state_interaction_and_opaque_or_unsupported_components_even_if_unused() {
    for kind in [
        Kind::Clock,
        Kind::FlipFlop(FlipFlop::D),
        Kind::Latch(Latch::D),
        Kind::Register,
        Kind::Counter,
        Kind::ShiftRegister,
        Kind::Ram,
        Kind::Random,
        Kind::Keyboard,
        Kind::Tty,
        Kind::DotMatrix,
        Kind::Plexer(Plexer::DisplayDecoder),
        Kind::Button,
        Kind::Switch,
        Kind::DipSwitch,
        Kind::Slider,
        Kind::Joystick,
        Kind::Transistor,
        Kind::TransmissionGate,
    ] {
        let mut p = generated();
        p.main_circuit_mut()
            .circuit
            .add_component(Component::new(kind, Point::ZERO));
        assert!(analyze(&p, p.main).is_err(), "{kind:?}");
    }
    let plugin = Component::new(Kind::Plugin, Point::ZERO);
    assert!(matches!(
        classify(&plugin),
        Classification::Boundary(BoundaryReason::Plugin)
    ));
}

#[test]
fn rejects_feedback_width_errors_unknown_outputs_and_conflicting_drivers() {
    let mut p = generated();
    let c = &mut p.main_circuit_mut().circuit;
    let mut gate = Component::new(Kind::Gate(Gate::Not), Point::new(3000, 100));
    let ports = gate.ports();
    c.add_component(gate.clone());
    c.add_wire(ports[0], ports[1]);
    assert!(
        analyze(&p, p.main)
            .unwrap_err()
            .message
            .contains("feedback")
    );
    let mut p = generated();
    let c = &mut p.main_circuit_mut().circuit;
    let output = c.components.iter().find(|p| p.output).unwrap().position;
    let mut constant = Component::new(Kind::Constant, output);
    constant.constant = 1;
    c.add_component(constant);
    assert!(
        analyze(&p, p.main)
            .unwrap_err()
            .message
            .contains("electrical")
    );
    let mut c = Circuit::default();
    let mut output = Component::new(Kind::Pin, Point::new(100, 100));
    output.output = true;
    output.label = "Y".into();
    c.add_component(output);
    let p = Project::single("floating", c);
    assert!(analyze(&p, p.main).unwrap_err().message.contains("unknown"));
    let mut p = generated();
    gate.bits = 2;
    p.main_circuit_mut().circuit.add_component(gate);
    let pos = p
        .main_circuit()
        .circuit
        .components
        .iter()
        .find(|p| p.label == "A")
        .unwrap()
        .position;
    p.main_circuit_mut()
        .circuit
        .add_wire(pos, Point::new(3000, pos.y));
    p.main_circuit_mut()
        .circuit
        .add_wire(Point::new(3000, pos.y), ports[0]);
    assert!(analyze(&p, p.main).unwrap_err().message.contains("widths"));
}

#[test]
fn interface_rules_names_cancellation_and_size_are_explicit() {
    let mut p = generated();
    for c in &mut p.main_circuit_mut().circuit.components {
        if c.kind == Kind::Pin {
            c.label.clear();
        }
    }
    let before = p.clone();
    let a = analyze(&p, p.main).unwrap();
    assert_eq!(a.inputs(), ["a", "b", "c"]);
    assert_eq!(a.outputs()[0].name(), "x");
    assert_eq!(p, before);
    let calls = std::cell::Cell::new(0);
    let error = analyze_cancellable(&p, p.main, || {
        calls.set(calls.get() + 1);
        calls.get() > 10
    })
    .unwrap_err();
    assert!(error.message.contains("cancelled"));
    assert_eq!(p, before);
    let pin = p
        .main_circuit_mut()
        .circuit
        .components
        .iter_mut()
        .find(|c| c.kind == Kind::Pin)
        .unwrap();
    pin.bits = 2;
    assert!(
        analyze(&p, p.main)
            .unwrap_err()
            .message
            .contains("single-bit")
    );
    let mut p = generated();
    let child = p.main;
    let root = wrap(&mut p, child, "bad-interface");
    let c = p
        .circuit_mut(root)
        .unwrap()
        .circuit
        .components
        .iter_mut()
        .find(|c| matches!(c.kind, Kind::Subcircuit(_)))
        .unwrap();
    c.interface.ports.pop();
    assert!(
        analyze(&p, root)
            .unwrap_err()
            .message
            .contains("interfaces")
    );
    let mut p = generated();
    for _ in 0..logisim_core::MAX_CIRCUIT_ITEMS {
        p.main_circuit_mut()
            .circuit
            .add_component(Component::new(Kind::Text, Point::ZERO));
    }
    assert!(
        analyze(&p, p.main)
            .unwrap_err()
            .message
            .contains("item limit")
    );
}

#[test]
fn internal_buses_splitters_and_tunnels_are_analyzed_by_the_existing_net_resolver() {
    let mut c = Circuit::default();
    let mut splitter = Component::new(Kind::Splitter, Point::new(300, 300));
    splitter.bits = 2;
    splitter.splitter.fanout = 2;
    splitter.splitter.mapping[0] = Some(0);
    splitter.splitter.mapping[1] = Some(1);
    let ports = splitter.ports();
    c.add_component(splitter.clone());
    for (i, pos) in ports.iter().skip(1).enumerate() {
        let mut pin = Component::new(Kind::Pin, *pos);
        pin.label = ["A", "B"][i].into();
        c.add_component(pin);
    }
    let mut tunnel = Component::new(Kind::Tunnel, ports[0]);
    tunnel.bits = 2;
    tunnel.label = "bus".into();
    c.add_component(tunnel.clone());
    splitter.position.x += 400;
    let ports = splitter.ports();
    c.add_component(splitter);
    tunnel.position = ports[0];
    c.add_component(tunnel);
    for (i, pos) in ports.iter().skip(1).enumerate() {
        let mut pin = Component::new(Kind::Pin, *pos);
        pin.output = true;
        pin.label = ["X", "Y"][i].into();
        c.add_component(pin);
    }
    let p = Project::single("bus", c);
    let a = analyze(&p, p.main).unwrap();
    for row in 0..4 {
        for i in 0..2 {
            assert_eq!(
                a.outputs()[i].values()[row],
                Cell::from(a.input_value(row, i).unwrap())
            );
        }
    }
}

#[test]
fn hierarchy_feedback_is_rejected_across_otherwise_pure_instance_boundaries() {
    let mut a = Analysis::default();
    a.add_input("A".into()).unwrap();
    a.add_output("Y".into()).unwrap();
    a.apply_expression(0, "~A").unwrap();
    let mut p = Project::single("inverter", a.generate().unwrap());
    let child = p.main;
    let root = wrap(&mut p, child, "feedback");
    let c = &mut p.circuit_mut(root).unwrap().circuit;
    for name in ["A", "Y"] {
        let pos = c
            .components
            .iter()
            .find(|c| c.kind == Kind::Pin && c.label == name)
            .unwrap()
            .position;
        let mut tunnel = Component::new(Kind::Tunnel, pos);
        tunnel.label = "loop".into();
        c.add_component(tunnel);
    }
    assert!(analyze(&p, root).unwrap_err().message.contains("feedback"));
}

#[test]
fn rom_document_contents_and_zero_input_constant_circuits_are_pure() {
    let mut c = Circuit::default();
    let mut rom = Component::new(Kind::Rom, Point::new(400, 300));
    rom.bits = 1;
    rom.memory.address_bits = 2;
    rom.memory.select_high = true;
    rom.memory.contents.set(2, 1);
    let ports = rom.ports();
    c.add_component(rom);
    for (i, bits, value) in [(1, 2, 2), (2, 1, 1)] {
        let mut k = Component::new(Kind::Constant, ports[i]);
        k.bits = bits;
        k.constant = value;
        c.add_component(k);
    }
    let mut output = Component::new(Kind::Pin, ports[0]);
    output.output = true;
    output.label = "Y".into();
    c.add_component(output);
    let p = Project::single("rom", c);
    let before = p.clone();
    let a = analyze(&p, p.main).unwrap();
    assert!(a.inputs().is_empty());
    assert_eq!(a.outputs()[0].values(), [Cell::One]);
    assert_eq!(p, before);
}

#[test]
fn interface_names_and_pin_count_never_silently_change_the_function() {
    let mut p = generated();
    for pin in &mut p.main_circuit_mut().circuit.components {
        if pin.kind == Kind::Pin && !pin.output {
            pin.label = "duplicate".into();
        }
    }
    assert!(
        analyze(&p, p.main)
            .unwrap_err()
            .message
            .contains("Duplicate")
    );
    let mut p = generated();
    p.main_circuit_mut()
        .circuit
        .components
        .iter_mut()
        .find(|p| p.kind == Kind::Pin)
        .unwrap()
        .label = "has space".into();
    assert!(analyze(&p, p.main).is_err());
    let mut p = generated();
    for i in 0..6 {
        let mut pin = Component::new(Kind::Pin, Point::new(2000, i * 50));
        pin.label = format!("Extra{i}");
        p.main_circuit_mut().circuit.add_component(pin);
    }
    assert!(
        analyze(&p, p.main)
            .unwrap_err()
            .message
            .contains("8 inputs")
    );
    let p = Project::default();
    assert!(
        analyze(&p, p.main)
            .unwrap_err()
            .message
            .contains("output Pin")
    );
}
