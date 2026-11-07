//! Attributes and terminal geometry observed through the reference UI.
use crate::{Component, Facing, Kind, Point};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TransistorType {
    #[default]
    P,
    N,
}
impl TransistorType {
    pub const ALL: [Self; 2] = [Self::P, Self::N];
    pub fn name(self) -> &'static str {
        match self {
            Self::P => "P-Type",
            Self::N => "N-Type",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransistorOptions {
    pub transistor_type: TransistorType,
    pub gate_bottom_right: bool,
}
impl Kind {
    pub fn is_transistor(self) -> bool {
        matches!(self, Self::Transistor | Self::TransmissionGate)
    }
}
impl Component {
    /// Local control offset; the attribute refers to world top/left, not handedness.
    pub fn transistor_gate_y(&self) -> i32 {
        if self.transistor.gate_bottom_right ^ matches!(self.facing, Facing::South | Facing::West) {
            20
        } else {
            -20
        }
    }
    /// Output, data input, transistor control / P control, optional N control.
    pub fn transistor_ports(&self) -> Vec<Point> {
        let y = self.transistor_gate_y();
        let mut ports = vec![Point::ZERO, Point::new(-40, 0), Point::new(-20, y)];
        if self.kind == Kind::TransmissionGate {
            ports.push(Point::new(-20, -y));
        }
        ports
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Circuit;
    #[test]
    fn transistor_terminals_follow_world_sides_and_attribute_edits_keep_wires() {
        for kind in [Kind::Transistor, Kind::TransmissionGate] {
            for facing in Facing::ALL {
                for right in [false, true] {
                    let mut c = Component::new(kind, Point::new(300, 300));
                    c.facing = facing;
                    c.bits = 32;
                    c.transistor.gate_bottom_right = right;
                    let offset = if right { 20 } else { -20 };
                    let middle = facing.transform(Point::new(-20, 0));
                    let gate = middle
                        + if matches!(facing, Facing::East | Facing::West) {
                            Point::new(0, offset)
                        } else {
                            Point::new(offset, 0)
                        };
                    let ports = c.ports();
                    assert_eq!(ports[0], c.position);
                    assert_eq!(ports[1], c.position + facing.transform(Point::new(-40, 0)));
                    assert_eq!(ports[2], c.position + gate);
                    if kind == Kind::TransmissionGate {
                        assert_eq!(ports[3], c.position + middle + middle - gate);
                    }
                    for i in 0..ports.len() {
                        assert_eq!(c.port_width(i), Some(if i < 2 { 32 } else { 1 }));
                        assert_eq!(c.drives_port(i), i == 0);
                    }
                    assert_eq!(c.port_width(ports.len()), None);
                    assert!(!kind.has_label());
                    let mut circuit = Circuit::default();
                    let id = circuit.add_component(c);
                    for p in ports {
                        circuit.add_wire(p, p + Point::new(0, 100));
                    }
                    circuit.normalize_wires();
                    let geometry = |c: &Circuit| {
                        c.wires
                            .iter()
                            .flat_map(|w| {
                                let start = w.a.y.min(w.b.y);
                                let end = w.a.y.max(w.b.y);
                                (start..end).map(move |y| (w.a.x, y))
                            })
                            .collect::<std::collections::BTreeSet<_>>()
                    };
                    let before = geometry(&circuit);
                    let mut edited = circuit.component(id).unwrap().clone();
                    edited.transistor.gate_bottom_right = !right;
                    edited.transistor.transistor_type = TransistorType::N;
                    circuit.replace_component(edited.clone());
                    assert_eq!(geometry(&circuit), before);
                    edited.facing = Facing::West;
                    edited.bits = 4;
                    circuit.replace_component(edited);
                    assert_eq!(geometry(&circuit), before);
                }
            }
        }
    }
}
