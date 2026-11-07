//! Circuit geometry and signal values. No GUI, file format or simulation runtime dependencies.

pub mod plugin;
mod signal;
pub use signal::{Bit, Value};

mod appearance;
mod external;
pub use appearance::{
    Appearance, Artwork, ArtworkFont, Geometry, MAX_SHAPES, Paint, Rgb, TextAnchor,
};

mod library;
mod project;
pub use library::LibraryImport;
pub use project::{
    CircuitDefinition, CircuitLabel, Interface, InterfacePort, MAX_CIRCUITS, Project,
};

mod arithmetic;
mod bus;
mod gates;
mod latches;
mod parity;
mod pins;
mod transistors;
mod ttl;
pub use arithmetic::{Arithmetic, ArithmeticOptions, BitSearch, Shift};
pub use bus::{BusControl, Pull};
pub use gates::{Gate, GateOptions, GateSize};
pub use latches::{Latch, LatchEnable, LatchRole};
pub use parity::{Parity, ParityOptions, ParityOutput};
pub use pins::{PinOptions, PinPull};
pub use transistors::{TransistorOptions, TransistorType};
pub use ttl::{Ttl, TtlOptions};

mod plexers;
pub use plexers::{Plexer, PlexerRole, PlexerTerminal, Plexing};

mod sequential;
pub use sequential::{ControlPosition, CounterOverflow, FlipFlop, Sequential, Trigger};

mod io;
mod joystick;
mod terminal;
pub use io::{Io, MatrixInput};
pub use terminal::TerminalOptions;
mod pla;
pub use pla::{Pla, PlaInput};
mod memory;
pub use memory::{Memory, MemoryContents, MemoryInterface, MemoryRole, MemoryTerminal};

mod wiring;
pub use wiring::{Extension, Radix, SplitAppearance, Splitter};

mod circuit;
mod component;
mod geometry;
mod kind;

pub use circuit::Circuit;
pub use component::{Component, HorizontalAlign, VerticalAlign};
pub use geometry::{Facing, GRID, Point, Wire};
pub use kind::Kind;

pub type Id = u64;
