use crate::model::{Chip, Kind};
use logisim_plugin_api::*;
pub fn dependencies(tool: &Tool) -> StableContract {
    let chip = Chip::parse(&tool.id).expect("validated tool");
    StableContract {
        outputs: chip
            .kind
            .outputs()
            .iter()
            .enumerate()
            .map(|(i, pin)| {
                let input_pins = match chip.kind {
                    Kind::Buffer74125 => &chip.kind.inputs()[i * 2..i * 2 + 2],
                    _ => chip.kind.inputs(),
                };
                let mut inputs: Vec<_> = input_pins.iter().map(|p| chip.index(*p) as u8).collect();
                if chip.power {
                    inputs.extend([
                        chip.index(chip.kind.count() / 2) as u8,
                        chip.index(chip.kind.count()) as u8,
                    ]);
                }
                OutputDependency {
                    port: chip.index(*pin) as u8,
                    inputs,
                }
            })
            .collect(),
        graphics_inputs: vec![],
        state_inputs: None,
    }
}
