use super::*;
use crate::{Circuit, Facing, Kind};
#[test]
fn parity_ports_follow_world_input_order_and_negation_for_all_sizes() {
    for kind in Parity::ALL {
        for facing in Facing::ALL {
            for size in [GateSize::Narrow, GateSize::Medium, GateSize::Wide] {
                for count in 2..=32 {
                    let mut c = Component::new(Kind::Parity(kind), Point::new(400, 400));
                    c.facing = facing;
                    c.parity.size = size;
                    c.parity.inputs = count;
                    c.bits = 32;
                    let plain = c.ports();
                    assert_eq!(plain.len(), usize::from(count) + 1);
                    let axis = |p: Point| {
                        if matches!(facing, Facing::East | Facing::West) {
                            p.y
                        } else {
                            p.x
                        }
                    };
                    assert!(plain[1..].windows(2).all(|p| axis(p[0]) < axis(p[1])));
                    c.parity.negated = 1 | (1 << (count - 1));
                    for (i, p) in c.ports().into_iter().enumerate() {
                        let delta = if i == 1 || i == usize::from(count) {
                            facing.transform(Point::new(-10, 0))
                        } else {
                            Point::ZERO
                        };
                        assert_eq!(p, plain[i] + delta);
                        assert_eq!(c.port_width(i), Some(32));
                        assert_eq!(c.drives_port(i), i == 0);
                    }
                    assert_eq!(c.port_width(plain.len()), None);
                }
            }
        }
    }
    let mut c = Component::new(Kind::Parity(Parity::Odd), Point::ZERO);
    c.parity.size = GateSize::Wide;
    c.parity.inputs = 4;
    assert_eq!(
        c.ports()[1..],
        [
            Point::new(-70, -30),
            Point::new(-70, -10),
            Point::new(-70, 10),
            Point::new(-70, 30)
        ]
    );
    c.parity.inputs = 32;
    assert_eq!(c.ports()[1], Point::new(-70, -160));
    assert_eq!(c.ports()[32], Point::new(-70, 160));
}
#[test]
fn parity_attribute_edits_keep_wire_geometry_and_hidden_inversions() {
    let mut circuit = Circuit::default();
    let mut c = Component::new(Kind::Parity(Parity::Even), Point::new(300, 300));
    c.parity.inputs = 32;
    c.parity.negated = 0x80000001;
    let ports = c.ports();
    let id = circuit.add_component(c.clone());
    for p in ports {
        circuit.add_wire(p, p + Point::new(-100, 0));
    }
    circuit.normalize_wires();
    let geometry = |c: &Circuit| {
        c.wires
            .iter()
            .flat_map(|w| (w.a.x.min(w.b.x)..w.a.x.max(w.b.x)).map(move |x| (x, w.a.y)))
            .collect::<std::collections::BTreeSet<_>>()
    };
    let before = geometry(&circuit);
    for (count, size, facing) in [
        (2, GateSize::Medium, Facing::East),
        (3, GateSize::Wide, Facing::South),
        (32, GateSize::Narrow, Facing::West),
    ] {
        c = circuit.component(id).unwrap().clone();
        c.parity.inputs = count;
        c.parity.size = size;
        c.facing = facing;
        circuit.replace_component(c);
        assert_eq!(geometry(&circuit), before);
        assert!(circuit.component(id).unwrap().parity_negated(31));
    }
}
