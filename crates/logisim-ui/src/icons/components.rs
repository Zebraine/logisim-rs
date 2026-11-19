//! Monochrome catalogue symbols on a 16-unit grid. The row owns all interaction.
use super::{Icon, paint};
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2, vec2};
use logisim_core::{Arithmetic, FlipFlop, Gate, Kind, Latch, Parity, Plexer, plugin::api::Tool};

pub(crate) enum ComponentIcon<'a> {
    Builtin(Kind),
    Plugin(&'a Tool),
}

impl ComponentIcon<'_> {
    pub(crate) fn paint(self, painter: &Painter, rect: Rect, color: Color32) {
        let pen = Pen {
            painter,
            rect,
            stroke: Stroke::new(1.4, color),
        };
        match self {
            Self::Builtin(kind) => pen.component(kind),
            Self::Plugin(tool) => pen.package(tool),
        }
    }
}

type Point = (u8, u8);
struct Pen<'a> {
    painter: &'a Painter,
    rect: Rect,
    stroke: Stroke,
}

impl Pen<'_> {
    fn at(&self, (x, y): Point) -> Pos2 {
        self.rect.min + vec2(f32::from(x), f32::from(y)) * (self.rect.width() / 16.)
    }
    fn bounds(&self, a: Point, b: Point) -> Rect {
        Rect::from_two_pos(self.at(a), self.at(b))
    }
    fn line(&self, a: Point, b: Point) {
        self.painter
            .line_segment([self.at(a), self.at(b)], self.stroke);
    }
    fn path<const N: usize>(&self, points: [Point; N]) {
        self.painter.add(Shape::line(
            points.map(|p| self.at(p)).to_vec(),
            self.stroke,
        ));
    }
    fn frame(&self, a: Point, b: Point) {
        self.painter
            .rect_stroke(self.bounds(a, b), 1, self.stroke, StrokeKind::Middle);
    }
    fn ring(&self, center: Point, radius: f32) {
        self.painter.circle_stroke(
            self.at(center),
            radius * self.rect.width() / 16.,
            self.stroke,
        );
    }
    fn dot(&self, center: Point) {
        self.painter
            .circle_filled(self.at(center), 1.25, self.stroke.color);
    }
    fn symbol(&self, icon: Icon, a: Point, b: Point) {
        paint(self.painter, self.bounds(a, b), icon, self.stroke.color);
    }

    fn component(&self, kind: Kind) {
        match kind {
            Kind::Gate(gate) => self.gate(gate, 2),
            Kind::Parity(parity) => {
                self.gate(
                    match parity {
                        Parity::Odd => Gate::Xor,
                        Parity::Even => Gate::Xnor,
                    },
                    5,
                );
                for y in [4, 8, 12] {
                    self.line((0, y), (3, y));
                }
            }
            Kind::ControlledBuffer | Kind::ControlledInverter => {
                self.gate(
                    if kind == Kind::ControlledBuffer {
                        Gate::Buffer
                    } else {
                        Gate::Not
                    },
                    2,
                );
                self.line((7, 11), (7, 16));
            }
            Kind::Plexer(plexer) => self.plexer(plexer),
            Kind::Arithmetic(op) => self.arithmetic(op),
            Kind::Pin => self.symbol(Icon::Pin, (1, 1), (15, 15)),
            Kind::Tunnel => self.symbol(Icon::Tunnel, (1, 2), (15, 14)),
            Kind::Text => self.symbol(Icon::Text, (2, 2), (14, 14)),
            Kind::Subcircuit(_) | Kind::Plugin => self.symbol(Icon::Chip, (0, 0), (16, 16)),
            Kind::Splitter => {
                self.line((1, 8), (15, 8));
                self.path([(5, 8), (10, 3), (15, 3)]);
                self.path([(5, 8), (10, 13), (15, 13)]);
            }
            Kind::Probe => {
                self.frame((4, 3), (15, 13));
                self.line((0, 8), (4, 8));
                self.path([(6, 9), (9, 9), (9, 6), (13, 6)]);
            }
            Kind::PullResistor => {
                self.path([(8, 15), (8, 12), (5, 10), (11, 7), (5, 4), (8, 3), (8, 1)]);
            }
            Kind::Clock => {
                self.path([(1, 12), (4, 12), (4, 4), (10, 4), (10, 12), (15, 12)]);
            }
            Kind::Constant => {
                self.frame((1, 3), (12, 13));
                self.line((4, 8), (9, 8));
                self.line((12, 8), (16, 8));
            }
            Kind::Power => {
                self.path([(3, 6), (8, 1), (13, 6)]);
                self.line((8, 1), (8, 15));
            }
            Kind::Ground => {
                self.line((8, 1), (8, 8));
                for (left, y) in [(2, 8), (4, 11), (6, 14)] {
                    self.line((left, y), (16 - left, y));
                }
            }
            Kind::Transistor => {
                self.line((1, 8), (5, 8));
                self.line((5, 4), (5, 12));
                self.path([(12, 1), (12, 4), (8, 4), (8, 12), (12, 12), (12, 15)]);
            }
            Kind::TransmissionGate => {
                self.path([(2, 3), (14, 13), (14, 3), (2, 13), (2, 3)]);
                self.line((8, 1), (8, 6));
                self.line((8, 10), (8, 15));
            }
            Kind::BitExtender => {
                self.path([(1, 6), (5, 6), (14, 2), (14, 14), (5, 10), (1, 10), (1, 6)]);
            }
            Kind::Latch(latch) => self.storage(latch == Latch::Sr),
            Kind::FlipFlop(ff) => {
                self.storage(ff.paired());
                self.path([(3, 9), (6, 11), (3, 13)]);
                if matches!(ff, FlipFlop::T | FlipFlop::Jk) {
                    self.path([(7, 7), (9, 7), (9, 5), (11, 5)]);
                }
            }
            Kind::Register | Kind::ShiftRegister => {
                self.frame((1, 3), (15, 13));
                if kind == Kind::Register {
                    for x in [6, 10] {
                        self.line((x, 3), (x, 13));
                    }
                } else {
                    self.arrow();
                }
            }
            Kind::Counter => {
                self.path([(1, 14), (1, 11), (6, 11), (6, 7), (10, 7), (10, 3), (15, 3)])
            }
            Kind::Random => {
                self.frame((2, 2), (14, 14));
                for point in [(5, 5), (8, 8), (11, 11)] {
                    self.dot(point);
                }
            }
            Kind::Rom => {
                self.memory_chip();
                self.line((6, 8), (10, 8));
            }
            Kind::Ram => {
                self.memory_chip();
                self.line((6, 6), (10, 6));
                self.line((6, 10), (10, 10));
            }
            Kind::PlaRom => {
                self.memory_chip();
                self.line((8, 4), (8, 12));
                self.line((5, 8), (11, 8));
                self.dot((8, 8));
            }
            Kind::Button => {
                self.frame((3, 2), (13, 6));
                self.path([(8, 6), (8, 10), (13, 10)]);
                self.line((2, 10), (8, 10));
                self.path([(1, 14), (4, 14), (4, 12)]);
                self.path([(12, 12), (12, 14), (15, 14)]);
            }
            Kind::Switch => {
                self.line((1, 11), (4, 11));
                self.ring((5, 11), 1.4);
                self.line((6, 10), (12, 4));
                self.ring((13, 11), 1.4);
            }
            Kind::DipSwitch => {
                self.frame((1, 2), (15, 14));
                for (x, y) in [(5, 6), (11, 10)] {
                    self.line((x, 5), (x, 11));
                    self.dot((x, y));
                }
            }
            Kind::Slider => {
                self.line((1, 8), (7, 8));
                self.frame((7, 3), (11, 13));
                self.line((11, 8), (15, 8));
            }
            Kind::Joystick => {
                self.frame((2, 11), (14, 14));
                self.line((7, 11), (10, 6));
                self.ring((11, 4), 2.4);
            }
            Kind::Keyboard => {
                self.frame((1, 3), (15, 13));
                for x in [4, 8, 12] {
                    self.dot((x, 6));
                }
                self.line((5, 10), (11, 10));
            }
            Kind::Tty => {
                self.frame((1, 2), (15, 14));
                self.path([(4, 5), (7, 8), (4, 11)]);
                self.line((9, 11), (12, 11));
            }
            Kind::Led => {
                self.ring((8, 7), 5.);
                self.line((6, 12), (6, 15));
                self.line((10, 12), (10, 15));
            }
            Kind::RgbLed => {
                for center in [(8, 5), (5, 10), (11, 10)] {
                    self.ring(center, 3.);
                }
            }
            Kind::HexDisplay | Kind::SevenSegment => {
                self.display(kind == Kind::SevenSegment);
            }
            Kind::DotMatrix => {
                for x in [3, 8, 13] {
                    for y in [3, 8, 13] {
                        self.dot((x, y));
                    }
                }
            }
        }
    }

    fn gate(&self, gate: Gate, left: u8) {
        let tip = if gate.inverted() { 11 } else { 15 };
        match gate {
            Gate::Buffer | Gate::Not => self.path([(left, 2), (tip, 8), (left, 14), (left, 2)]),
            Gate::And | Gate::Nand => self.symbol(Icon::And, (left, 2), (tip, 14)),
            Gate::Or | Gate::Nor | Gate::Xor | Gate::Xnor => {
                self.symbol(Icon::Or, (left, 2), (tip, 14));
                if matches!(gate, Gate::Xor | Gate::Xnor) {
                    self.path([(left - 2, 3), (left, 8), (left - 2, 13)]);
                }
            }
        }
        if gate.inverted() {
            self.ring((14, 8), 1.7);
        }
    }

    fn plexer(&self, kind: Plexer) {
        let (left, right) = match kind {
            Plexer::Multiplexer | Plexer::BitSelector | Plexer::PriorityEncoder => (3, 13),
            Plexer::Demultiplexer | Plexer::Decoder | Plexer::DisplayDecoder => (13, 3),
        };
        self.path([(left, 1), (right, 4), (right, 12), (left, 15), (left, 1)]);
        let edge = if left == 3 { 0 } else { 16 };
        for y in [5, 11] {
            self.line((edge, y), (left, y));
        }
        match kind {
            Plexer::Multiplexer | Plexer::Demultiplexer => self.line((8, 14), (8, 16)),
            Plexer::Decoder | Plexer::PriorityEncoder => self.line((6, 8), (10, 8)),
            Plexer::BitSelector => self.dot((8, 8)),
            Plexer::DisplayDecoder => self.frame((6, 5), (10, 11)),
        }
    }

    fn arithmetic(&self, op: Arithmetic) {
        match op {
            Arithmetic::Adder => self.symbol(Icon::Plus, (1, 1), (15, 15)),
            Arithmetic::Subtractor => self.symbol(Icon::Minus, (1, 1), (15, 15)),
            Arithmetic::Multiplier => {
                self.line((3, 3), (13, 13));
                self.line((3, 13), (13, 3));
            }
            Arithmetic::Divider => {
                self.line((2, 8), (14, 8));
                self.dot((8, 3));
                self.dot((8, 13));
            }
            Arithmetic::Negator => self.path([(14, 4), (14, 9), (3, 9), (7, 5)]),
            Arithmetic::Comparator => {
                self.path([(3, 3), (12, 7), (3, 11)]);
                self.line((3, 14), (13, 14));
            }
            Arithmetic::Shifter => {
                self.arrow();
                self.line((14, 3), (14, 13));
            }
            Arithmetic::BitAdder => {
                self.frame((1, 2), (15, 14));
                self.symbol(Icon::Plus, (4, 4), (12, 12));
            }
            Arithmetic::BitFinder => self.symbol(Icon::Search, (1, 1), (15, 15)),
        }
    }

    fn arrow(&self) {
        self.line((4, 8), (11, 8));
        self.path([(8, 5), (11, 8), (8, 11)]);
    }

    fn storage(&self, paired: bool) {
        self.frame((3, 2), (13, 14));
        self.line((0, 5), (3, 5));
        self.line((13, 5), (16, 5));
        if paired {
            self.line((0, 8), (3, 8));
        }
    }

    fn memory_chip(&self) {
        self.frame((3, 2), (13, 14));
        for y in [5, 11] {
            self.line((0, y), (3, y));
            self.line((13, y), (16, y));
        }
    }

    fn display(&self, seven: bool) {
        self.frame((2, 1), (14, 15));
        self.path([(5, 8), (5, 4), (11, 4), (11, 12)]);
        self.line((5, 8), (11, 8));
        if seven {
            self.path([(5, 9), (5, 12), (10, 12)]);
        } else {
            self.line((5, 9), (5, 12));
        }
    }

    /// Abstract the package and its populated pin banks, including text-only plugins.
    fn package(&self, tool: &Tool) {
        let [[x0, y0], [x1, y1]] = tool.bounds;
        let width = (x1 - x0) as f32;
        let height = (y1 - y0) as f32;
        let aspect = (width / height).clamp(0.55, 1.8);
        let size = vec2(aspect.min(1.), (1. / aspect).min(1.)) * self.rect.width() * 0.72;
        let body = Rect::from_center_size(self.rect.center(), size);
        self.painter
            .rect_stroke(body, 1, self.stroke, StrokeKind::Middle);

        // Three slots per edge keep densely pinned packages legible at list size.
        let mut banks = [[false; 3]; 4];
        for port in &tool.ports {
            let [x, y] = port.at;
            let distances = [
                x.abs_diff(x0),
                x.abs_diff(x1),
                y.abs_diff(y0),
                y.abs_diff(y1),
            ];
            let side = (0..4)
                .min_by_key(|&i| distances[i])
                .expect("four package edges");
            let position = if side < 2 {
                (y - y0) as f32 / height
            } else {
                (x - x0) as f32 / width
            };
            banks[side][(position * 3.).clamp(0., 2.) as usize] = true;
        }
        for (side, slots) in banks.into_iter().enumerate() {
            let (start, along, outward) = match side {
                0 => (body.left_top(), Vec2::Y * size.y, -Vec2::X),
                1 => (body.right_top(), Vec2::Y * size.y, Vec2::X),
                2 => (body.left_top(), Vec2::X * size.x, -Vec2::Y),
                _ => (body.left_bottom(), Vec2::X * size.x, Vec2::Y),
            };
            for (i, occupied) in slots.into_iter().enumerate() {
                if occupied {
                    let at = start + along * (i as f32 + 0.5) / 3.;
                    self.painter
                        .line_segment([at, at + outward * 2.4], self.stroke);
                }
            }
        }
    }
}
