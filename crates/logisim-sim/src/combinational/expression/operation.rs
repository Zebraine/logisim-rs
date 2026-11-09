use super::super::Node;
use crate::{Bit, Value, arithmetic, bus, gates, memory, parity, pla, plexers};
use logisim_core::{Component, ControlledLogic, Extension, GateLogic, Kind, ParityLogic};

/// Snapshots reuse the engine's observed pure kernels, including their attributes.
/// Gate families keep only electrical configuration; other snapshots are cloned
/// at compile time, never per evaluation. No stateful kernel enters
/// this plan; in particular the output-holding display decoder is not a plexer here.
pub(super) enum Operation {
    Gate(GateLogic, Box<[bool]>),
    Parity(ParityLogic, Box<[bool]>),
    Arithmetic(Box<Component>),
    Plexer(Box<Component>),
    Rom(Box<Component>),
    Pla(Box<logisim_core::Pla>),
    Constant(Value),
    Controlled(ControlledLogic),
    Extend(u8, Extension),
}

impl Operation {
    pub(super) fn new(c: &Component, node: &Node) -> Self {
        match c.kind {
            Kind::Gate(gate) => Self::Gate(
                c.gate_logic().expect("matched component kind"),
                node.inputs
                    .iter()
                    .map(|i| gate.unary() || i.connected)
                    .collect(),
            ),
            Kind::Parity(_) => Self::Parity(
                c.parity_logic().expect("matched component kind"),
                node.inputs.iter().map(|i| i.connected).collect(),
            ),
            Kind::Arithmetic(_) => Self::Arithmetic(Box::new(c.clone())),
            Kind::Plexer(kind) if kind != logisim_core::Plexer::DisplayDecoder => {
                Self::Plexer(Box::new(c.clone()))
            }
            Kind::Rom => Self::Rom(Box::new(c.clone())),
            Kind::PlaRom => Self::Pla(Box::new(c.pla.clone())),
            Kind::Constant => Self::Constant(Value::known(c.bits, c.constant)),
            Kind::Power => Self::Constant(Value::filled(c.bits, Bit::One)),
            Kind::Ground => Self::Constant(Value::known(c.bits, 0)),
            Kind::ControlledBuffer | Kind::ControlledInverter => {
                Self::Controlled(c.controlled_logic().expect("matched component kind"))
            }
            Kind::BitExtender => Self::Extend(c.bits.clamp(1, 32), c.extension),
            _ => unreachable!("analysis admits only supported pure operations"),
        }
    }

    pub(super) fn evaluate(
        &self,
        ports: &[Value],
        optional: &mut Vec<Option<Value>>,
        mut emit: impl FnMut(usize, Value),
    ) {
        if let Self::Gate(_, present) | Self::Parity(_, present) = self {
            optional.clear();
            optional.extend(
                present
                    .iter()
                    .zip(&ports[1..])
                    .map(|(&p, &v)| p.then_some(v)),
            );
        }
        let output = match self {
            Self::Gate(config, _) => gates::evaluate(config, optional),
            Self::Parity(config, _) => parity::evaluate(config, optional),
            Self::Arithmetic(c) | Self::Plexer(c) => {
                let outputs = if matches!(self, Self::Arithmetic(_)) {
                    arithmetic::evaluate(c, ports)
                } else {
                    plexers::evaluate(c, ports)
                };
                for (index, value) in outputs {
                    emit(index, value);
                }
                return;
            }
            Self::Rom(c) => memory::rom(c, ports),
            Self::Pla(p) => pla::evaluate(p, ports[1], ports[2].bit(0)),
            Self::Constant(value) => *value,
            Self::Controlled(c) => bus::evaluate(c, ports[1], ports[2].bit(0)),
            Self::Extend(width, extension) => {
                let input = ports[1];
                let extra = match extension {
                    Extension::Zero => Bit::Zero,
                    Extension::One => Bit::One,
                    Extension::Sign => input.bit(input.width() - 1),
                    Extension::Input => ports[2].bit(0),
                };
                Value::from_bits((0..*width).map(|i| {
                    if i < input.width() {
                        input.bit(i)
                    } else {
                        extra
                    }
                }))
            }
        };
        emit(0, output);
    }
}
