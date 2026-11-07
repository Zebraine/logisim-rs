//! Editable circuit geometry. No GUI, file format or signal evaluation dependencies.

mod appearance;
pub use appearance::{
    Appearance, Artwork, ArtworkFont, Geometry, MAX_SHAPES, Paint, Rgb, TextAnchor,
};

mod project;
pub use project::{CircuitDefinition, Interface, InterfacePort, MAX_CIRCUITS, Project};

mod arithmetic;
mod bus;
mod latches;
mod parity;
mod transistors;
pub use arithmetic::{Arithmetic, ArithmeticOptions, BitSearch, Shift};
pub use bus::{BusControl, Pull};
pub use latches::{Latch, LatchEnable, LatchRole};
pub use parity::{Parity, ParityOptions, ParityOutput};
pub use transistors::{TransistorOptions, TransistorType};

mod plexers;
pub use plexers::{Plexer, PlexerRole, PlexerTerminal, Plexing};

mod sequential;
pub use sequential::{ControlPosition, CounterOverflow, FlipFlop, Sequential, Trigger};

mod io;
mod joystick;
mod terminal;
pub use io::{Io, MatrixInput};
pub use terminal::TerminalOptions;
mod memory;
pub use memory::{Memory, MemoryContents, MemoryInterface, MemoryRole, MemoryTerminal};

mod wire_geometry;
mod wiring;
pub use wiring::{Extension, Radix, SplitAppearance, Splitter};

use std::collections::{BTreeMap, BTreeSet};
use std::ops::{Add, Sub};

pub const GRID: i32 = 10;
pub type Id = u64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const ZERO: Self = Self::new(0, 0);
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
    pub fn snapped(x: f32, y: f32) -> Self {
        let snap = |v: f32| (v.clamp(-1e6, 1e6) / GRID as f32).round() as i32 * GRID;
        Self::new(snap(x), snap(y))
    }
}
impl Add for Point {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x.saturating_add(rhs.x), self.y.saturating_add(rhs.y))
    }
}
impl Sub for Point {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x.saturating_sub(rhs.x), self.y.saturating_sub(rhs.y))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Facing {
    #[default]
    East,
    South,
    West,
    North,
}
impl Facing {
    pub const ALL: [Self; 4] = [Self::East, Self::South, Self::West, Self::North];
    pub fn name(self) -> &'static str {
        match self {
            Self::East => "East",
            Self::South => "South",
            Self::West => "West",
            Self::North => "North",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::North,
            Self::North => Self::East,
        }
    }
    pub fn inverse(self) -> Self {
        match self {
            Self::South => Self::North,
            Self::North => Self::South,
            other => other,
        }
    }
    pub fn transform(self, p: Point) -> Point {
        match self {
            Self::East => p,
            Self::South => Point::new(p.y.saturating_neg(), p.x),
            Self::West => Point::new(p.x.saturating_neg(), p.y.saturating_neg()),
            Self::North => Point::new(p.y, p.x.saturating_neg()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    Not,
    Buffer,
    And,
    Or,
    Nand,
    Nor,
    Xor,
    Xnor,
}
impl Gate {
    pub const ALL: [Self; 8] = [
        Self::Not,
        Self::Buffer,
        Self::And,
        Self::Or,
        Self::Nand,
        Self::Nor,
        Self::Xor,
        Self::Xnor,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Not => "NOT Gate",
            Self::Buffer => "Buffer",
            Self::And => "AND Gate",
            Self::Or => "OR Gate",
            Self::Nand => "NAND Gate",
            Self::Nor => "NOR Gate",
            Self::Xor => "XOR Gate",
            Self::Xnor => "XNOR Gate",
        }
    }
    pub fn unary(self) -> bool {
        matches!(self, Self::Not | Self::Buffer)
    }
    pub fn inverted(self) -> bool {
        matches!(self, Self::Not | Self::Nand | Self::Nor | Self::Xnor)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Subcircuit(Id),
    Gate(Gate),
    Parity(Parity),
    ControlledBuffer,
    ControlledInverter,
    PullResistor,
    Transistor,
    TransmissionGate,
    Pin,
    Tunnel,
    Text,
    Constant,
    Power,
    Ground,
    Probe,
    Splitter,
    BitExtender,
    Clock,
    FlipFlop(FlipFlop),
    Latch(Latch),
    Register,
    Counter,
    ShiftRegister,
    Button,
    Switch,
    DipSwitch,
    Slider,
    Joystick,
    Keyboard,
    Tty,
    Led,
    SevenSegment,
    HexDisplay,
    RgbLed,
    DotMatrix,
    Rom,
    Ram,
    Plexer(Plexer),
    Arithmetic(Arithmetic),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GateSize {
    #[default]
    Narrow,
    Medium,
    Wide,
}
impl GateSize {
    pub fn name(self) -> &'static str {
        match self {
            Self::Narrow => "Narrow",
            Self::Medium => "Medium",
            Self::Wide => "Wide",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HorizontalAlign {
    #[default]
    Left,
    Center,
    Right,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VerticalAlign {
    #[default]
    Top,
    Center,
    Baseline,
    Bottom,
}
impl Kind {
    pub fn rotatable(self) -> bool {
        !matches!(
            self,
            Self::Text
                | Self::Keyboard
                | Self::Tty
                | Self::SevenSegment
                | Self::HexDisplay
                | Self::DotMatrix
                | Self::BitExtender
                | Self::Latch(_)
                | Self::FlipFlop(_)
                | Self::Register
                | Self::Counter
                | Self::ShiftRegister
                | Self::Rom
                | Self::Ram
                | Self::Arithmetic(_)
        )
    }
    pub fn has_label(self) -> bool {
        matches!(
            self,
            Self::Text
                | Self::Tunnel
                | Self::Button
                | Self::Switch
                | Self::DipSwitch
                | Self::Slider
                | Self::Led
                | Self::RgbLed
                | Self::Pin
                | Self::Gate(_)
                | Self::Parity(_)
                | Self::ControlledBuffer
                | Self::ControlledInverter
                | Self::Probe
                | Self::Subcircuit(_)
                | Self::Clock
                | Self::Latch(_)
                | Self::FlipFlop(_)
                | Self::Register
                | Self::Counter
                | Self::ShiftRegister
                | Self::Rom
                | Self::Ram
        )
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Subcircuit(_) => "Subcircuit",
            Self::Gate(g) => g.name(),
            Self::Latch(l) => l.name(),
            Self::Parity(p) => p.name(),
            Self::ControlledBuffer => "Controlled Buffer",
            Self::ControlledInverter => "Controlled Inverter",
            Self::PullResistor => "Pull Resistor",
            Self::Transistor => "Transistor",
            Self::TransmissionGate => "Transmission Gate",
            Self::Pin => "Pin",
            Self::Tunnel => "Tunnel",
            Self::Text => "Text",
            Self::Constant => "Constant",
            Self::Power => "Power",
            Self::Ground => "Ground",
            Self::Probe => "Probe",
            Self::Splitter => "Splitter",
            Self::BitExtender => "Bit Extender",
            Self::Clock => "Clock",
            Self::FlipFlop(ff) => ff.name(),
            Self::Register => "Register",
            Self::Counter => "Counter",
            Self::ShiftRegister => "Shift Register",
            Self::Button => "Button",
            Self::Switch => "Switch",
            Self::DipSwitch => "Dip Switch",
            Self::Slider => "Slider",
            Self::Joystick => "Joystick",
            Self::Keyboard => "Keyboard",
            Self::Tty => "TTY",
            Self::Led => "LED",
            Self::SevenSegment => "7-Segment Display",
            Self::HexDisplay => "Hex Digit Display",
            Self::RgbLed => "RGB LED",
            Self::DotMatrix => "LED Matrix",
            Self::Rom => "ROM",
            Self::Ram => "RAM",
            Self::Plexer(p) => p.name(),
            Self::Arithmetic(a) => a.name(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Component {
    /// Derived interface; rebuilt from the project definition, never independently serialized.
    pub interface: Interface,
    pub id: Id,
    /// Gate output / single terminal, or text anchor. Imported positions stay exact.
    pub position: Point,
    pub kind: Kind,
    pub facing: Facing,
    pub bits: u8,
    pub inputs: u8,
    pub output: bool,
    pub label: String,
    pub font_size: u8,
    pub color: [u8; 3],
    pub gate_size: GateSize,
    pub text_horizontal: HorizontalAlign,
    pub text_vertical: VerticalAlign,
    pub constant: u32,
    pub radix: Radix,
    pub label_facing: Facing,
    pub splitter: Splitter,
    pub input_bits: u8,
    pub extension: Extension,
    pub sequential: Sequential,
    pub plexing: Plexing,
    pub arithmetic: ArithmeticOptions,
    pub memory: Memory,
    pub io: Io,
    pub terminal: TerminalOptions,
    pub bus: BusControl,
    pub transistor: TransistorOptions,
    pub parity: ParityOptions,
    pub latch_enable: LatchEnable,
}
impl Component {
    pub fn new(kind: Kind, position: Point) -> Self {
        Self {
            interface: Interface::default(),
            id: 0,
            position,
            kind,
            facing: match kind {
                Kind::Power => Facing::North,
                Kind::Ground | Kind::PullResistor => Facing::South,
                Kind::Led | Kind::RgbLed => Facing::West,
                _ => Facing::East,
            },
            bits: match kind {
                Kind::Splitter | Kind::HexDisplay | Kind::Joystick => 4,
                Kind::BitExtender => 16,
                Kind::Keyboard | Kind::Tty => 7,
                Kind::Slider
                | Kind::Register
                | Kind::Counter
                | Kind::ShiftRegister
                | Kind::Rom
                | Kind::Ram
                | Kind::Arithmetic(_)
                | Kind::Plexer(Plexer::BitSelector) => 8,
                _ => 1,
            },
            inputs: 2,
            output: false,
            label: if kind == Kind::Tunnel {
                "tunnel".into()
            } else if kind == Kind::Text {
                "Text".into()
            } else {
                String::new()
            },
            font_size: if kind == Kind::Joystick { 12 } else { 14 },
            color: if matches!(kind, Kind::Tunnel | Kind::Joystick) {
                [0, 0, 0]
            } else {
                [51, 65, 85]
            },
            gate_size: GateSize::Narrow,
            text_horizontal: HorizontalAlign::Left,
            text_vertical: VerticalAlign::Top,
            constant: 1,
            radix: Radix::Binary,
            label_facing: if kind == Kind::Clock {
                Facing::West
            } else {
                Facing::North
            },
            splitter: Splitter::default(),
            input_bits: 8,
            extension: Extension::Zero,
            sequential: Sequential::default(),
            arithmetic: ArithmeticOptions::default(),
            memory: Memory::default(),
            io: Io::for_kind(kind),
            terminal: TerminalOptions::default(),
            bus: BusControl::default(),
            transistor: TransistorOptions::default(),
            parity: ParityOptions::default(),
            latch_enable: LatchEnable::default(),
            plexing: Plexing {
                select_bits: if kind == Kind::Plexer(Plexer::PriorityEncoder) {
                    3
                } else {
                    1
                },
                ..Plexing::default()
            },
        }
    }
    pub fn input_count(&self) -> u8 {
        match self.kind {
            Kind::Gate(g) if g.unary() => 1,
            Kind::Gate(_) => self.inputs.clamp(2, 8),
            Kind::Parity(_) => self.inputs.clamp(2, 32),
            _ => 0,
        }
    }
    pub fn local_ports(&self) -> Vec<Point> {
        if matches!(self.kind, Kind::Parity(_)) {
            return self.parity_ports();
        }
        if self.kind.is_transistor() {
            return self.transistor_ports();
        }
        if self.kind.is_controlled() {
            return self.controlled_ports();
        }
        if self.kind.is_io() {
            return self.io_ports();
        }
        if self.kind.is_memory() {
            return self.memory_terminals().iter().map(|p| p.offset).collect();
        }
        if matches!(self.kind, Kind::Plexer(_)) {
            return self
                .plexer_terminals()
                .into_iter()
                .map(|p| p.offset)
                .collect();
        }
        if let Some(ports) = self.arithmetic_ports() {
            return ports;
        }
        if let Some(ports) = self.sequential_ports() {
            return ports;
        }
        if matches!(self.kind, Kind::Subcircuit(_)) {
            return self.interface.ports.iter().map(|p| p.offset).collect();
        }
        if self.kind == Kind::Text {
            return vec![];
        }
        if self.kind == Kind::Splitter {
            let mut ports = self.splitter.ports();
            if matches!(self.facing, Facing::West | Facing::North) {
                ports[1..].reverse();
            }
            return ports;
        }
        if self.kind == Kind::BitExtender {
            let mut ports = vec![Point::ZERO, Point::new(-40, 0)];
            if self.extension == Extension::Input {
                ports.push(Point::new(-20, -20));
            }
            return ports;
        }
        let count = self.input_count();
        let mut ports = vec![Point::ZERO];
        for i in 0..count {
            let y = if count == 1 {
                0
            } else {
                // Positions measured from the running reference for 2–8 inputs.
                let half = i32::from(count / 2);
                let index = i32::from(i) - half;
                let index = if count.is_multiple_of(2) && index >= 0 {
                    index + 1
                } else {
                    index
                };
                match (self.gate_size, count) {
                    (GateSize::Medium | GateSize::Wide, 2) | (GateSize::Medium, 3) => index * 20,
                    (GateSize::Wide, 3) => index * 30,
                    (GateSize::Wide, 4) => [-30, -10, 10, 30][usize::from(i)],
                    _ => index * GRID,
                }
            };
            ports.push(Point::new(-self.gate_depth(), y));
        }
        ports
    }
    pub fn gate_width(&self) -> i32 {
        match self.kind {
            Kind::Gate(Gate::Buffer) | Kind::ControlledBuffer => 20,
            Kind::Gate(Gate::Not) | Kind::ControlledInverter => {
                if self.gate_size == GateSize::Wide {
                    30
                } else {
                    20
                }
            }
            Kind::Gate(_) | Kind::Parity(_) => match self.gate_size {
                GateSize::Narrow => 30,
                GateSize::Medium => 50,
                GateSize::Wide => 70,
            },
            _ => 0,
        }
    }
    pub fn gate_depth(&self) -> i32 {
        self.gate_width()
            + match self.kind {
                Kind::Gate(Gate::Nand | Gate::Nor | Gate::Xor) => 10,
                Kind::Gate(Gate::Xnor) => 20,
                _ => 0,
            }
    }
    pub fn ports(&self) -> Vec<Point> {
        self.local_ports()
            .into_iter()
            .map(|p| self.position + self.facing.transform(p))
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wire {
    pub id: Id,
    pub a: Point,
    pub b: Point,
}
impl Wire {
    pub fn horizontal(self) -> bool {
        self.a.y == self.b.y
    }
    pub fn contains(self, p: Point) -> bool {
        if self.horizontal() {
            p.y == self.a.y && (self.a.x.min(self.b.x)..=self.a.x.max(self.b.x)).contains(&p.x)
        } else {
            p.x == self.a.x && (self.a.y.min(self.b.y)..=self.a.y.max(self.b.y)).contains(&p.y)
        }
    }
    pub fn touches(self, other: Self) -> bool {
        self.contains(other.a)
            || self.contains(other.b)
            || other.contains(self.a)
            || other.contains(self.b)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Circuit {
    pub appearance: Option<Appearance>,
    pub components: Vec<Component>,
    pub wires: Vec<Wire>,
    next_id: Id,
}
impl Circuit {
    fn id(&mut self) -> Id {
        self.next_id += 1;
        self.next_id
    }
    pub fn add_component(&mut self, mut component: Component) -> Id {
        component.id = self.id();
        let id = component.id;
        self.components.push(component);
        id
    }
    pub fn component(&self, id: Id) -> Option<&Component> {
        self.components.iter().find(|c| c.id == id)
    }
    pub fn ids(&self) -> BTreeSet<Id> {
        self.components
            .iter()
            .map(|c| c.id)
            .chain(self.wires.iter().map(|w| w.id))
            .collect()
    }
    pub fn remove(&mut self, ids: &BTreeSet<Id>) {
        self.components.retain(|c| !ids.contains(&c.id));
        self.wires.retain(|w| !ids.contains(&w.id));
    }
    pub fn add_wire(&mut self, a: Point, b: Point) -> Option<Id> {
        if a == b || (a.x != b.x && a.y != b.y) {
            return None;
        }
        if let Some(w) = self
            .wires
            .iter()
            .find(|w| (w.a == a && w.b == b) || (w.a == b && w.b == a))
        {
            return Some(w.id);
        }
        let id = self.id();
        self.wires.push(Wire { id, a, b });
        Some(id)
    }
    pub fn route(a: Point, b: Point, horizontal_first: bool) -> [Point; 3] {
        [
            a,
            if horizontal_first {
                Point::new(b.x, a.y)
            } else {
                Point::new(a.x, b.y)
            },
            b,
        ]
    }
    pub fn add_route(&mut self, a: Point, b: Point, horizontal_first: bool) -> Vec<Id> {
        Self::route(a, b, horizontal_first)
            .windows(2)
            .filter_map(|p| self.add_wire(p[0], p[1]))
            .collect()
    }
    /// Split at terminals before rerouting, so a terminal on a wire's interior stays connected.
    fn split_at(&mut self, points: &[Point], skip: &BTreeSet<Id>) {
        let old = std::mem::take(&mut self.wires);
        for wire in old {
            if skip.contains(&wire.id) {
                self.wires.push(wire);
                continue;
            }
            let mut stops = vec![wire.a, wire.b];
            stops.extend(points.iter().copied().filter(|p| wire.contains(*p)));
            stops.sort();
            stops.dedup();
            for (index, pair) in stops.windows(2).enumerate() {
                let id = if index == 0 { wire.id } else { self.id() };
                self.wires.push(Wire {
                    id,
                    a: pair[0],
                    b: pair[1],
                });
            }
        }
    }
    pub fn move_selection(&mut self, ids: &BTreeSet<Id>, delta: Point) {
        if delta == Point::ZERO {
            return;
        }
        let mut old_ports: Vec<_> = self
            .components
            .iter()
            .filter(|c| ids.contains(&c.id))
            .flat_map(Component::ports)
            .collect();
        let moving_wires: Vec<_> = self
            .wires
            .iter()
            .filter(|w| ids.contains(&w.id))
            .copied()
            .collect();
        old_ports.extend(
            self.wires
                .iter()
                .flat_map(|w| [w.a, w.b])
                .chain(self.components.iter().flat_map(Component::ports))
                .filter(|point| moving_wires.iter().any(|w| w.contains(*point))),
        );
        old_ports.sort();
        old_ports.dedup();
        self.split_at(&old_ports, ids);
        let fixed: BTreeSet<_> = self
            .components
            .iter()
            .filter(|c| !ids.contains(&c.id))
            .flat_map(Component::ports)
            .collect();
        for c in &mut self.components {
            if ids.contains(&c.id) {
                c.position = c.position + delta;
            }
        }
        let map: BTreeMap<_, _> = old_ports
            .iter()
            .filter(|p| !fixed.contains(p))
            .map(|p| (*p, *p + delta))
            .collect();
        self.reroute(&map, ids, delta);
        for point in old_ports.iter().filter(|p| fixed.contains(p)) {
            self.add_route(*point, *point + delta, true);
        }
    }
    /// Change attributes and keep surviving port indices attached to their wires.
    pub fn replace_component(&mut self, replacement: Component) {
        let Some(old) = self.component(replacement.id).cloned() else {
            return;
        };
        if old == replacement {
            return;
        }
        let before = old.ports();
        let after = replacement.ports();
        let pairs: Vec<_> = if old.kind == replacement.kind
            && (old.kind.is_memory()
                || matches!(old.kind, Kind::DotMatrix | Kind::Joystick)
                || old.kind.is_transistor()
                || matches!(old.kind, Kind::Parity(_) | Kind::Latch(_))
                || (old.kind.is_controlled()
                    && (old.bus.control_left != replacement.bus.control_left
                        || old.gate_size != replacement.gate_size))
                || (old.kind == Kind::Arithmetic(Arithmetic::BitAdder)
                    && old.bit_adder_inputs() != replacement.bit_adder_inputs())
                || (old.kind == Kind::Plexer(Plexer::DisplayDecoder)
                    && old.plexing.multibit != replacement.plexing.multibit))
        {
            // Observed input layout edits keep wire geometry; a simultaneous move translates it.
            let delta = replacement.position - old.position;
            before.iter().map(|p| (*p, *p + delta)).collect()
        } else if matches!(old.kind, Kind::Subcircuit(_)) && old.kind == replacement.kind {
            old.interface
                .ports
                .iter()
                .zip(&before)
                .filter_map(|(port, before)| {
                    replacement
                        .interface
                        .ports
                        .iter()
                        .position(|p| p.pin == port.pin)
                        .map(|index| (*before, after[index]))
                })
                .collect()
        } else if matches!(old.kind, Kind::Plexer(_)) && old.kind == replacement.kind {
            let new = replacement.plexer_terminals();
            old.plexer_terminals()
                .iter()
                .zip(&before)
                .filter_map(|(port, point)| {
                    new.iter()
                        .position(|p| p.role == port.role)
                        .map(|i| (*point, after[i]))
                })
                .collect()
        } else {
            before.iter().copied().zip(after).collect()
        };
        let map: BTreeMap<_, _> = pairs.into_iter().filter(|(a, b)| a != b).collect();
        if !map.is_empty() {
            self.split_at(&before, &BTreeSet::new());
        }
        *self.components.iter_mut().find(|c| c.id == old.id).unwrap() = replacement;
        self.reroute(&map, &BTreeSet::new(), Point::ZERO);
    }
    fn reroute(&mut self, ports: &BTreeMap<Point, Point>, selected: &BTreeSet<Id>, delta: Point) {
        let old = std::mem::take(&mut self.wires);
        for wire in old {
            let (a, b) = if selected.contains(&wire.id) {
                (wire.a + delta, wire.b + delta)
            } else {
                (
                    *ports.get(&wire.a).unwrap_or(&wire.a),
                    *ports.get(&wire.b).unwrap_or(&wire.b),
                )
            };
            let route = Self::route(a, b, wire.horizontal());
            let mut first = true;
            for pair in route.windows(2) {
                if pair[0] != pair[1] {
                    let id = if first { wire.id } else { self.id() };
                    self.wires.push(Wire {
                        id,
                        a: pair[0],
                        b: pair[1],
                    });
                    first = false;
                }
            }
        }
    }
    pub fn junctions(&self) -> Vec<Point> {
        let ports: BTreeSet<_> = self.components.iter().flat_map(Component::ports).collect();
        let points: BTreeSet<_> = self
            .wires
            .iter()
            .flat_map(|w| [w.a, w.b])
            .chain(ports.iter().copied())
            .collect();
        points
            .into_iter()
            .filter(|p| {
                let branches: usize = self
                    .wires
                    .iter()
                    .filter(|w| w.contains(*p))
                    .map(|w| if w.a == *p || w.b == *p { 1 } else { 2 })
                    .sum();
                branches + usize::from(ports.contains(p)) >= 3
            })
            .collect()
    }
    /// Topological highlighting only; no values, propagation or simulation semantics.
    pub fn connected_wires(&self, seed: Id) -> BTreeSet<Id> {
        let mut result = BTreeSet::from([seed]);
        let ports: BTreeSet<_> = self.components.iter().flat_map(Component::ports).collect();
        let tunnels: Vec<_> = self
            .components
            .iter()
            .filter(|c| c.kind == Kind::Tunnel && !c.label.is_empty())
            .collect();
        loop {
            let mut next = result.clone();
            let attached: Vec<_> = tunnels
                .iter()
                .filter(|t| {
                    self.wires
                        .iter()
                        .any(|w| result.contains(&w.id) && w.contains(t.position))
                })
                .collect();
            for wire in &self.wires {
                if self.wires.iter().any(|w| {
                    result.contains(&w.id)
                        && (w.touches(*wire)
                            || ports.iter().any(|p| w.contains(*p) && wire.contains(*p)))
                }) || tunnels.iter().any(|t| {
                    wire.contains(t.position) && attached.iter().any(|a| a.label == t.label)
                }) {
                    next.insert(wire.id);
                }
            }
            if next == result {
                return result;
            }
            result = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moving_a_wire_keeps_attached_branch_and_fixed_pin_connected() {
        let mut d = Circuit::default();
        let pin = d.add_component(Component::new(Kind::Pin, Point::ZERO));
        let moving = d.add_wire(Point::ZERO, Point::new(100, 0)).unwrap();
        d.add_wire(Point::new(50, 0), Point::new(50, 100));
        d.move_selection(&BTreeSet::from([moving]), Point::new(20, 30));
        assert_eq!(d.component(pin).unwrap().position, Point::ZERO);
        assert!(
            d.wires
                .iter()
                .any(|w| w.id == moving && w.a == Point::new(20, 30) && w.b == Point::new(120, 30))
        );
        assert_eq!(d.connected_wires(moving).len(), d.wires.len());
    }
    #[test]
    fn moving_a_whole_selection_translates_each_wire_once() {
        let mut d = Circuit::default();
        d.add_component(Component::new(Kind::Pin, Point::new(50, 0)));
        d.add_wire(Point::ZERO, Point::new(100, 0));
        d.move_selection(&d.ids(), Point::new(20, 30));
        assert_eq!(d.wires.len(), 1);
        assert_eq!(
            (d.wires[0].a, d.wires[0].b),
            (Point::new(20, 30), Point::new(120, 30))
        );
        assert_eq!(d.components[0].position, Point::new(70, 30));
    }
    #[test]
    fn gates_have_grid_aligned_ports_in_all_orientations() {
        for gate in Gate::ALL {
            for facing in Facing::ALL {
                for inputs in 2..=8 {
                    let mut c = Component::new(Kind::Gate(gate), Point::new(120, 80));
                    c.facing = facing;
                    c.inputs = inputs;
                    let ports = c.ports();
                    assert_eq!(ports.len(), usize::from(c.input_count()) + 1);
                    assert_eq!(
                        ports.iter().copied().collect::<BTreeSet<_>>().len(),
                        ports.len()
                    );
                    assert!(ports.iter().all(|p| p.x % GRID == 0 && p.y % GRID == 0));
                }
            }
        }
    }
    #[test]
    fn moving_connected_gate_keeps_terminal_and_branch() {
        let mut d = Circuit::default();
        let id = d.add_component(Component::new(Kind::Gate(Gate::And), Point::new(200, 100)));
        let port = d.component(id).unwrap().ports()[1];
        d.add_wire(Point::new(0, port.y), port);
        d.add_wire(Point::new(50, port.y), Point::new(50, 200));
        d.move_selection(&BTreeSet::from([id]), Point::new(40, 30));
        assert!(
            d.wires
                .iter()
                .any(|w| w.a == port + Point::new(40, 30) || w.b == port + Point::new(40, 30))
        );
        assert!(d.junctions().contains(&Point::new(50, port.y)));
        assert!(d.wires.iter().all(|w| w.a.x == w.b.x || w.a.y == w.b.y));
    }
    #[test]
    fn moving_a_terminal_on_a_wire_interior_preserves_both_sides() {
        let mut d = Circuit::default();
        let id = d.add_component(Component::new(Kind::Pin, Point::new(50, 0)));
        d.add_wire(Point::ZERO, Point::new(100, 0));
        d.move_selection(&BTreeSet::from([id]), Point::new(0, 30));
        let seed = d.wires[0].id;
        assert_eq!(d.connected_wires(seed).len(), d.wires.len());
        assert!(d.wires.iter().any(|w| w.contains(Point::new(50, 30))));
    }
    #[test]
    fn crossing_is_separate_until_an_endpoint_forms_a_junction() {
        let mut d = Circuit::default();
        let a = d.add_wire(Point::new(0, 50), Point::new(100, 50)).unwrap();
        let b = d.add_wire(Point::new(50, 0), Point::new(50, 100)).unwrap();
        assert!(!d.connected_wires(a).contains(&b));
        assert!(d.junctions().is_empty());
        d.add_wire(Point::new(50, 50), Point::new(80, 50));
        assert!(d.connected_wires(a).contains(&b));
        assert!(d.junctions().contains(&Point::new(50, 50)));
    }
    #[test]
    fn same_named_tunnels_join_separate_wires_without_changing_geometry() {
        let mut d = Circuit::default();
        for x in [0, 200] {
            let mut t = Component::new(Kind::Tunnel, Point::new(x, 0));
            t.label = "DATA".into();
            d.add_component(t);
        }
        let a = d.add_wire(Point::ZERO, Point::new(0, 50)).unwrap();
        let b = d.add_wire(Point::new(200, 0), Point::new(200, 50)).unwrap();
        assert!(d.connected_wires(a).contains(&b));
        d.components[1].bits = 4;
        assert!(d.connected_wires(a).contains(&b));
        d.components[1].label = "data".into();
        assert!(!d.connected_wires(a).contains(&b));
    }
    #[test]
    fn rotation_preserves_existing_port_connections() {
        let mut d = Circuit::default();
        let id = d.add_component(Component::new(Kind::Gate(Gate::Not), Point::new(100, 100)));
        let before = d.component(id).unwrap().ports()[1];
        d.add_wire(Point::new(0, 100), before);
        let mut changed = d.component(id).unwrap().clone();
        changed.facing = Facing::South;
        d.replace_component(changed);
        assert!(d.wires.iter().any(|w| w.contains(Point::new(100, 80))));
        assert!(d.wires.iter().all(|w| w.a.x == w.b.x || w.a.y == w.b.y));
    }
}
