use logisim_plugin_api::*;
use std::sync::OnceLock;
pub(crate) fn manifest() -> &'static Manifest {
    static VALUE: OnceLock<Manifest> = OnceLock::new();
    VALUE.get_or_init(|| {
        let mut properties: Vec<_> = [
            "ArrowUp",
            "ArrowDown",
            "ArrowLeft",
            "ArrowRight",
            "Space",
            "Enter",
            "Z",
            "X",
        ]
        .into_iter()
        .enumerate()
        .map(|(i, default)| Property {
            id: format!("key{i}"),
            label: format!("Channel {i} key"),
            default: default.into(),
            kind: PropertyKind::Choice {
                values: interaction::Key::ALL
                    .iter()
                    .map(|k| k.name().into())
                    .collect(),
            },
        })
        .collect();
        properties.extend([
            Property {
                id: "capacity".into(),
                label: "Queue capacity".into(),
                default: "16".into(),
                kind: PropertyKind::Integer { min: 1, max: 64 },
            },
            Property {
                id: "trigger".into(),
                label: "Read edge".into(),
                default: "rising".into(),
                kind: PropertyKind::Choice {
                    values: vec!["rising".into(), "falling".into()],
                },
            },
        ]);
        Manifest {
            abi: ABI_VERSION,
            id: "org.logisim-rs.input".into(),
            version: "1.0.0".into(),
            name: "Interactive input".into(),
            tools: vec![Tool {
                id: "keyboard".into(),
                name: "Key controller".into(),
                bounds: [[0, -20], [200, 100]],
                ports: [
                    ("clock", "Clock", [0, 0], 1, false),
                    ("read", "Read", [0, 30], 1, false),
                    ("clear", "Clear", [0, 60], 1, false),
                    ("held", "Held keys", [200, 0], 8, true),
                    ("channel", "Event channel", [200, 20], 3, true),
                    ("pressed", "Key down", [200, 40], 1, true),
                    ("valid", "Event available", [200, 60], 1, true),
                    ("overflow", "Overflow", [200, 80], 1, true),
                    ("count", "Queued events", [200, 100], 7, true),
                ]
                .into_iter()
                .map(|(id, label, at, width, output)| Port {
                    id: id.into(),
                    label: label.into(),
                    at,
                    width,
                    output,
                })
                .collect(),
                graphics: vec![Shape::Rect {
                    min: [0, -20],
                    max: [200, 100],
                    fill: [248, 250, 252, 255],
                    stroke: [71, 85, 105, 255],
                }],
                properties,
                actions: vec![Action {
                    id: "key".into(),
                    label: "Keyboard input".into(),
                    argument: Some("Input event".into()),
                }],
            }],
        }
    })
}
pub(crate) fn drawing(p: &Properties, state: &crate::state::State) -> Vec<Shape> {
    let held = state.mask(p);
    let mut graphics = vec![];
    for i in 0..8 {
        let x = 8 + (i % 4) * 46;
        let y = -10 + (i / 4) * 38;
        graphics.push(Shape::Rect {
            min: [x, y],
            max: [x + 42, y + 32],
            fill: if held & (1 << i) == 0 {
                [226, 232, 240, 255]
            } else {
                [134, 239, 172, 255]
            },
            stroke: [100, 116, 139, 255],
        });
        let key = &p[&format!("key{i}")];
        let label = match key.as_str() {
            "ArrowUp" => "↑",
            "ArrowDown" => "↓",
            "ArrowLeft" => "←",
            "ArrowRight" => "→",
            "Space" => "SPC",
            "Enter" => "ENT",
            _ => key,
        };
        graphics.push(Shape::Text {
            at: [x + 6, y + 3],
            text: format!("{i}:{label}"),
            size: 10,
            color: [30, 41, 59, 255],
        });
    }
    graphics.push(Shape::Text {
        at: [8, 72],
        text: format!(
            "FIFO {:02}/{}{}",
            state.queue.len(),
            p["capacity"],
            if state.overflow { " !" } else { "" }
        ),
        size: 12,
        color: [30, 41, 59, 255],
    });
    graphics
}
