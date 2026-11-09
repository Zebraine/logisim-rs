//! Persistent component attributes and defaults. No instance runtime state.
mod connections;
mod ports;

use crate::{
    ArithmeticOptions, ArtworkFont, BusControl, Extension, Facing, GateOptions, Id, Interface, Io,
    Kind, LatchEnable, Memory, ParityOptions, PinOptions, Pla, Plexer, Plexing, Point, Radix,
    Sequential, Splitter, TerminalOptions, TransistorOptions, TtlOptions,
};

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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Component {
    pub plugin: Option<logisim_plugin_api::Component>,
    /// Derived interface; rebuilt from the project definition, never independently serialized.
    pub interface: Interface,
    /// Inert .circ library descriptor, never a runtime dependency.
    pub id: Id,
    /// Gate output / single terminal, or text anchor. Imported positions stay exact.
    pub position: Point,
    pub kind: Kind,
    pub facing: Facing,
    pub bits: u8,
    pub output: bool,
    pub pin: PinOptions,
    pub label: String,
    pub font: ArtworkFont,
    pub color: [u8; 3],
    pub color_alpha: u8,
    pub gate_options: GateOptions,
    pub text_horizontal: HorizontalAlign,
    pub text_vertical: VerticalAlign,
    pub constant: u32,
    pub radix: Radix,
    pub label_facing: Facing,
    pub label_center: bool,
    pub splitter: Splitter,
    pub input_bits: u8,
    pub extension: Extension,
    pub sequential: Sequential,
    pub plexing: Plexing,
    pub arithmetic: ArithmeticOptions,
    pub memory: Memory,
    pub pla: Pla,
    pub io: Io,
    pub terminal: TerminalOptions,
    pub bus: BusControl,
    pub transistor: TransistorOptions,
    pub parity: ParityOptions,
    pub ttl: TtlOptions,
    pub latch_enable: LatchEnable,
}
impl Component {
    pub fn new(kind: Kind, position: Point) -> Self {
        Self {
            plugin: None,
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
                | Kind::Random
                | Kind::Rom
                | Kind::Ram
                | Kind::Arithmetic(_)
                | Kind::Plexer(Plexer::BitSelector) => 8,
                _ => 1,
            },
            output: false,
            pin: PinOptions::default(),
            label: if kind == Kind::Tunnel {
                "tunnel".into()
            } else if kind == Kind::Text {
                "Text".into()
            } else {
                String::new()
            },
            font: ArtworkFont {
                size: if matches!(kind, Kind::Joystick | Kind::PlaRom | Kind::Ttl(_)) {
                    12
                } else {
                    14
                },
                ..Default::default()
            },
            color: if matches!(
                kind,
                Kind::Tunnel | Kind::Joystick | Kind::PlaRom | Kind::Ttl(_)
            ) {
                [0, 0, 0]
            } else {
                [51, 65, 85]
            },
            color_alpha: 255,
            gate_options: GateOptions::default(),
            text_horizontal: HorizontalAlign::Left,
            text_vertical: VerticalAlign::Top,
            constant: 1,
            radix: Radix::Binary,
            label_facing: if kind == Kind::Clock {
                Facing::West
            } else {
                Facing::North
            },
            label_center: false,
            splitter: Splitter::default(),
            input_bits: 8,
            extension: Extension::Zero,
            sequential: Sequential::default(),
            arithmetic: ArithmeticOptions::default(),
            memory: Memory::default(),
            pla: Pla::default(),
            io: Io::for_kind(kind),
            terminal: TerminalOptions::default(),
            bus: BusControl::default(),
            transistor: TransistorOptions::default(),
            parity: ParityOptions::default(),
            ttl: TtlOptions::default(),
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
}
