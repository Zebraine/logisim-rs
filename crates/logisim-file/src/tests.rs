use super::*;
use logisim_core::{
    Component, Facing, Gate, GateSize, HorizontalAlign, Kind, Point, VerticalAlign,
};
fn xml(body: &str) -> String {
    format!(
        r##"<project source="2.16.2.2.jar" version="1.0"><lib name="w" desc="#Wiring"/><lib name="g" desc="#Gates"/><lib name="b" desc="#Base"/><main name="main"/><circuit name="main">{body}</circuit></project>"##
    )
}
#[test]
fn runtime_subset_defaults_and_library_ids() {
    let imported = parse(&xml(r#"<comp lib="g" name="AND Gate" loc="(340,190)"/><comp lib="w" name="Pin" loc="(200,150)"><a name="tristate" val="false"/></comp><comp lib="w" name="Pin" loc="(440,190)"><a name="output" val="true"/></comp><wire from="(200,150)" to="(310,150)"/><comp lib="w" name="Tunnel" loc="(-20,0)"/><comp lib="b" name="Text" loc="(662,192)"><a name="text">line 1
line 2 &amp; 线</a><a name="font" val="Dialog plain 24"/></comp>"#)).unwrap();
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    let c = &imported.document.circuit.components;
    assert_eq!(
        c[0].ports(),
        vec![
            Point::new(340, 190),
            Point::new(310, 180),
            Point::new(310, 200)
        ]
    );
    assert!(c[2].output);
    assert_eq!(c[3].label, "tunnel");
    assert_eq!(c[4].position, Point::new(662, 192));
    assert_eq!(c[4].label, "line 1\nline 2 & 线");
    assert_eq!(c[4].text_vertical, VerticalAlign::Baseline);
    assert_eq!(c[4].text_horizontal, HorizontalAlign::Center);
}
#[test]
fn full_supported_roundtrip_preserves_attributes_ports_and_wires() {
    let mut document = Document {
        name: "A & B \"主\"".into(),
        ..Document::default()
    };
    for (i, g) in Gate::ALL.into_iter().enumerate() {
        for facing in Facing::ALL {
            for size in [GateSize::Narrow, GateSize::Medium, GateSize::Wide] {
                if g.unary()
                    && (size == GateSize::Medium || (g == Gate::Buffer && size != GateSize::Narrow))
                {
                    continue;
                }
                let mut c = Component::new(Kind::Gate(g), Point::new(i as i32 * 100, 200));
                c.facing = facing;
                c.gate_size = size;
                c.bits = 32;
                c.inputs = 8;
                c.label = "<&\" > 线\nnext\tend".into();
                c.font_size = 24;
                c.color = [1, 2, 255];
                document.circuit.add_component(c);
            }
        }
    }
    for kind in [Kind::Text, Kind::Tunnel, Kind::Pin] {
        let mut c = Component::new(kind, Point::new(-133, 192));
        c.label = "line1\nline2 & 线".into();
        document.circuit.add_component(c);
    }
    document
        .circuit
        .add_wire(Point::new(-100, 0), Point::new(100, 0));
    document
        .circuit
        .add_wire(Point::new(0, -100), Point::new(0, 100));
    let saved = serialize(&document).unwrap();
    let imported = parse(&saved).unwrap();
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    assert_eq!(imported.document.name, document.name);
    for (a, b) in document
        .circuit
        .components
        .iter()
        .zip(&imported.document.circuit.components)
    {
        assert_eq!(a.ports(), b.ports());
        let mut a = a.clone();
        a.id = b.id;
        // Unary input count is not a persistent attribute.
        if matches!(a.kind,Kind::Gate(g) if g.unary()) {
            a.inputs = 2;
        }
        assert_eq!(&a, b);
    }
    assert_eq!(serialize(&imported.document).unwrap(), saved);
}
#[test]
fn unsupported_content_is_reported_without_becoming_a_supported_component() {
    let body = r#"<comp lib="w" name="Clock" loc="(0,0)"/><comp lib="g" name="AND Gate" loc="(100,0)"><a name="negated-unknown" val="true"/></comp><comp lib="w" name="Pin" loc="(10,0)"/><comp name="sub" loc="(20,0)"/><comp lib="g" name="OR Gate" loc="(100,100)"/>"#;
    let imported = parse(&xml(body)).unwrap();
    assert_eq!(imported.warnings.len(), 4);
    assert_eq!(imported.document.circuit.components.len(), 1);
    let saved = serialize(&imported.document).unwrap();
    assert!(!saved.contains("Clock"));
    assert!(!saved.contains("negated-unknown"));
}
#[test]
fn main_selection_and_omitted_project_content_are_explicit() {
    let s = xml("<a name=\"clabel\">shared\nlabel</a>").replace(
        "</project>",
        "<circuit name=\"other\"/><message>data</message></project>",
    );
    let imported = parse(&s).unwrap();
    assert_eq!(imported.warnings.len(), 3);
    assert!(parse(&s.replace("<main name=\"main\"/>", "<main name=\"missing\"/>")).is_err());
}
#[test]
fn rejects_malformed_dtd_duplicate_and_invalid_wire_inputs() {
    for s in [
        "not XML".into(),
        "<!DOCTYPE project [<!ENTITY x 'x'>]><project version=\"1.0\"/>".into(),
        xml("<wire from=\"(0,0)\" to=\"(1,1)\"/>"),
        xml("<wire from=\"(0,0)\" to=\"(99999999,0)\"/>"),
        xml("").replace("</project>", "<circuit name=\"main\"/></project>"),
        xml("").replace("</project>", "<lib name=\"g\" desc=\"#Base\"/></project>"),
    ] {
        assert!(parse(&s).is_err(), "{s}");
    }
    assert!(parse(&" ".repeat(MAX_BYTES + 1)).is_err());
    let mut d = Document::default();
    let mut c = Component::new(Kind::Text, Point::ZERO);
    c.label = "bad\u{0}".into();
    d.circuit.add_component(c);
    assert!(serialize(&d).is_err());
}
#[test]
fn measured_gate_matrix_has_exact_ports_and_rotations() {
    for size in [GateSize::Narrow, GateSize::Medium, GateSize::Wide] {
        for n in 2..=8 {
            let expected: &[i32] = match (size, n) {
                (GateSize::Medium | GateSize::Wide, 2) => &[-20, 20],
                (GateSize::Medium, 3) => &[-20, 0, 20],
                (GateSize::Wide, 3) => &[-30, 0, 30],
                (GateSize::Wide, 4) => &[-30, -10, 10, 30],
                (_, 2) => &[-10, 10],
                (_, 3) => &[-10, 0, 10],
                (_, 4) => &[-20, -10, 10, 20],
                (_, 5) => &[-20, -10, 0, 10, 20],
                (_, 6) => &[-30, -20, -10, 10, 20, 30],
                (_, 7) => &[-30, -20, -10, 0, 10, 20, 30],
                _ => &[-40, -30, -20, -10, 10, 20, 30, 40],
            };
            for g in [
                Gate::And,
                Gate::Or,
                Gate::Nand,
                Gate::Nor,
                Gate::Xor,
                Gate::Xnor,
            ] {
                let width = match size {
                    GateSize::Narrow => 30,
                    GateSize::Medium => 50,
                    GateSize::Wide => 70,
                };
                let depth = width
                    + match g {
                        Gate::Nand | Gate::Nor | Gate::Xor => 10,
                        Gate::Xnor => 20,
                        _ => 0,
                    };
                let mut c = Component::new(Kind::Gate(g), Point::ZERO);
                c.gate_size = size;
                c.inputs = n;
                for facing in Facing::ALL {
                    c.facing = facing;
                    assert_eq!(
                        c.ports()[1..],
                        expected
                            .iter()
                            .map(|y| facing.transform(Point::new(-depth, *y)))
                            .collect::<Vec<_>>()
                    );
                }
            }
        }
    }
}
