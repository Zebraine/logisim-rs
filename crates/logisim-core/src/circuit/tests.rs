use super::*;
use crate::{Facing, GRID, Gate};
#[test]
fn moving_a_wire_keeps_attached_branch_and_fixed_pin_connected() {
    let mut d = Circuit::default();
    let pin = d.add_component(Component::new(Kind::Pin, Point::ZERO));
    let moving = d.add_wire(Point::ZERO, Point::new(100, 0)).unwrap();
    d.add_wire(Point::new(50, 0), Point::new(50, 100));
    d.move_selection(&BTreeSet::from([moving]), Point::new(20, 30));
    assert_eq!(d.component(pin).unwrap().position, Point::ZERO);
    assert!(
        d.wires
            .iter()
            .any(|w| w.id == moving && w.a == Point::new(20, 30) && w.b == Point::new(120, 30))
    );
    assert_eq!(d.connected_wires(moving).len(), d.wires.len());
}
#[test]
fn moving_a_whole_selection_translates_each_wire_once() {
    let mut d = Circuit::default();
    d.add_component(Component::new(Kind::Pin, Point::new(50, 0)));
    d.add_wire(Point::ZERO, Point::new(100, 0));
    d.move_selection(&d.ids(), Point::new(20, 30));
    assert_eq!(d.wires.len(), 1);
    assert_eq!(
        (d.wires[0].a, d.wires[0].b),
        (Point::new(20, 30), Point::new(120, 30))
    );
    assert_eq!(d.components[0].position, Point::new(70, 30));
}
#[test]
fn gates_have_grid_aligned_ports_in_all_orientations() {
    for gate in Gate::ALL {
        for facing in Facing::ALL {
            for inputs in 2..=8 {
                let mut c = Component::new(Kind::Gate(gate), Point::new(120, 80));
                c.facing = facing;
                c.gate_options.inputs = inputs;
                let ports = c.ports();
                assert_eq!(ports.len(), usize::from(c.input_count()) + 1);
                assert_eq!(
                    ports.iter().copied().collect::<BTreeSet<_>>().len(),
                    ports.len()
                );
                assert!(ports.iter().all(|p| p.x % GRID == 0 && p.y % GRID == 0));
            }
        }
    }
}
#[test]
fn moving_connected_gate_keeps_terminal_and_branch() {
    let mut d = Circuit::default();
    let id = d.add_component(Component::new(Kind::Gate(Gate::And), Point::new(200, 100)));
    let port = d.component(id).unwrap().ports()[1];
    d.add_wire(Point::new(0, port.y), port);
    d.add_wire(Point::new(50, port.y), Point::new(50, 200));
    d.move_selection(&BTreeSet::from([id]), Point::new(40, 30));
    assert!(
        d.wires
            .iter()
            .any(|w| w.a == port + Point::new(40, 30) || w.b == port + Point::new(40, 30))
    );
    assert!(d.junctions().contains(&Point::new(50, port.y)));
    assert!(d.wires.iter().all(|w| w.a.x == w.b.x || w.a.y == w.b.y));
}
#[test]
fn moving_a_terminal_on_a_wire_interior_preserves_both_sides() {
    let mut d = Circuit::default();
    let id = d.add_component(Component::new(Kind::Pin, Point::new(50, 0)));
    d.add_wire(Point::ZERO, Point::new(100, 0));
    d.move_selection(&BTreeSet::from([id]), Point::new(0, 30));
    let seed = d.wires[0].id;
    assert_eq!(d.connected_wires(seed).len(), d.wires.len());
    assert!(d.wires.iter().any(|w| w.contains(Point::new(50, 30))));
}
#[test]
fn crossing_is_separate_until_an_endpoint_forms_a_junction() {
    let mut d = Circuit::default();
    let a = d.add_wire(Point::new(0, 50), Point::new(100, 50)).unwrap();
    let b = d.add_wire(Point::new(50, 0), Point::new(50, 100)).unwrap();
    assert!(!d.connected_wires(a).contains(&b));
    assert!(d.junctions().is_empty());
    d.add_wire(Point::new(50, 50), Point::new(80, 50));
    assert!(d.connected_wires(a).contains(&b));
    assert!(d.junctions().contains(&Point::new(50, 50)));
}
#[test]
fn same_named_tunnels_join_separate_wires_without_changing_geometry() {
    let mut d = Circuit::default();
    for x in [0, 200] {
        let mut t = Component::new(Kind::Tunnel, Point::new(x, 0));
        t.label = "DATA".into();
        d.add_component(t);
    }
    let a = d.add_wire(Point::ZERO, Point::new(0, 50)).unwrap();
    let b = d.add_wire(Point::new(200, 0), Point::new(200, 50)).unwrap();
    assert!(d.connected_wires(a).contains(&b));
    d.components[1].bits = 4;
    assert!(d.connected_wires(a).contains(&b));
    d.components[1].label = "data".into();
    assert!(!d.connected_wires(a).contains(&b));
}
#[test]
fn rotation_preserves_existing_port_connections() {
    let mut d = Circuit::default();
    let id = d.add_component(Component::new(Kind::Gate(Gate::Not), Point::new(100, 100)));
    let before = d.component(id).unwrap().ports()[1];
    d.add_wire(Point::new(0, 100), before);
    let mut changed = d.component(id).unwrap().clone();
    changed.facing = Facing::South;
    d.replace_component(changed);
    assert!(d.wires.iter().any(|w| w.contains(Point::new(100, 80))));
    assert!(d.wires.iter().all(|w| w.a.x == w.b.x || w.a.y == w.b.y));
}
