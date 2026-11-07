use super::*;
use crate::Bit;
use logisim_core::{Component, Facing, Gate, Point};
fn inverter(p: &mut Project, name: &str, bits: u8) -> (Id, Id, Id) {
    let id = p.add(name.into()).unwrap();
    let c = &mut p.circuit_mut(id).unwrap().circuit;
    let mut input = Component::new(Kind::Pin, Point::new(100, 100));
    input.bits = bits;
    let input = c.add_component(input);
    let mut gate = Component::new(Kind::Gate(Gate::Not), Point::new(180, 100));
    gate.bits = bits;
    c.add_component(gate);
    let mut output = Component::new(Kind::Pin, Point::new(240, 100));
    output.bits = bits;
    output.output = true;
    output.facing = Facing::West;
    let output = c.add_component(output);
    c.add_wire(Point::new(100, 100), Point::new(160, 100));
    c.add_wire(Point::new(180, 100), Point::new(240, 100));
    (id, input, output)
}
#[test]
fn nested_instances_are_isolated_and_four_state_signals_cross_boundaries() {
    let mut p = Project::default();
    let (inv, child_input, child_output) = inverter(&mut p, "inv", 4);
    let wrapper = p.add("wrapper".into()).unwrap();
    let inner = p.instance(inv, Point::new(200, 100)).unwrap();
    let inner_id = p.circuit_mut(wrapper).unwrap().circuit.add_component(inner);
    let c = &mut p.circuit_mut(wrapper).unwrap().circuit;
    let mut input = Component::new(Kind::Pin, Point::new(100, 100));
    input.bits = 4;
    c.add_component(input);
    let mut output = Component::new(Kind::Pin, Point::new(250, 100));
    output.bits = 4;
    output.output = true;
    output.facing = Facing::West;
    c.add_component(output);
    c.add_wire(Point::new(100, 100), Point::new(170, 100));
    c.add_wire(Point::new(200, 100), Point::new(250, 100));
    let mut ids = vec![];
    for (i, value) in [10, 15].into_iter().enumerate() {
        let y = 100 + i as i32 * 100;
        let instance = p.instance(wrapper, Point::new(200, y)).unwrap();
        let c = &mut p.main_circuit_mut().circuit;
        let instance = c.add_component(instance);
        let mut input = Component::new(Kind::Pin, Point::new(100, y));
        input.bits = 4;
        let input = c.add_component(input);
        c.add_wire(Point::new(100, y), Point::new(170, y));
        ids.push((instance, input, value));
    }
    p.refresh_interfaces(false);
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&p, p.main);
    for (_, input, value) in &ids {
        sim.set_input(*input, *value);
    }
    sim.advance(64);
    for (instance, _, value) in &ids {
        let leaf = sim.view(&[*instance, inner_id]).unwrap();
        assert_eq!(
            leaf.port_value(child_input, 0).unwrap().number(),
            Some(*value)
        );
        assert_eq!(
            leaf.port_value(child_output, 0).unwrap().number(),
            Some(15 ^ value)
        );
    }
    // A disconnected parent input must replace the nested Pin's standalone zero drive.
    p.main_circuit_mut().circuit.wires.clear();
    sim.synchronize(&p, p.main);
    sim.advance(64);
    let leaf = sim.view(&[ids[0].0, inner_id]).unwrap();
    assert_eq!(
        leaf.port_value(child_input, 0).unwrap().bit(0),
        Bit::Unknown
    );
    assert_eq!(leaf.port_value(child_output, 0).unwrap().bit(0), Bit::Error);
}
#[test]
fn tunnels_and_multiple_output_ports_are_local_to_each_instance() {
    let mut p = Project::default();
    let sub = p.add("pass".into()).unwrap();
    let c = &mut p.circuit_mut(sub).unwrap().circuit;
    let mut input = Component::new(Kind::Pin, Point::new(100, 100));
    input.bits = 4;
    c.add_component(input);
    for x in [100, 200] {
        let mut t = Component::new(Kind::Tunnel, Point::new(x, 100));
        t.bits = 4;
        t.label = "local".into();
        c.add_component(t);
    }
    for (y, value) in [(100, None), (200, Some(3))] {
        let mut pin = Component::new(Kind::Pin, Point::new(200, y));
        pin.bits = 4;
        pin.output = true;
        pin.facing = Facing::West;
        c.add_component(pin);
        if let Some(value) = value {
            let mut source = Component::new(Kind::Constant, Point::new(200, y));
            source.bits = 4;
            source.constant = value;
            c.add_component(source);
        }
    }
    let mut outputs = vec![];
    for (i, value) in [10, 12].into_iter().enumerate() {
        let y = 100 + i as i32 * 100;
        let instance = p.instance(sub, Point::new(200, y)).unwrap();
        let ports = instance.interface.ports.clone();
        let c = &mut p.main_circuit_mut().circuit;
        let id = c.add_component(instance);
        let mut source = Component::new(Kind::Constant, Point::new(170, y));
        source.bits = 4;
        source.constant = value;
        c.add_component(source);
        outputs.push((id, value, ports));
    }
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&p, 1);
    sim.advance(64);
    for (id, value, ports) in outputs {
        let outputs: Vec<_> = ports
            .iter()
            .enumerate()
            .filter(|(_, p)| p.output)
            .map(|(i, _)| sim.view(&[]).unwrap().port_value(id, i).unwrap().number())
            .collect();
        assert_eq!(outputs, vec![Some(value), Some(3)]);
    }
    sim.set_enabled(false);
    sim.reset();
    assert!(!sim.enabled());
    while sim.step() {}
    assert_eq!(sim.pending_count(), 0);
}
#[test]
fn deep_or_recursive_projects_fail_without_partial_simulation() {
    let mut p = Project::default();
    let mut parent = 1;
    for i in 0..34 {
        let next = p.add(format!("c{i}")).unwrap();
        let c = p.instance(next, Point::ZERO).unwrap();
        p.circuit_mut(parent).unwrap().circuit.add_component(c);
        parent = next;
    }
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&p, 1);
    assert!(sim.error().unwrap().contains("32"));
    assert!(sim.view(&[]).is_none());
    let c = p.instance(1, Point::ZERO).unwrap();
    p.circuit_mut(parent).unwrap().circuit.add_component(c);
    sim.synchronize(&p, 1);
    assert!(sim.error().unwrap().contains("recursive"));
}
#[test]
fn standalone_navigation_retains_inputs_and_reset_clears_them() {
    let mut p = Project::default();
    let (child, _, _) = inverter(&mut p, "child", 1);
    let pin = p
        .main_circuit_mut()
        .circuit
        .add_component(Component::new(Kind::Pin, Point::ZERO));
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&p, 1);
    sim.set_input(pin, 1);
    sim.advance(64);
    sim.synchronize(&p, child);
    sim.advance(64);
    sim.synchronize(&p, 1);
    sim.advance(64);
    assert_eq!(sim.input(pin).unwrap().number(), Some(1));
    sim.synchronize(&p, child);
    sim.reset();
    sim.synchronize(&p, 1);
    sim.advance(64);
    assert_eq!(sim.input(pin).unwrap().number(), Some(0));
}
#[test]
fn custom_package_rotation_works_and_visual_edits_preserve_paused_signals() {
    let mut p = Project::default();
    let (child, input, output) = inverter(&mut p, "pack", 4);
    let mut art = p.circuit(child).unwrap().editable_appearance();
    art.facing = Facing::North;
    p.circuit_mut(child).unwrap().circuit.appearance = Some(art);
    let mut instance = p.instance(child, Point::new(200, 200)).unwrap();
    instance.facing = Facing::West;
    let input_index = instance
        .interface
        .ports
        .iter()
        .position(|port| port.pin == input)
        .unwrap();
    let output_index = instance
        .interface
        .ports
        .iter()
        .position(|port| port.pin == output)
        .unwrap();
    let mut source = Component::new(Kind::Constant, instance.ports()[input_index]);
    source.bits = 4;
    source.constant = 10;
    let id = p.main_circuit_mut().circuit.add_component(instance);
    p.main_circuit_mut().circuit.add_component(source);
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&p, p.main);
    sim.advance(64);
    assert_eq!(
        sim.view(&[])
            .unwrap()
            .port_value(id, output_index)
            .unwrap()
            .number(),
        Some(5)
    );
    sim.set_enabled(false);
    let steps = sim.steps();
    let value = sim.view(&[id]).unwrap().port_value(output, 0);
    p.circuit_mut(child)
        .unwrap()
        .circuit
        .appearance
        .as_mut()
        .unwrap()
        .shapes[0]
        .paint
        .fill = Some([100, 150, 200]);
    p.refresh_interfaces(true);
    assert!(!sim.synchronize(&p, p.main));
    assert!(!sim.enabled());
    assert_eq!(sim.steps(), steps);
    assert_eq!(sim.view(&[id]).unwrap().port_value(output, 0), value);
}
