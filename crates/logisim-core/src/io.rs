//! Input/output geometry and persistent attributes. Switch states belong to each simulator instance.
use crate::{Component, Facing, Kind, Point};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MatrixInput {
    #[default]
    Columns,
    Rows,
    Select,
}
impl MatrixInput {
    pub const ALL: [Self; 3] = [Self::Columns, Self::Rows, Self::Select];
    pub fn name(self) -> &'static str {
        match self {
            Self::Columns => "Columns",
            Self::Rows => "Rows",
            Self::Select => "Select rows/columns",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Io {
    pub on_color: [u8; 4],
    pub off_color: [u8; 4],
    pub background: [u8; 4],
    pub active_high: bool,
    pub switch_count: u8,
    pub slider_reverse: bool,
    pub slider_value: u32,
    pub rgb_multibit: bool,
    pub matrix_columns: u8,
    pub matrix_rows: u8,
    pub matrix_input: MatrixInput,
    pub matrix_circular: bool,
    pub matrix_persistence: u32,
}
impl Default for Io {
    fn default() -> Self {
        Self {
            on_color: [240, 0, 0, 255],
            off_color: [220, 220, 220, 255],
            background: [255; 4],
            active_high: true,
            switch_count: 4,
            slider_reverse: false,
            slider_value: 0,
            rgb_multibit: false,
            matrix_columns: 5,
            matrix_rows: 7,
            matrix_input: MatrixInput::Columns,
            matrix_circular: false,
            matrix_persistence: 0,
        }
    }
}
impl Io {
    pub fn for_kind(kind: Kind) -> Self {
        let mut io = Self::default();
        if matches!(
            kind,
            Kind::Button | Kind::Switch | Kind::DipSwitch | Kind::Slider
        ) {
            io.on_color = [255; 4];
        }
        if kind == Kind::Joystick {
            io.on_color = [255, 0, 0, 255];
        }
        if kind == Kind::Tty {
            io.on_color = [0, 0, 0, 255];
            io.background = [0, 0, 0, 64];
        }
        if kind == Kind::DotMatrix {
            io.on_color = [0, 255, 0, 255];
        }
        if matches!(kind, Kind::Led | Kind::DotMatrix) {
            io.off_color = [64, 64, 64, 255];
        }
        io
    }
}
impl Kind {
    pub fn is_io(self) -> bool {
        matches!(
            self,
            Self::Button
                | Self::Switch
                | Self::DipSwitch
                | Self::Slider
                | Self::Joystick
                | Self::Keyboard
                | Self::Tty
                | Self::Led
                | Self::SevenSegment
                | Self::HexDisplay
                | Self::RgbLed
                | Self::DotMatrix
        )
    }
}
impl Component {
    pub fn slider_max(&self) -> u32 {
        u32::MAX >> (32 - self.bits.clamp(1, 32))
    }
    /// Integer track positions, rounded to the nearest representable bus value.
    pub fn slider_value_at(&self, percent: u8) -> u32 {
        let percent = percent.min(100);
        let percent = if self.io.slider_reverse {
            100 - percent
        } else {
            percent
        };
        ((u64::from(self.slider_max()) * u64::from(percent) + 50) / 100) as u32
    }
    pub fn io_body(&self) -> [Point; 2] {
        if self.kind == Kind::Joystick {
            let center = self.joystick_center() - self.position;
            return [center - Point::new(15, 15), center + Point::new(15, 15)]
                .map(|p| self.facing.inverse().transform(p));
        }
        // DIP ordering and slider orientation are anchored in world axes, not a rotated body.
        if matches!(self.kind, Kind::DipSwitch | Kind::Slider) {
            let n = i32::from(self.io.switch_count.clamp(1, 32)) * 20;
            let (a, b) = match (self.kind, self.facing) {
                (Kind::DipSwitch, Facing::East) => ((-30, 0), (0, n)),
                (Kind::DipSwitch, Facing::West) => ((0, 0), (30, n)),
                (Kind::DipSwitch, Facing::South) => ((0, -30), (n, 0)),
                (Kind::DipSwitch, Facing::North) => ((0, 0), (n, 30)),
                (_, Facing::East) => ((-120, -15), (0, 15)),
                (_, Facing::West) => ((0, -15), (120, 15)),
                (_, Facing::South) => ((-60, -30), (60, 0)),
                (_, Facing::North) => ((-60, 0), (60, 30)),
            };
            return [a, b].map(|(x, y)| self.facing.inverse().transform(Point::new(x, y)));
        }
        match self.kind {
            Kind::Keyboard => [Point::new(0, -15), Point::new(145, 10)],
            Kind::Tty => [
                Point::new(0, -15 * i32::from(self.terminal.rows.clamp(1, 48))),
                Point::new(7 * (i32::from(self.terminal.columns.clamp(1, 120)) + 3), 10),
            ],
            Kind::RgbLed => [Point::new(-30, -10), Point::new(0, 10)],
            Kind::DotMatrix => {
                let cols = i32::from(self.io.matrix_columns.clamp(1, 32));
                let rows = i32::from(self.io.matrix_rows.clamp(1, 32));
                let min = match self.io.matrix_input {
                    MatrixInput::Columns => Point::new(-5, -10 * rows),
                    MatrixInput::Rows => Point::new(0, -5),
                    MatrixInput::Select => Point::new(0, -10 * ((rows - 1) / 2)),
                };
                [min, min + Point::new(10 * cols, 10 * rows)]
            }
            Kind::Switch => [Point::new(-20, -15), Point::new(0, 15)],
            Kind::SevenSegment => [Point::new(-5, 0), Point::new(35, 60)],
            Kind::HexDisplay => [Point::new(-15, -60), Point::new(25, 0)],
            _ => [Point::new(-20, -10), Point::new(0, 10)],
        }
    }
    pub fn io_ports(&self) -> Vec<Point> {
        match self.kind {
            Kind::Keyboard => vec![
                Point::new(140, 10),
                Point::new(130, 10),
                Point::ZERO,
                Point::new(10, 10),
                Point::new(20, 10),
            ],
            Kind::Tty => vec![
                Point::new(0, -10),
                Point::ZERO,
                Point::new(10, 10),
                Point::new(20, 10),
            ],
            Kind::Joystick => vec![
                Point::ZERO,
                self.facing.inverse().transform(
                    if matches!(self.facing, Facing::East | Facing::West) {
                        Point::new(0, 10)
                    } else {
                        Point::new(-10, 0)
                    },
                ),
            ],
            Kind::RgbLed => (-1..=1)
                .map(|i| {
                    self.facing.inverse().transform(
                        if matches!(self.facing, Facing::East | Facing::West) {
                            Point::new(0, i * 10)
                        } else {
                            Point::new(i * 10, 0)
                        },
                    )
                })
                .collect(),
            Kind::DotMatrix => match self.io.matrix_input {
                MatrixInput::Columns => (0..self.io.matrix_columns.clamp(1, 32))
                    .map(|i| Point::new(i32::from(i) * 10, 0))
                    .collect(),
                MatrixInput::Rows => (0..self.io.matrix_rows.clamp(1, 32))
                    .map(|i| Point::new(0, i32::from(i) * 10))
                    .collect(),
                MatrixInput::Select => vec![Point::ZERO, Point::new(0, 10)],
            },
            Kind::SevenSegment => (0..8)
                .map(|i| Point::new((i % 4) * 10, (i / 4) * 60))
                .collect(),
            Kind::HexDisplay => vec![Point::ZERO, Point::new(10, 0)],
            Kind::Button | Kind::Led | Kind::Slider => vec![Point::ZERO],
            Kind::Switch => vec![Point::ZERO, Point::new(-20, 0)],
            Kind::DipSwitch => (0..self.io.switch_count.clamp(1, 32))
                .map(|i| {
                    let offset = 10 + 20 * i32::from(i);
                    self.facing.inverse().transform(
                        if matches!(self.facing, Facing::East | Facing::West) {
                            Point::new(0, offset)
                        } else {
                            Point::new(offset, 0)
                        },
                    )
                })
                .collect(),
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Circuit, Facing};
    #[test]
    fn measured_ports_widths_and_rotations_preserve_connections() {
        let seven = Component::new(Kind::SevenSegment, Point::new(200, 100));
        assert_eq!(
            seven.ports(),
            vec![
                Point::new(200, 100),
                Point::new(210, 100),
                Point::new(220, 100),
                Point::new(230, 100),
                Point::new(200, 160),
                Point::new(210, 160),
                Point::new(220, 160),
                Point::new(230, 160)
            ]
        );
        for i in 0..8 {
            assert_eq!(seven.port_width(i), Some(1));
            assert!(!seven.drives_port(i));
        }
        let hex = Component::new(Kind::HexDisplay, Point::ZERO);
        assert_eq!(hex.ports(), vec![Point::ZERO, Point::new(10, 0)]);
        assert_eq!((hex.port_width(0), hex.port_width(1)), (Some(4), Some(1)));
        assert!(!hex.kind.rotatable());
        assert!(!seven.kind.rotatable());
        assert_eq!(Component::new(Kind::Led, Point::ZERO).facing, Facing::West);
        for kind in [Kind::Button, Kind::Led] {
            for facing in Facing::ALL {
                let mut c = Component::new(kind, Point::new(100, 100));
                c.facing = facing;
                assert_eq!(c.ports(), vec![Point::new(100, 100)]);
                assert_eq!(c.port_width(0), Some(1));
                assert_eq!(c.drives_port(0), kind == Kind::Button);
                let mut circuit = Circuit::default();
                let id = circuit.add_component(c);
                circuit.add_wire(Point::new(100, 100), Point::new(200, 100));
                let mut next = circuit.component(id).unwrap().clone();
                next.facing = facing.next();
                circuit.replace_component(next);
                assert_eq!(circuit.wires[0].a, Point::new(100, 100));
                let mut moved = circuit.component(id).unwrap().clone();
                moved.position = Point::new(100, 150);
                circuit.replace_component(moved);
                assert!(
                    circuit
                        .wires
                        .iter()
                        .any(|w| w.a == Point::new(100, 150) || w.b == Point::new(100, 150))
                );
            }
        }
    }
}

#[cfg(test)]
mod input_tests {
    use super::*;
    use crate::Circuit;
    #[test]
    fn input_geometry_preserves_observed_order_and_existing_wires() {
        for facing in Facing::ALL {
            let mut switch = Component::new(Kind::Switch, Point::new(200, 200));
            switch.facing = facing;
            switch.bits = 32;
            assert_eq!(
                switch.ports(),
                vec![
                    switch.position,
                    switch.position + facing.transform(Point::new(-20, 0))
                ]
            );
            assert_eq!(switch.port_width(1), Some(32));
            assert!(switch.drives_port(0) && !switch.drives_port(1));
            for count in [1, 4, 32] {
                let mut dip = Component::new(Kind::DipSwitch, Point::ZERO);
                dip.facing = facing;
                dip.io.switch_count = count;
                let ports: Vec<_> = (0..count)
                    .map(|i| {
                        if matches!(facing, Facing::East | Facing::West) {
                            Point::new(0, 10 + 20 * i32::from(i))
                        } else {
                            Point::new(10 + 20 * i32::from(i), 0)
                        }
                    })
                    .collect();
                assert_eq!(dip.ports(), ports);
                for i in 0..usize::from(count) {
                    assert!(dip.drives_port(i));
                    assert_eq!(dip.port_width(i), Some(1));
                }
                assert!(!dip.drives_port(usize::from(count)));
                let mut circuit = Circuit::default();
                let id = circuit.add_component(dip.clone());
                for p in ports {
                    circuit.add_wire(p, p + Point::new(100, 0));
                }
                let wires = circuit.wires.clone();
                dip.id = id;
                dip.io.switch_count = 2;
                circuit.replace_component(dip);
                assert_eq!(circuit.wires, wires);
            }
            let mut slider = Component::new(Kind::Slider, Point::ZERO);
            slider.facing = facing;
            assert_eq!(slider.ports(), vec![Point::ZERO]);
            assert_eq!(slider.port_width(0), Some(8));
            let world = slider.io_body().map(|p| facing.transform(p));
            let expected = match facing {
                Facing::East => [Point::new(-120, -15), Point::new(0, 15)],
                Facing::West => [Point::new(0, -15), Point::new(120, 15)],
                Facing::South => [Point::new(-60, -30), Point::new(60, 0)],
                Facing::North => [Point::new(-60, 0), Point::new(60, 30)],
            };
            assert_eq!(world, expected);
        }
    }
    #[test]
    fn slider_full_width_rounding_and_reverse_direction() {
        let mut c = Component::new(Kind::Slider, Point::ZERO);
        for (bits, middle, almost, max) in [
            (1, 1, 1, 1),
            (4, 8, 15, 15),
            (8, 128, 252, 255),
            (32, 0x80000000, 0xfd70a3d6, u32::MAX),
        ] {
            c.bits = bits;
            assert_eq!(c.slider_value_at(0), 0);
            assert_eq!(c.slider_value_at(50), middle);
            assert_eq!(c.slider_value_at(99), almost);
            assert_eq!(c.slider_value_at(100), max);
            c.io.slider_reverse = true;
            assert_eq!(c.slider_value_at(0), max);
            assert_eq!(c.slider_value_at(1), almost);
            assert_eq!(c.slider_value_at(100), 0);
            c.io.slider_reverse = false;
        }
    }
}

#[cfg(test)]
mod display_tests {
    use super::*;
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
}
