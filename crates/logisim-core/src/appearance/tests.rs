use super::*;
use crate::{Component, Project};
fn project() -> (Project, Id, Id, Id) {
    let mut p = Project::default();
    let child = p.add("part".into()).unwrap();
    let c = &mut p.circuit_mut(child).unwrap().circuit;
    let input = c.add_component(Component::new(Kind::Pin, Point::new(100, 100)));
    let mut pin = Component::new(Kind::Pin, Point::new(200, 100));
    pin.output = true;
    pin.facing = Facing::West;
    let output = c.add_component(pin);
    (p, child, input, output)
}
#[test]
fn custom_anchor_basis_rotates_graphics_and_ports_and_sets_new_instance_facing() {
    let (mut p, child, input, output) = project();
    let mut a = p.circuit(child).unwrap().editable_appearance();
    a.anchor = Point::new(100, 70);
    a.facing = Facing::North;
    a.ports.insert(input, Point::new(50, 60));
    a.ports.insert(output, Point::new(80, 60));
    p.circuit_mut(child).unwrap().circuit.appearance = Some(a);
    p.refresh_interfaces(false);
    let mut instance = p.instance(child, Point::new(200, 100)).unwrap();
    assert_eq!(instance.facing, Facing::North);
    assert_eq!(
        instance.ports(),
        vec![Point::new(150, 90), Point::new(180, 90)]
    );
    instance.facing = Facing::East;
    assert_eq!(
        instance.ports(),
        vec![Point::new(210, 50), Point::new(210, 80)]
    );
    assert_eq!(instance.interface.ports[0].pin, input);
    p.validate().unwrap();
}
#[test]
fn pin_identity_survives_internal_moves_new_pins_append_and_removed_pins_disappear() {
    let (mut p, child, input, _) = project();
    let mut a = p.circuit(child).unwrap().editable_appearance();
    a.ports.insert(input, Point::new(40, 90));
    p.circuit_mut(child).unwrap().circuit.appearance = Some(a.clone());
    let c = &mut p.circuit_mut(child).unwrap().circuit;
    let mut moved = c.component(input).unwrap().clone();
    moved.position = Point::new(300, 200);
    c.replace_component(moved);
    let new = c.add_component(Component::new(Kind::Pin, Point::new(100, 200)));
    p.refresh_interfaces(true);
    let appearance = p
        .circuit(child)
        .unwrap()
        .circuit
        .appearance
        .as_ref()
        .unwrap();
    assert_eq!(appearance.ports[&input], Point::new(40, 90));
    assert_eq!(appearance.ports[&new], Point::new(40, 100));
    assert_eq!(appearance.shapes, a.shapes);
    p.circuit_mut(child)
        .unwrap()
        .circuit
        .remove(&BTreeSet::from([input]));
    p.refresh_interfaces(true);
    assert!(
        !p.circuit(child)
            .unwrap()
            .circuit
            .appearance
            .as_ref()
            .unwrap()
            .ports
            .contains_key(&input)
    );
    p.validate().unwrap();
}
#[test]
fn custom_interface_edits_keep_existing_wire_coordinates_as_observed() {
    let (mut p, child, input, _) = project();
    let a = p.circuit(child).unwrap().editable_appearance();
    p.circuit_mut(child).unwrap().circuit.appearance = Some(a);
    let instance = p.instance(child, Point::new(300, 200)).unwrap();
    let old = instance.ports()[0];
    let id = p.main_circuit_mut().circuit.add_component(instance);
    p.main_circuit_mut()
        .circuit
        .add_wire(old - Point::new(100, 0), old);
    p.circuit_mut(child)
        .unwrap()
        .circuit
        .appearance
        .as_mut()
        .unwrap()
        .ports
        .insert(input, Point::new(20, 140));
    p.refresh_interfaces(true);
    let new = p.main_circuit().circuit.component(id).unwrap().ports()[0];
    assert_ne!(old, new);
    let wires = &p.main_circuit().circuit.wires;
    assert_eq!(wires.len(), 1);
    assert_eq!((wires[0].a, wires[0].b), (old - Point::new(100, 0), old));
    assert!(!wires[0].contains(new));
}
