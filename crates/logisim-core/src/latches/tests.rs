use super::*;
use crate::Circuit;

#[test]
fn latch_geometry_matches_both_observed_layouts_and_all_control_positions() {
    use LatchRole::*;
    for kind in Latch::ALL {
        for new in [true, false] {
            for mode in LatchEnable::ALL {
                if kind == Latch::D && mode == LatchEnable::None {
                    continue;
                }
                for controls in ControlPosition::ALL {
                    let mut c = Component::new(Kind::Latch(kind), Point::new(200, 200));
                    c.sequential.new_layout = new;
                    c.sequential.controls = controls;
                    c.latch_enable = mode;
                    let terminals = c.latch_terminals();
                    let get = |role| terminals.iter().find(|(r, _)| *r == role).map(|(_, p)| *p);
                    assert_eq!(get(Q), Some(Point::ZERO));
                    assert_eq!(get(NotQ), Some(Point::new(0, if new { 40 } else { 20 })));
                    assert_eq!(
                        get(Enable),
                        if mode == LatchEnable::None {
                            None
                        } else {
                            Some(Point::new(
                                if new { -60 } else { -40 },
                                if !new && kind == Latch::Sr { 10 } else { 0 },
                            ))
                        }
                    );
                    assert_eq!(
                        get(if kind == Latch::D { D } else { R }),
                        Some(Point::new(
                            if new { -60 } else { -40 },
                            if new { 40 } else { 20 }
                        ))
                    );
                    if kind == Latch::Sr {
                        assert_eq!(
                            get(S),
                            Some(Point::new(
                                if new { -60 } else { -40 },
                                if new { 20 } else { 0 }
                            ))
                        );
                    }
                    let expected = match (new, controls) {
                        (true, ControlPosition::AboveBelow) => [(-30, -20), (-30, 60)],
                        (true, ControlPosition::BelowAbove) => [(-30, 60), (-30, -20)],
                        (true, ControlPosition::Legacy) => [(-40, 60), (-20, 60)],
                        (false, ControlPosition::AboveBelow) => [(-20, -10), (-20, 30)],
                        (false, ControlPosition::BelowAbove) => [(-20, 30), (-20, -10)],
                        (false, ControlPosition::Legacy) => [(-30, 30), (-10, 30)],
                    };
                    for (role, (x, y)) in [Preset, Clear].into_iter().zip(expected) {
                        assert_eq!(get(role), Some(Point::new(x, y)));
                    }
                    for (i, (_, local)) in terminals.iter().enumerate() {
                        assert_eq!(c.ports()[i], c.position + *local);
                        assert_eq!(c.port_width(i), Some(1));
                        assert_eq!(c.drives_port(i), i < 2);
                    }
                    assert_eq!(c.port_width(terminals.len()), None);
                    c.sequential.negate_controls = true;
                    assert_eq!(c.latch_terminals(), terminals);
                    assert!(!c.kind.rotatable());
                }
            }
        }
    }
}

#[test]
fn latch_attribute_edits_keep_wire_geometry_when_ports_move_or_disappear() {
    let mut circuit = Circuit::default();
    let c = Component::new(Kind::Latch(Latch::Sr), Point::new(200, 200));
    let id = circuit.add_component(c.clone());
    circuit.add_wire(c.ports()[2] - Point::new(50, 0), c.ports()[2]);
    circuit.add_wire(c.ports()[3] - Point::new(50, 0), c.ports()[3]);
    let before = circuit.wires.clone();
    for mode in [LatchEnable::None, LatchEnable::Low, LatchEnable::High] {
        for new in [false, true] {
            for controls in ControlPosition::ALL {
                let mut c = circuit.component(id).unwrap().clone();
                c.latch_enable = mode;
                c.sequential.new_layout = new;
                c.sequential.controls = controls;
                circuit.replace_component(c);
                assert_eq!(circuit.wires, before);
            }
        }
    }
}
