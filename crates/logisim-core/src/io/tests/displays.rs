use super::super::*;
use crate::Circuit;
#[test]
fn rgb_channels_keep_world_order_and_have_independent_fixed_widths() {
    for facing in Facing::ALL {
        let mut c = Component::new(Kind::RgbLed, Point::new(200, 200));
        c.facing = facing;
        for multi in [false, true] {
            c.io.rgb_multibit = multi;
            let expected: Vec<_> = (-1..=1)
                .map(|i| {
                    c.position
                        + if matches!(facing, Facing::East | Facing::West) {
                            Point::new(0, 10 * i)
                        } else {
                            Point::new(10 * i, 0)
                        }
                })
                .collect();
            assert_eq!(c.ports(), expected);
            for i in 0..3 {
                assert_eq!(c.port_width(i), Some(if multi { 8 } else { 1 }));
                assert!(!c.drives_port(i));
            }
            assert_eq!(c.port_width(3), None);
        }
    }
}
#[test]
fn matrix_dimensions_geometry_and_mode_edits_keep_existing_wire_paths() {
    for mode in MatrixInput::ALL {
        for (cols, rows) in [(1, 1), (3, 4), (5, 7), (32, 32)] {
            let mut c = Component::new(Kind::DotMatrix, Point::new(200, 400));
            c.io.matrix_columns = cols;
            c.io.matrix_rows = rows;
            c.io.matrix_input = mode;
            let ports = c.ports();
            assert_eq!(
                ports.len(),
                match mode {
                    MatrixInput::Columns => cols as usize,
                    MatrixInput::Rows => rows as usize,
                    MatrixInput::Select => 2,
                }
            );
            for (i, p) in ports.iter().enumerate() {
                let offset = if mode == MatrixInput::Columns {
                    Point::new(10 * i as i32, 0)
                } else {
                    Point::new(0, 10 * i as i32)
                };
                assert_eq!(*p, c.position + offset);
                assert_eq!(
                    c.port_width(i),
                    Some(match mode {
                        MatrixInput::Columns => rows,
                        MatrixInput::Rows => cols,
                        MatrixInput::Select =>
                            if i == 0 {
                                cols
                            } else {
                                rows
                            },
                    })
                );
                assert!(!c.drives_port(i));
            }
            assert_eq!(c.port_width(ports.len()), None);
            let [min, max] = c.io_body();
            assert_eq!(
                max - min,
                Point::new(i32::from(cols) * 10, i32::from(rows) * 10)
            );
            let mut circuit = Circuit::default();
            c.id = circuit.add_component(c.clone());
            for p in ports {
                circuit.add_wire(
                    p,
                    p + if mode == MatrixInput::Columns {
                        Point::new(0, 100)
                    } else {
                        Point::new(-100, 0)
                    },
                );
            }
            let before = circuit.wires.clone();
            c.io.matrix_input = MatrixInput::Select;
            c.io.matrix_columns = 2;
            c.io.matrix_rows = 2;
            circuit.replace_component(c);
            // Every resulting segment remains on the original geometric wire union.
            for wire in &circuit.wires {
                assert!(
                    before
                        .iter()
                        .any(|w| w.contains(wire.a) && w.contains(wire.b))
                );
            }
        }
    }
    assert!(!Kind::DotMatrix.rotatable());
    assert!(!Kind::DotMatrix.has_label());
}
