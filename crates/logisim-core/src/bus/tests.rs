use super::*;
use crate::{Circuit, Facing, GateSize};
#[test]
fn controlled_terminal_roles_widths_and_layout_edits_match_observation() {
    for kind in [Kind::ControlledBuffer, Kind::ControlledInverter] {
        for (size, depth) in [(GateSize::Narrow, 20), (GateSize::Wide, 30)] {
            let depth = if kind == Kind::ControlledBuffer {
                20
            } else {
                depth
            };
            for facing in Facing::ALL {
                for left in [false, true] {
                    let mut c = Component::new(kind, Point::new(200, 100));
                    c.gate_size = size;
                    c.bits = 32;
                    c.facing = facing;
                    c.bus.control_left = left;
                    let expected = [
                        Point::ZERO,
                        Point::new(-depth, 0),
                        Point::new(10 - depth, if left { -10 } else { 10 }),
                    ];
                    assert_eq!(
                        c.ports(),
                        expected.map(|p| c.position + facing.transform(p))
                    );
                    assert_eq!(
                        (0..3).map(|i| c.port_width(i)).collect::<Vec<_>>(),
                        [Some(32), Some(32), Some(1)]
                    );
                    assert!(c.drives_port(0) && !c.drives_port(1) && !c.drives_port(2));
                    let mut circuit = Circuit::default();
                    let id = circuit.add_component(c);
                    let old = circuit.component(id).unwrap().clone();
                    let port = old.ports()[2];
                    circuit.add_wire(port, port + Point::new(100, 0));
                    let wires = circuit.wires.clone();
                    let mut edited = old;
                    edited.bus.control_left = !left;
                    edited.gate_size = GateSize::Wide;
                    circuit.replace_component(edited);
                    assert_eq!(circuit.wires, wires);
                }
            }
        }
    }
    let c = Component::new(Kind::PullResistor, Point::ZERO);
    assert_eq!(c.facing, Facing::South);
    assert_eq!(c.ports(), [Point::ZERO]);
    assert_eq!(c.port_width(0), None);
    assert!(!c.drives_port(0));
}
