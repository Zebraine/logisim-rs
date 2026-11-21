//! V1 records common properties and each kind’s applicable options explicitly.
//! Extra document overrides survive native saves. Ports and local IDs are derived.
use super::*;
use logisim_core::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct Part {
    pub agent_id: String,
    pub kind: PartKind,
    pub position: Point,
    pub properties: Properties,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin: Option<plugin::api::Component>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemoryData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pla: Option<PlaData>,
}

// Exhaustive in both directions. Only subcircuits have a document reference.
macro_rules! part_kinds {
    ($($unit:ident),* ; $($family:ident),*) => {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        #[serde(rename_all="snake_case", deny_unknown_fields)]
        pub(super) enum PartKind { $($unit,)* $($family($family),)* Subcircuit(String) }
        impl PartKind {
            fn encode(kind: Kind, names: &BTreeMap<Id,String>) -> Result<Self> {
                Ok(match kind {
                    $(Kind::$unit => Self::$unit,)*
                    $(Kind::$family(value) => Self::$family(value),)*
                    Kind::Subcircuit(id) => Self::Subcircuit(names.get(&id).ok_or_else(|| Error(format!("Missing subcircuit {id}")))?.clone()),
                })
            }
            fn decode(self, names: &BTreeMap<String, Id>) -> Result<Kind> {
                Ok(match self {
                    $(Self::$unit => Kind::$unit,)*
                    $(Self::$family(value) => Kind::$family(value),)*
                    Self::Subcircuit(id) => Kind::Subcircuit(*names.get(&id).ok_or_else(|| Error(format!("Unknown subcircuit {id}")))?),
                })
            }
        }
    }
}
part_kinds!(Plugin, ControlledBuffer, ControlledInverter, PullResistor, Transistor,
    TransmissionGate, Pin, Tunnel, Text, Constant, Power, Ground, Probe, Splitter,
    BitExtender, Clock, Register, Counter, ShiftRegister, Random, Button, Switch,
    DipSwitch, Slider, Joystick, Keyboard, Tty, Led, SevenSegment, HexDisplay,
    RgbLed, DotMatrix, Rom, PlaRom, Ram; Gate, FlipFlop, Parity, Latch, Plexer, Arithmetic);

macro_rules! properties {
    ($($field:ident: $ty:ty => $kinds:pat),+ $(,)?) => {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        #[serde(deny_unknown_fields)]
        pub(super) struct Properties {
            $(#[serde(skip_serializing_if="Option::is_none")] $field: Option<$ty>,)+
        }
        impl Properties {
            fn encode(c: &Component) -> Self {
                let defaults = Component::new(c.kind, c.position);
                Self { $($field: (matches!(c.kind, $kinds) || c.$field != defaults.$field)
                    .then(|| c.$field.clone()),)+ }
            }
            fn apply(self, c: &mut Component) -> Result<()> {
                $(if let Some(value) = self.$field {
                    c.$field = value;
                } else if matches!(c.kind, $kinds) {
                    return Err(Error(format!("missing properties.{}", stringify!($field))));
                })+
                Ok(())
            }
        }
    }
}
properties!(
    facing: Facing => _, bits: u8 => _, label: String => _, font: ArtworkFont => _,
    color: [u8;3] => _, color_alpha: u8 => _, label_facing: Facing => _, label_center: bool => _,
    output: bool => Kind::Pin, pin: PinOptions => Kind::Pin,
    gate_options: GateOptions => Kind::Gate(_) | Kind::Parity(_),
    text_horizontal: HorizontalAlign => Kind::Text, text_vertical: VerticalAlign => Kind::Text,
    constant: u32 => Kind::Constant, radix: Radix => Kind::Pin | Kind::Probe,
    splitter: logisim_core::Splitter => Kind::Splitter, input_bits: u8 => Kind::BitExtender, extension: Extension => Kind::BitExtender,
    sequential: Sequential => Kind::FlipFlop(_) | Kind::Latch(_) | Kind::Register | Kind::Counter | Kind::ShiftRegister | Kind::Random | Kind::Clock,
    plexing: Plexing => Kind::Plexer(_), arithmetic: ArithmeticOptions => Kind::Arithmetic(_),
    io: Io => Kind::Button | Kind::Switch | Kind::DipSwitch | Kind::Slider | Kind::Joystick | Kind::Keyboard | Kind::Tty | Kind::Led | Kind::SevenSegment | Kind::HexDisplay | Kind::RgbLed | Kind::DotMatrix,
    terminal: TerminalOptions => Kind::Keyboard | Kind::Tty, bus: BusControl => Kind::ControlledBuffer | Kind::ControlledInverter | Kind::PullResistor,
    transistor: TransistorOptions => Kind::Transistor | Kind::TransmissionGate, parity: ParityOptions => Kind::Parity(_), latch_enable: LatchEnable => Kind::Latch(_),
);

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct MemoryData {
    address_bits: u8,
    interface: MemoryInterface,
    select_high: bool,
    simple: bool,
    /// Nonzero half-open runs [start, end, word], ordered and disjoint.
    contents: Vec<[u32; 3]>,
}

impl MemoryData {
    fn encode(m: &Memory) -> Self {
        let mut changes = m.contents.boundaries().peekable();
        let mut contents = vec![];
        while let Some((start, word)) = changes.next() {
            if let Some(&(end, _)) = changes.peek()
                && word != 0
            {
                contents.push([start, end, word]);
            }
        }
        Self {
            address_bits: m.address_bits,
            interface: m.interface,
            select_high: m.select_high,
            simple: m.simple,
            contents,
        }
    }
    fn decode(self, kind: Kind, bits: u8) -> Result<Memory> {
        if !kind.is_memory()
            || !(2..=24).contains(&self.address_bits)
            || !(1..=32).contains(&bits)
            || (kind == Kind::Ram && !self.contents.is_empty())
            || (kind == Kind::Rom && self.interface != MemoryInterface::SharedSync)
        {
            return Err(Error("Invalid memory attributes or RAM contents".into()));
        }
        let mut contents = MemoryContents::default();
        let mut cursor = 0;
        for [start, end, word] in self.contents {
            if start < cursor
                || end <= start
                || end > 1 << self.address_bits
                || word == 0
                || word > u32::MAX >> (32 - bits)
            {
                return Err(Error(format!("Invalid ROM contents run at {start}")));
            }
            contents.fill(start, end, word);
            cursor = end;
        }
        Ok(Memory {
            address_bits: self.address_bits,
            interface: self.interface,
            select_high: self.select_high,
            simple: self.simple,
            contents,
        })
    }
}
#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct PlaData {
    inputs: u8,
    terms: u8,
    outputs: u8,
    select_high: bool,
    input_plane: Vec<PlaInput>,
    output_plane: Vec<bool>,
}
impl PlaData {
    fn encode(p: &Pla) -> Self {
        let (inputs, terms, outputs) = p.dimensions();
        Self {
            inputs,
            terms,
            outputs,
            select_high: p.select_high,
            input_plane: (0..terms)
                .flat_map(|r| (0..inputs).map(move |c| p.input(r, c)))
                .collect(),
            output_plane: (0..terms)
                .flat_map(|r| (0..outputs).map(move |c| p.output(r, c)))
                .collect(),
        }
    }
    fn decode(self) -> Result<Pla> {
        if [self.inputs, self.terms, self.outputs]
            .iter()
            .any(|n| !(1..=32).contains(n))
            || self.input_plane.len() != self.inputs as usize * self.terms as usize
            || self.output_plane.len() != self.outputs as usize * self.terms as usize
        {
            return Err(Error("Invalid PLA dimensions or plane length".into()));
        }
        let mut p = Pla::new(self.inputs, self.terms, self.outputs);
        p.select_high = self.select_high;
        for row in 0..self.terms {
            for col in 0..self.inputs {
                p.set_input(
                    row,
                    col,
                    self.input_plane[row as usize * self.inputs as usize + col as usize],
                );
            }
            for col in 0..self.outputs {
                p.set_output(
                    row,
                    col,
                    self.output_plane[row as usize * self.outputs as usize + col as usize],
                );
            }
        }
        Ok(p)
    }
}
impl Part {
    pub fn encode(c: &Component, agent_id: String, names: &BTreeMap<Id, String>) -> Result<Self> {
        Ok(Self {
            agent_id,
            kind: PartKind::encode(c.kind, names)?,
            position: c.position,
            properties: Properties::encode(c),
            plugin: c.plugin.clone(),
            memory: c.kind.is_memory().then(|| MemoryData::encode(&c.memory)),
            pla: (c.kind == Kind::PlaRom).then(|| PlaData::encode(&c.pla)),
        })
    }
    pub fn decode(self, names: &BTreeMap<String, Id>) -> Result<Component> {
        let kind = self.kind.decode(names)?;
        let mut c = Component::new(kind, self.position);
        self.properties.apply(&mut c)?;
        c.plugin = self.plugin;
        if kind.is_memory() != self.memory.is_some() || (kind == Kind::PlaRom) != self.pla.is_some()
        {
            return Err(Error("Missing or unexpected memory/PLA field".into()));
        }
        if let Some(m) = self.memory {
            c.memory = m.decode(kind, c.bits)?;
        }
        if let Some(p) = self.pla {
            c.pla = p.decode()?;
        }
        Ok(c)
    }
}

pub(super) fn validate(c: &Component) -> Result<()> {
    if !valid_point(c.position)
        || !(1..=32).contains(&c.bits)
        || c.label.len() > 65536
        || c.font.size == 0
        || c.font.family.is_empty()
        || c.font.family.len() > 256
    {
        return Err(Error("Invalid position, width, label or font".into()));
    }
    if let Some(plugin) = &c.plugin {
        plugin.validate().map_err(Error)?;
        if serde_json::to_vec(plugin)
            .map_err(|e| Error(e.to_string()))?
            .len()
            > plugin::api::MAX_MESSAGE
        {
            return Err(Error("Plugin definition exceeds message limit".into()));
        }
    }
    // Reuse the electrical family validators. Native label/style fields are
    // independent of the legacy codec's ability to export them.
    let mut scratch = String::new();
    crate::gates::write(&mut scratch, c)?;
    crate::parity::write(&mut scratch, c)?;
    crate::latches::write(&mut scratch, c)?;
    crate::sequential::write(&mut scratch, c)?;
    crate::arithmetic::write(&mut scratch, c)?;
    crate::plexers::write(&mut scratch, c)?;
    crate::bus::write(&mut scratch, c)?;
    crate::memory::write(&mut scratch, c)?;
    crate::io::write(&mut scratch, c)?;
    if (c.kind == Kind::Constant && c.bits < 32 && c.constant >= 1 << c.bits)
        || (c.kind == Kind::BitExtender && !(1..=32).contains(&c.input_bits))
        || (c.kind == Kind::Splitter
            && (!(1..=32).contains(&c.splitter.fanout)
                || c.splitter.mapping[..c.bits as usize]
                    .iter()
                    .flatten()
                    .any(|&b| b >= c.splitter.fanout)))
    {
        return Err(Error(
            "Invalid constant, extender or splitter properties".into(),
        ));
    }
    Ok(())
}
