use crate::model::{Chip, Kind};
use logisim_plugin_api::*;
use std::sync::OnceLock;
const INK: [u8; 4] = [51, 65, 85, 255];
fn text(at: [i32; 2], text: impl Into<String>, size: u8, color: [u8; 4]) -> Shape {
    Shape::Text {
        at,
        text: text.into(),
        size,
        color,
    }
}
fn rect(min: [i32; 2], max: [i32; 2], fill: [u8; 4]) -> Shape {
    Shape::Rect {
        min,
        max,
        fill,
        stroke: INK,
    }
}
fn line(a: [i32; 2], b: [i32; 2]) -> Shape {
    Shape::Line { a, b, color: INK }
}
pub fn drawing(chip: Chip, internal: bool) -> Vec<Shape> {
    let n = chip.kind.count();
    let width = i32::from(n) * 10;
    let fill = if internal {
        [248, 250, 253, 255]
    } else {
        [48, 60, 77, 255]
    };
    let ink = if internal { INK } else { [235, 241, 248, 255] };
    let mut shapes = vec![
        rect([0, -23], [width, 23], fill),
        Shape::Ellipse {
            min: [-6, -6],
            max: [6, 6],
            fill,
            stroke: ink,
        },
    ];
    for pin in 1..=n {
        let [x, y] = chip.kind.position(pin);
        let sign = y.signum();
        shapes.push(rect(
            [x - 3, (sign * 23).min(sign * 29)],
            [x + 3, (sign * 23).max(sign * 29)],
            [160, 170, 185, 255],
        ));
        let label = if internal && chip.kind != Kind::Buffer74125 {
            chip.kind.labels()[usize::from(pin - 1)].to_owned()
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
    if !internal || chip.kind != Kind::Buffer74125 {
        if internal {
            shapes.push(rect([8, -12], [width - 8, 12], fill));
        }
        shapes.push(text([width / 2 - 22, -8], chip.kind.name(), 14, ink));
    } else {
        for (pair, out) in chip
            .kind
            .inputs()
            .as_chunks::<2>()
            .0
            .iter()
            .zip(chip.kind.outputs())
        {
            let control = chip.kind.position(pair[0]);
            let data = chip.kind.position(pair[1]);
            let out = chip.kind.position(*out);
            let cx = (control[0] + out[0]) / 2;
            let cy = if out[1] > 0 { 7 } else { -7 };
            let side = out[1].signum();
            let data_y = cy + side * 10;
            let control_y = cy - side * 10;
            let bubble_y = cy - side * 5;
            shapes.extend([
                line(data, [data[0], data_y]),
                line([data[0], data_y], [cx - 12, data_y]),
                line([cx - 12, data_y], [cx - 12, cy]),
                line([cx - 12, cy], [cx - 8, cy]),
                line([cx - 8, cy - 6], [cx - 8, cy + 6]),
                line([cx - 8, cy - 6], [cx + 8, cy]),
                line([cx - 8, cy + 6], [cx + 8, cy]),
                line([cx + 8, cy], [out[0], cy]),
                line([out[0], cy], out),
                line(control, [control[0], control_y]),
                line([control[0], control_y], [cx, control_y]),
                Shape::Ellipse {
                    min: [cx - 2, bubble_y - 2],
                    max: [cx + 2, bubble_y + 2],
                    fill,
                    stroke: INK,
                },
                line([cx, control_y], [cx, bubble_y - side * 2]),
            ]);
        }
    }
    shapes
}
pub fn manifest() -> &'static Manifest {
    static MANIFEST: OnceLock<Manifest> = OnceLock::new();
    MANIFEST.get_or_init(|| Manifest {
        abi: ABI_VERSION,
        id: "org.logisim-rs.ttl-combinational".into(),
        version: "1.0.0".into(),
        name: "TTL · Combinational".into(),
        tools: Kind::ALL
            .into_iter()
            .flat_map(|kind| {
                [false, true].map(|power| {
                    let chip = Chip { kind, power };
                    Tool {
                        id: chip.id(),
                        name: format!("{}{}", kind.name(), if power { " · Vcc/GND" } else { "" }),
                        bounds: [[0, -30], [i32::from(kind.count()) * 10, 30]],
                        ports: chip
                            .pins()
                            .into_iter()
                            .map(|pin| Port {
                                id: format!("pin-{pin}"),
                                label: format!("{pin} · {}", kind.labels()[usize::from(pin - 1)]),
                                at: kind.position(pin),
                                width: 1,
                                output: kind.outputs().contains(&pin),
                            })
                            .collect(),
                        graphics: drawing(chip, false),
                        properties: vec![Property {
                            id: "internal".into(),
                            label: "Show internal structure".into(),
                            default: "false".into(),
                            kind: PropertyKind::Boolean,
                        }],
                        actions: vec![],
                    }
                })
            })
            .collect(),
    })
}
