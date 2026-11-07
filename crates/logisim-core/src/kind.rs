//! Component identity and editor capabilities; family behavior lives in its own module.
use crate::{Arithmetic, FlipFlop, Gate, Id, Latch, Parity, Plexer, Ttl};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Plugin,
    Subcircuit(Id),
    Gate(Gate),
    Ttl(Ttl),
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
    Random,
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
    PlaRom,
    Ram,
    Plexer(Plexer),
    Arithmetic(Arithmetic),
}

impl Kind {
    pub fn rotatable(self) -> bool {
        !matches!(
            self,
            Self::Plugin
                | Self::Text
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
                | Self::Random
                | Self::Rom
                | Self::PlaRom
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
                | Self::Ttl(_)
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
                | Self::Random
                | Self::Rom
                | Self::PlaRom
                | Self::Ram
        )
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Plugin => "Plugin component",
            Self::Subcircuit(_) => "Subcircuit",
            Self::Gate(g) => g.name(),
            Self::Ttl(t) => t.name(),
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
            Self::Random => "Random Generator",
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
            Self::PlaRom => "PLA ROM",
            Self::Ram => "RAM",
            Self::Plexer(p) => p.name(),
            Self::Arithmetic(a) => a.name(),
        }
    }
}
