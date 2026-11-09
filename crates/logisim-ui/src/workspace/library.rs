//! UI tool order and stable, untranslated group identifiers. Domain families supply their variants.
use logisim_core::{Arithmetic, FlipFlop, Gate, Kind, Latch, Parity, Plexer};

pub(super) struct ToolGroup {
    pub key: &'static str,
    pub tools: Vec<Kind>,
}
pub(super) fn groups() -> &'static [ToolGroup] {
    static GROUPS: std::sync::OnceLock<[ToolGroup; 7]> = std::sync::OnceLock::new();
    GROUPS.get_or_init(|| {
        [
            ToolGroup {
                key: "Wiring",
                tools: vec![
                    Kind::Splitter,
                    Kind::Pin,
                    Kind::Probe,
                    Kind::Tunnel,
                    Kind::PullResistor,
                    Kind::Clock,
                    Kind::Constant,
                    Kind::Power,
                    Kind::Ground,
                    Kind::Transistor,
                    Kind::TransmissionGate,
                    Kind::BitExtender,
                ],
            },
            ToolGroup {
                key: "Gates",
                tools: Gate::ALL
                    .into_iter()
                    .map(Kind::Gate)
                    .chain(Parity::ALL.into_iter().map(Kind::Parity))
                    .chain([Kind::ControlledBuffer, Kind::ControlledInverter])
                    .collect(),
            },
            ToolGroup {
                key: "Plexers",
                tools: Plexer::ALL.into_iter().map(Kind::Plexer).collect(),
            },
            ToolGroup {
                key: "Arithmetic",
                tools: Arithmetic::ALL.into_iter().map(Kind::Arithmetic).collect(),
            },
            ToolGroup {
                key: "Memory",
                tools: Latch::ALL
                    .into_iter()
                    .map(Kind::Latch)
                    .chain(FlipFlop::ALL.into_iter().map(Kind::FlipFlop))
                    .chain([
                        Kind::Register,
                        Kind::Counter,
                        Kind::ShiftRegister,
                        Kind::Random,
                        Kind::Rom,
                        Kind::PlaRom,
                        Kind::Ram,
                    ])
                    .collect(),
            },
            ToolGroup {
                key: "Input/Output",
                tools: vec![
                    Kind::Button,
                    Kind::Switch,
                    Kind::DipSwitch,
                    Kind::Slider,
                    Kind::Joystick,
                    Kind::Keyboard,
                    Kind::Tty,
                    Kind::Led,
                    Kind::RgbLed,
                    Kind::SevenSegment,
                    Kind::HexDisplay,
                    Kind::DotMatrix,
                ],
            },
            ToolGroup {
                key: "Base",
                tools: vec![Kind::Text],
            },
        ]
    })
}

#[cfg(test)]
mod tests;
