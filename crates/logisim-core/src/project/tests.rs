use super::*;
fn pins(groups: &[(Facing, usize)]) -> CircuitDefinition {
    let mut d = CircuitDefinition {
        id: 1,
        name: "part".into(),
        circuit: Circuit::default(),
    };
    for (side, (facing, count)) in groups.iter().enumerate() {
        for i in 0..*count {
            let mut c = Component::new(
                Kind::Pin,
                Point::new(side as i32 * 100 + i as i32 * 20, i as i32 * 30),
            );
            c.facing = *facing;
            c.output = *facing == Facing::West;
            d.circuit.add_component(c);
        }
    }
    d
}
#[test]
fn observed_default_interfaces_and_rotations() {
    let mixed = pins(&[(Facing::East, 3), (Facing::West, 2)]).interface();
    assert_eq!(
        mixed.ports.iter().map(|p| p.offset).collect::<Vec<_>>(),
        vec![
            Point::new(-30, 0),
            Point::new(-30, 10),
            Point::new(-30, 20),
            Point::ZERO,
            Point::new(0, 10)
        ]
    );
    assert_eq!(
        (mixed.min, mixed.max),
        (Point::new(-30, -5), Point::new(0, 25))
    );
    let northwest = pins(&[(Facing::East, 2), (Facing::South, 1)]).interface();
    assert_eq!(
        northwest.ports.iter().map(|p| p.offset).collect::<Vec<_>>(),
        vec![Point::new(-10, 10), Point::new(-10, 20), Point::ZERO]
    );
    let uneven = pins(&[(Facing::East, 5), (Facing::West, 1)]).interface();
    assert_eq!(uneven.ports[0].offset, Point::new(-30, -20));
    assert_eq!(uneven.ports[4].offset, Point::new(-30, 20));
    let mut c = Component::new(Kind::Subcircuit(1), Point::new(200, 200));
    c.interface = mixed;
    for facing in Facing::ALL {
        c.facing = facing;
        assert_eq!(
            c.ports()[1],
            c.position + facing.transform(Point::new(-30, 10))
        );
    }
    let empty = pins(&[]).interface();
    assert!(empty.ports.is_empty());
    assert_eq!(empty.min, Point::ZERO);
    assert_eq!(empty.max, Point::new(30, 30));
}
#[test]
fn references_survive_rename_and_disallow_recursive_or_used_deletion() {
    let mut p = Project::default();
    let a = p.add("A".into()).unwrap();
    let b = p.add("B".into()).unwrap();
    let c = p.instance(a, Point::ZERO).unwrap();
    p.main_circuit_mut().circuit.add_component(c);
    let c = p.instance(b, Point::ZERO).unwrap();
    p.circuit_mut(a).unwrap().circuit.add_component(c);
    assert!(!p.can_instantiate(b, 1));
    assert!(!p.can_instantiate(a, a));
    assert!(p.can_instantiate(1, b));
    assert!(p.remove(a).is_err());
    assert!(p.rename(b, "A".into()).is_err());
    p.rename(a, "Renamed".into()).unwrap();
    assert_eq!(
        p.main_circuit().circuit.components[0].interface.name,
        "Renamed"
    );
    p.validate().unwrap();
    p.main = b;
    p.remove(1).unwrap();
    assert_eq!(p.main, b);
}
#[test]
fn changing_pin_order_preserves_connections_by_pin_identity() {
    let mut p = Project::default();
    let sub = p.add("pair".into()).unwrap();
    p.circuit_mut(sub).unwrap().circuit = pins(&[(Facing::East, 2), (Facing::West, 1)]).circuit;
    let c = p.instance(sub, Point::new(200, 200)).unwrap();
    let pin = c.interface.ports[0].pin;
    let at = c.ports()[0];
    let id = p.main_circuit_mut().circuit.add_component(c);
    p.main_circuit_mut()
        .circuit
        .add_wire(Point::new(100, at.y), at);
    p.circuit_mut(sub).unwrap().circuit.components[0].position.y = 500;
    p.refresh_interfaces(true);
    let c = p.main_circuit().circuit.component(id).unwrap();
    let index = c.interface.ports.iter().position(|p| p.pin == pin).unwrap();
    let after = c.ports()[index];
    assert_ne!(at, after);
    assert!(
        p.main_circuit()
            .circuit
            .wires
            .iter()
            .any(|w| w.a == after || w.b == after)
    );
    assert_eq!(c.interface.ports[index].pin, pin);
}
