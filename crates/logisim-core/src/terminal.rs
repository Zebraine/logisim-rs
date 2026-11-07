//! Character terminal settings only; text buffers and cursor belong to simulator instances.
use crate::{Kind, Trigger};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalOptions {
    pub buffer_len: u16,
    pub rows: u8,
    pub columns: u8,
    pub trigger: Trigger,
}
impl Default for TerminalOptions {
    fn default() -> Self {
        Self {
            buffer_len: 32,
            rows: 8,
            columns: 32,
            trigger: Trigger::Rising,
        }
    }
}
impl Kind {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Keyboard | Self::Tty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Circuit, Component, Point};
    #[test]
    fn fixed_terminal_geometry_widths_and_dimension_edits_preserve_wires() {
        for kind in [Kind::Keyboard, Kind::Tty] {
            let mut c = Component::new(kind, Point::new(200, 400));
            assert_eq!(c.bits, 7);
            assert!(!kind.rotatable() && !kind.has_label());
            let offsets = if kind == Kind::Keyboard {
                vec![(140, 10), (130, 10), (0, 0), (10, 10), (20, 10)]
            } else {
                vec![(0, -10), (0, 0), (10, 10), (20, 10)]
            };
            for bits in [7, 16] {
                c.bits = bits;
                assert_eq!(
                    c.ports(),
                    offsets
                        .iter()
                        .map(|(x, y)| c.position + Point::new(*x, *y))
                        .collect::<Vec<_>>()
                );
                for i in 0..offsets.len() {
                    assert_eq!(c.port_width(i), Some(if i == 0 { bits } else { 1 }));
                    assert_eq!(c.drives_port(i), kind == Kind::Keyboard && i < 2);
                }
                assert_eq!(c.port_width(offsets.len()), None);
            }
            let mut circuit = Circuit::default();
            c.id = circuit.add_component(c.clone());
            for p in c.ports() {
                circuit.add_wire(p, p + Point::new(0, 100));
            }
            let wires = circuit.wires.clone();
            c.terminal.rows = 48;
            c.terminal.columns = 120;
            c.terminal.buffer_len = 256;
            assert_eq!(
                c.io_body(),
                if kind == Kind::Keyboard {
                    [Point::new(0, -15), Point::new(145, 10)]
                } else {
                    [Point::new(0, -720), Point::new(861, 10)]
                }
            );
            circuit.replace_component(c);
            assert_eq!(circuit.wires, wires);
        }
    }
}
