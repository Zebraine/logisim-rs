use super::*;
use crate::{Circuit, Facing, Gate, Kind, Point};
#[test]
fn inversion_extends_the_world_ordered_input_without_rerouting_wires() {
    for facing in Facing::ALL {
        let mut c = Component::new(Kind::Gate(Gate::And), Point::new(300, 300));
        c.facing = facing;
        c.gate_options.negated = 1;
        let mut circuit = Circuit::default();
        let port = c.ports()[1];
        let id = circuit.add_component(c);
        circuit.add_wire(port, port + facing.transform(Point::new(-40, 0)));
        let before = circuit.wires.clone();
        let mut c = circuit.component(id).unwrap().clone();
        c.gate_options.negated = 0;
        let plain = c.ports()[1];
        assert_eq!(port, plain + facing.transform(Point::new(-10, 0)));
        let points = c.ports();
        let axis = |p: Point| {
            if matches!(facing, Facing::East | Facing::West) {
                p.y
            } else {
                p.x
            }
        };
        assert!(axis(points[1]) < axis(points[2]));
        circuit.replace_component(c);
        assert_eq!(circuit.wires, before);
    }
}
