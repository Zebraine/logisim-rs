use logisim_plugin_api::*;
pub fn text(at: [i32; 2], text: impl Into<String>, size: u8, color: [u8; 4]) -> Shape {
    Shape::Text {
        at,
        text: text.into(),
        size,
        color,
    }
}
fn body(min: [i32; 2], max: [i32; 2]) -> Shape {
    Shape::Rect {
        min,
        max,
        fill: [237, 242, 249, 255],
        stroke: [51, 65, 85, 255],
    }
}
pub fn manifest() -> Manifest {
    let mut graphics = vec![body([-160, -90], [0, 90])];
    for i in 0..32 {
        graphics.push(text(
            [-146 + 76 * (i / 16), -84 + 10 * (i % 16)],
            format!("{i:02}"),
            7,
            [51, 65, 85, 255],
        ));
    }
    for (at, label) in [
        ([-157, -15], "W"),
        ([-157, 72], "CLK"),
        ([-12, -55], "A"),
        ([-12, 45], "B"),
        ([-125, 80], "WE"),
        ([-95, 80], "rW"),
        ([-55, 80], "rA"),
        ([-35, 80], "rB"),
    ] {
        graphics.push(text(at, label, 7, [51, 65, 85, 255]));
    }
    let ports = [
        ("a", "Read data A", [0, -50], 32, true),
        ("b", "Read data B", [0, 50], 32, true),
        ("w", "Write data", [-160, -10], 32, false),
        ("clock", "Clock", [-160, 80], 1, false),
        ("enable", "Write enable", [-120, 90], 1, false),
        ("write", "Write address", [-90, 90], 5, false),
        ("read-a", "Read address A", [-50, 90], 5, false),
        ("read-b", "Read address B", [-30, 90], 5, false),
    ]
    .into_iter()
    .map(|(id, label, at, width, output)| Port {
        id: id.into(),
        label: label.into(),
        at,
        width,
        output,
    })
    .collect();
    Manifest {
        abi: ABI_VERSION,
        id: "org.logisim-rs.course".into(),
        version: "1.0.0".into(),
        name: "Course (Wasm)".into(),
        tools: vec![
            Tool {
                id: "register-file".into(),
                name: "Register File (Wasm)".into(),
                bounds: [[-160, -90], [0, 90]],
                ports,
                graphics,
                properties: vec![Property {
                    id: "trigger".into(),
                    label: "Trigger".into(),
                    default: "rising".into(),
                    kind: PropertyKind::Choice {
                        values: ["rising", "falling", "high", "low"]
                            .map(String::from)
                            .to_vec(),
                    },
                }],
                actions: vec![Action {
                    id: "write".into(),
                    label: "Write register".into(),
                    argument: Some("Address (decimal), word (hex)".into()),
                }],
            },
            Tool {
                id: "riscv-probe".into(),
                name: "RISC-V Probe (Wasm)".into(),
                bounds: [[-65, -25], [65, 0]],
                ports: vec![Port {
                    id: "instruction".into(),
                    label: "Instruction".into(),
                    at: [0, 0],
                    width: 32,
                    output: false,
                }],
                graphics: vec![body([-65, -25], [65, 0])],
                properties: vec![],
                actions: vec![],
            },
        ],
    }
}
