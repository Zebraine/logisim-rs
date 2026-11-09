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
mod tests;
