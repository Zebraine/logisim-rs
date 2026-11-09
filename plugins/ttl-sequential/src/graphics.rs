use crate::{
    model::{Chip, LABELS, OUTPUTS, position},
    state::State,
};
use logisim_core::Bit;
use logisim_plugin_api::*;
use std::sync::OnceLock;
const INK: [u8; 4] = [51, 65, 85, 255];
fn text(at: [i32; 2], value: impl Into<String>, size: u8, color: [u8; 4]) -> Shape {
    Shape::Text {
        at,
        text: value.into(),
        size,
        color,
    }
}
pub fn drawing(state: Option<&State>) -> Vec<Shape> {
    let internal = state.is_some();
    let fill = if internal {
        [248, 250, 253, 255]
    } else {
        [48, 60, 77, 255]
    };
    let ink = if internal { INK } else { [235, 241, 248, 255] };
    let mut shapes = vec![
        Shape::Rect {
            min: [0, -23],
            max: [160, 23],
            fill,
            stroke: INK,
        },
        Shape::Ellipse {
            min: [-6, -6],
            max: [6, 6],
            fill,
            stroke: ink,
        },
    ];
    for pin in 1..=16 {
        let [x, y] = position(pin);
        let sign = y.signum();
        shapes.push(Shape::Rect {
            min: [x - 3, (sign * 23).min(sign * 29)],
            max: [x + 3, (sign * 23).max(sign * 29)],
            fill: [160, 170, 185, 255],
            stroke: INK,
        });
        let label = if internal {
            LABELS[usize::from(pin - 1)].to_owned()
        } else {
            pin.to_string()
        };
        shapes.push(text(
            [x - label.len() as i32 * 2, if y > 0 { 14 } else { -22 }],
            label,
            7,
            ink,
        ));
    }
    if let Some(state) = state {
        let bits: String = state
            .bits
            .iter()
            .map(|v| match v {
                Bit::Zero => '0',
                Bit::One => '1',
                Bit::Unknown => 'X',
                Bit::Error => 'E',
            })
            .collect();
        shapes.push(text([42, -8], bits, 14, ink));
    } else {
        shapes.push(text([58, -8], "74165", 14, ink));
    }
    shapes
}
pub fn manifest() -> &'static Manifest {
    static MANIFEST: OnceLock<Manifest> = OnceLock::new();
    MANIFEST.get_or_init(|| Manifest {
        abi: ABI_VERSION,
        id: "org.logisim-rs.ttl-sequential".into(),
        version: "1.0.0".into(),
        name: "TTL · Sequential".into(),
        tools: [false, true]
            .into_iter()
            .map(|power| {
                let chip = Chip { power };
                Tool {
                    id: chip.id().into(),
                    name: if power { "74165 · Vcc/GND" } else { "74165" }.into(),
                    bounds: [[0, -30], [160, 30]],
                    ports: chip
                        .pins()
                        .into_iter()
                        .map(|pin| Port {
                            id: format!("pin-{pin}"),
                            label: format!("{pin} · {}", LABELS[usize::from(pin - 1)]),
                            at: position(pin),
                            width: 1,
                            output: OUTPUTS.contains(&pin),
                        })
                        .collect(),
                    graphics: drawing(None),
                    properties: vec![Property {
                        id: "internal".into(),
                        label: "Show internal structure".into(),
                        default: "false".into(),
                        kind: PropertyKind::Boolean,
                    }],
                    actions: vec![],
                }
            })
            .collect(),
    })
}
