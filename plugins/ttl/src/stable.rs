use crate::model::Chip;
use logisim_plugin_api::*;
pub fn dependencies(tool: &Tool) -> StableContract {
    let chip = Chip::parse(&tool.id).expect("validated tool");
    StableContract {
        outputs: chip
            .kind
            .gates()
            .iter()
            .map(|&[a, b, o]| {
                let mut inputs = vec![chip.index(a) as u8];
                if a != b {
                    inputs.push(chip.index(b) as u8);
                }
                if chip.power {
                    inputs.extend([chip.index(7) as u8, chip.index(14) as u8]);
                }
                OutputDependency {
                    port: chip.index(o) as u8,
                    inputs,
                }
            })
            .collect(),
        graphics_inputs: vec![],
        state_inputs: None,
    }
}
