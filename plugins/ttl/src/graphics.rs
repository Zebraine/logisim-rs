use crate::model::{Chip, Ttl};
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
    let fill = if internal {
        [248, 250, 253, 255]
    } else {
        [48, 60, 77, 255]
    };
    let ink = if internal { INK } else { [235, 241, 248, 255] };
    let mut shapes = vec![rect([0, -23], [140, 23], fill)];
    shapes.push(Shape::Ellipse {
        min: [-6, -6],
        max: [6, 6],
        fill,
        stroke: ink,
    });
    for pin in 1..=14 {
        let p = Ttl::pin_position(pin);
        let sign = p.y.signum();
        shapes.push(rect(
            [p.x - 3, (sign * 23).min(sign * 29)],
            [p.x + 3, (sign * 23).max(sign * 29)],
            [160, 170, 185, 255],
        ));
        shapes.push(text([p.x - 3, sign * 18 - 4], pin.to_string(), 7, ink));
    }
    if !internal {
        shapes.push(text([50, -8], chip.kind.name(), 14, ink));
        return shapes;
    }
    for &[a, b, o] in chip.kind.gates() {
        let a = Ttl::pin_position(a);
        let b = Ttl::pin_position(b);
        let out = Ttl::pin_position(o);
        let cx = (a.x.min(b.x).min(out.x) + a.x.max(b.x).max(out.x)) / 2;
        let cy = if out.y > 0 { 8 } else { -8 };
        let direction = if out.x > a.x { 1 } else { -1 };
        let input_x = cx - direction * 7;
        for (p, y) in if a == b {
            vec![(a, cy)]
        } else {
            vec![(a, cy - 3), (b, cy + 3)]
        } {
            shapes.push(line([p.x, p.y], [p.x, y]));
            shapes.push(line([p.x, y], [input_x, y]));
        }
        let inverted = matches!(
            chip.kind,
            Ttl::Ic7400 | Ttl::Ic7402 | Ttl::Ic7404 | Ttl::Ic747266
        );
        let output_x = cx + direction * if inverted { 11 } else { 7 };
        shapes.push(line([output_x, cy], [out.x, cy]));
        shapes.push(line([out.x, cy], [out.x, out.y]));
        shapes.push(rect([cx - 7, cy - 6], [cx + 7, cy + 6], fill));
        if inverted {
            shapes.push(Shape::Ellipse {
                min: [cx + direction * 9 - 2, cy - 2],
                max: [cx + direction * 9 + 2, cy + 2],
                fill,
                stroke: INK,
            });
        }
        let symbol = match chip.kind {
            Ttl::Ic7400 | Ttl::Ic7408 => "&",
            Ttl::Ic7402 | Ttl::Ic7432 => "≥1",
            Ttl::Ic7404 => "1",
            _ => "=1",
        };
        shapes.push(text([cx - 5, cy - 4], symbol, 7, ink));
    }
    shapes
}
pub fn manifest() -> &'static Manifest {
    static MANIFEST: OnceLock<Manifest> = OnceLock::new();
    MANIFEST.get_or_init(|| Manifest {
        abi: ABI_VERSION,
        id: "org.logisim-rs.ttl".into(),
        version: "1.0.0".into(),
        name: "TTL".into(),
        tools: Ttl::ALL
            .into_iter()
            .flat_map(|kind| {
                [false, true].map(|power| {
                    let chip = Chip { kind, power };
                    Tool {
                        id: chip.id(),
                        name: if power {
                            format!("{} · Vcc/GND", kind.name())
                        } else {
                            kind.name().into()
                        },
                        bounds: [[0, -30], [140, 30]],
                        ports: chip
                            .pins()
                            .iter()
                            .map(|&pin| {
                                let p = Ttl::pin_position(pin);
                                Port {
                                    id: format!("pin-{pin}"),
                                    label: format!(
                                        "{pin} · {}",
                                        match pin {
                                            7 => "GND",
                                            14 => "Vcc",
                                            _ if kind.output(pin) => "Output",
                                            _ => "Input",
                                        }
                                    ),
                                    at: [p.x, p.y],
                                    width: 1,
                                    output: kind.output(pin),
                                }
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
