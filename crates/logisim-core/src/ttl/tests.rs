use super::*;
use crate::{Circuit, Facing};
#[test]
fn dip_pin_geometry_roles_and_attribute_edits_preserve_wires() {
    let physical = [
        (10, 30),
        (30, 30),
        (50, 30),
        (70, 30),
        (90, 30),
        (110, 30),
        (130, 30),
        (130, -30),
        (110, -30),
        (90, -30),
        (70, -30),
        (50, -30),
        (30, -30),
        (10, -30),
    ];
    for kind in Ttl::ALL {
        for facing in Facing::ALL {
            for power in [false, true] {
                let mut c = Component::new(Kind::Ttl(kind), Point::new(300, 300));
                c.facing = facing;
                c.ttl.power_pins = power;
                assert_eq!(c.ports().len(), if power { 14 } else { 12 });
                let outputs: &[u8] = match kind {
                    Ttl::Ic7402 => &[1, 4, 10, 13],
                    Ttl::Ic7404 => &[2, 4, 6, 8, 10, 12],
                    _ => &[3, 6, 8, 11],
                };
                for n in 1..=14 {
                    let index = c.ttl_index(n);
                    if !power && matches!(n, 7 | 14) {
                        assert!(index.is_none());
                        continue;
                    }
                    let i = index.unwrap();
                    let (x, y) = physical[usize::from(n - 1)];
                    assert_eq!(
                        c.ports()[i],
                        c.position + facing.transform(Point::new(x, y))
                    );
                    assert_eq!(c.drives_port(i), outputs.contains(&n));
                    assert_eq!(c.port_width(i), Some(1));
                }
                assert_eq!(c.port_width(c.ports().len()), None);
                assert!(!c.drives_port(c.ports().len()));
                let mut circuit = Circuit::default();
                let id = circuit.add_component(c.clone());
                for point in c.ports() {
                    circuit.add_wire(
                        point,
                        point
                            + facing.transform(Point::new(0, if point.y > 300 { 40 } else { -40 })),
                    );
                }
                let wires = circuit.wires.clone();
                c.id = id;
                c.facing = facing.next();
                c.ttl.power_pins = !power;
                c.ttl.internal = true;
                circuit.replace_component(c);
                assert_eq!(circuit.wires, wires);
            }
        }
    }
}
