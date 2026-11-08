use logisim_core::{Bit, Value};
use logisim_course::decode_instruction;
use logisim_plugin_api::*;
use std::sync::OnceLock;

pub fn probe(input: Value) -> Vec<Shape> {
    input.number().map_or_else(Vec::new, |word| {
        vec![text(
            [-60, -20],
            decode_instruction(word),
            9,
            if word == 0 {
                [200, 45, 55, 255]
            } else {
                [37, 99, 235, 255]
            },
        )]
    })
}
pub fn register_words(words: &[Value; 32]) -> Vec<Shape> {
    words
        .iter()
        .enumerate()
        .map(|(i, word)| {
            let value = word.number().map_or_else(
                || {
                    (0..8)
                        .rev()
                        .map(|n| {
                            let bits = (0..4).map(|b| word.bit(n * 4 + b)).collect::<Vec<_>>();
                            if bits.contains(&Bit::Error) {
                                'E'
                            } else if bits.contains(&Bit::Unknown) {
                                'X'
                            } else {
                                char::from_digit(
                                    bits.iter().enumerate().fold(0, |v, (i, b)| {
                                        v | if *b == Bit::One { 1 << i } else { 0 }
                                    }),
                                    16,
                                )
                                .unwrap()
                            }
                        })
                        .collect()
                },
                |v| format!("{v:08X}"),
            );
            text(
                [-126 + 76 * (i / 16) as i32, -84 + 10 * (i % 16) as i32],
                value,
                7,
                [51, 65, 85, 255],
            )
        })
        .collect()
}
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
pub fn manifest() -> &'static Manifest {
    static MANIFEST: OnceLock<Manifest> = OnceLock::new();
    MANIFEST.get_or_init(build_manifest)
}
fn build_manifest() -> Manifest {
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
