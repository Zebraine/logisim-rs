use logisim_plugin_api::*;
use std::sync::OnceLock;
pub(super) fn manifest() -> &'static Manifest {
    static VALUE: OnceLock<Manifest> = OnceLock::new();
    VALUE.get_or_init(|| Manifest {
        abi: ABI_VERSION,
        id: "org.logisim-rs.display".into(),
        version: "1.0.0".into(),
        name: "Pixel displays".into(),
        tools: [16, 32]
            .into_iter()
            .map(|side| {
                let size = side * 8;
                let ports: Vec<_> = [
                    ("clock", "Clock", 1),
                    ("x", "X", 5),
                    ("y", "Y", 5),
                    ("color", "Color", 8),
                    ("write", "Write", 1),
                    ("clear", "Clear", 1),
                    ("present", "Present", 1),
                ]
                .into_iter()
                .enumerate()
                .map(|(i, (id, label, width))| Port {
                    id: id.into(),
                    label: label.into(),
                    at: [0, i as i32 * 20],
                    width,
                    output: false,
                })
                .collect();
                let mut graphics = vec![
                    Shape::Rect {
                        min: [0, -20],
                        max: [size + 60, size + 10],
                        fill: [248, 250, 252, 255],
                        stroke: [71, 85, 105, 255],
                    },
                    Shape::Rect {
                        min: [50, 0],
                        max: [size + 50, size],
                        fill: [0, 0, 0, 255],
                        stroke: [71, 85, 105, 255],
                    },
                ];
                for (port, label) in ports
                    .iter()
                    .zip(["CLK", "X", "Y", "RGB", "WE", "CLR", "PRES"])
                {
                    graphics.push(Shape::Text {
                        at: [6, port.at[1]],
                        text: label.into(),
                        size: 10,
                        color: [30, 41, 59, 255],
                    });
                }
                Tool {
                    id: format!("framebuffer{side}"),
                    name: format!("Framebuffer {side}×{side}"),
                    bounds: [[0, -20], [size + 60, size + 10]],
                    ports,
                    graphics,
                    properties: vec![Property {
                        id: "palette".into(),
                        label: "Palette".into(),
                        default: "RGB332".into(),
                        kind: PropertyKind::Choice {
                            values: vec!["RGB332".into(), "Grayscale".into()],
                        },
                    }],
                    actions: vec![],
                }
            })
            .collect(),
    })
}
pub(super) fn tool(id: &str) -> Result<(&'static Tool, usize), String> {
    manifest()
        .tools
        .iter()
        .enumerate()
        .find(|(_, t)| t.id == id)
        .map(|(i, t)| (t, if i == 0 { 16 } else { 32 }))
        .ok_or_else(|| "Unknown display tool.".into())
}
