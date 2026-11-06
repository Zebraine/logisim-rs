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
    let c = &imported.document.main_circuit().circuit.components;
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
    let mut document = Document::single("A & B \"主\"", logisim_core::Circuit::default());
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
                document.main_circuit_mut().circuit.add_component(c);
            }
        }
    }
    for kind in [Kind::Text, Kind::Tunnel, Kind::Pin] {
        let mut c = Component::new(kind, Point::new(-133, 192));
        c.label = "line1\nline2 & 线".into();
        document.main_circuit_mut().circuit.add_component(c);
    }
    document
        .main_circuit_mut()
        .circuit
        .add_wire(Point::new(-100, 0), Point::new(100, 0));
    document
        .main_circuit_mut()
        .circuit
        .add_wire(Point::new(0, -100), Point::new(0, 100));
    let saved = serialize(&document).unwrap();
    let imported = parse(&saved).unwrap();
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    assert_eq!(
        imported.document.main_circuit().name,
        document.main_circuit().name
    );
    for (a, b) in document
        .main_circuit_mut()
        .circuit
        .components
        .iter()
        .zip(&imported.document.main_circuit().circuit.components)
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
    let body = r#"<comp lib="w" name="Transistor" loc="(0,0)"/><comp lib="g" name="AND Gate" loc="(100,0)"><a name="negated-unknown" val="true"/></comp><comp lib="w" name="Pin" loc="(10,0)"/><comp name="sub" loc="(20,0)"/><comp lib="g" name="OR Gate" loc="(100,100)"/>"#;
    let imported = parse(&xml(body)).unwrap();
    assert_eq!(imported.warnings.len(), 5);
    assert_eq!(imported.document.main_circuit().circuit.components.len(), 1);
    let saved = serialize(&imported.document).unwrap();
    assert!(!saved.contains("Transistor"));
    assert!(!saved.contains("negated-unknown"));
}

#[test]
fn sequential_runtime_defaults_and_options_roundtrip_with_exact_terminals() {
    use logisim_core::{ControlPosition, FlipFlop, Trigger};
    let input=xml(r#"<comp lib="w" name="Clock" loc="(100,100)"/><comp lib="m" name="Register" loc="(300,100)"/><comp lib="m" name="D Flip-Flop" loc="(500,100)"/>"#).replace("<main", "<lib name=\"m\" desc=\"#Memory\"/><main");
    let imported = parse(&input).unwrap();
    assert!(imported.warnings.is_empty());
    let components = &imported.document.main_circuit().circuit.components;
    assert_eq!(components[0].label_facing, Facing::West);
    assert_eq!(components[1].bits, 8);
    assert_eq!(components[2].ports()[2], Point::new(440, 100));
    let mut document = Document::default();
    for facing in Facing::ALL {
        let mut c = Component::new(Kind::Clock, Point::new(100, 100));
        c.facing = facing;
        c.label_facing = Facing::South;
        c.sequential.high_ticks = 2;
        c.sequential.low_ticks = 3;
        c.label = "时钟 & clock".into();
        document.main_circuit_mut().circuit.add_component(c);
    }
    for trigger in Trigger::ALL {
        for bits in [1, 4, 8, 32] {
            let mut c = Component::new(Kind::Register, Point::new(300, 100));
            c.bits = bits;
            c.sequential.trigger = trigger;
            c.sequential.select_high = true;
            document.main_circuit_mut().circuit.add_component(c);
        }
    }
    for ff in FlipFlop::ALL {
        for new_layout in [true, false] {
            for controls in ControlPosition::ALL {
                if !new_layout && controls != ControlPosition::Legacy {
                    continue;
                }
                for trigger in [Trigger::Rising, Trigger::Falling] {
                    let mut c = Component::new(Kind::FlipFlop(ff), Point::new(500, 100));
                    c.sequential.trigger = trigger;
                    c.sequential.new_layout = new_layout;
                    c.sequential.controls = controls;
                    c.sequential.negate_controls = true;
                    document.main_circuit_mut().circuit.add_component(c);
                }
            }
        }
    }
    let saved = serialize(&document).unwrap();
    let roundtrip = parse(&saved).unwrap();
    assert!(roundtrip.warnings.is_empty(), "{:?}", roundtrip.warnings);
    for (before, after) in document
        .main_circuit()
        .circuit
        .components
        .iter()
        .zip(&roundtrip.document.main_circuit().circuit.components)
    {
        let mut before = before.clone();
        before.id = after.id;
        assert_eq!(before.ports(), after.ports());
        assert_eq!(&before, after);
    }
    assert_eq!(serialize(&roundtrip.document).unwrap(), saved);
}

#[test]
fn unknown_sequential_attributes_are_omitted_with_source_protection_warnings() {
    let input=xml(r#"<comp lib="m" name="D Flip-Flop" loc="(0,0)"><a name="trigger" val="high"/></comp><comp lib="w" name="Clock" loc="(100,0)"><a name="highDuration" val="0"/></comp><comp lib="m" name="Register" loc="(200,0)"><a name="initialValue" val="123"/></comp><comp lib="m" name="Counter" loc="(300,0)"/>"#).replace("<main","<lib name=\"m\" desc=\"#Memory\"/><main");
    let imported = parse(&input).unwrap();
    assert_eq!(imported.warnings.len(), 4);
    assert!(
        imported
            .document
            .main_circuit()
            .circuit
            .components
            .is_empty()
    );
    let saved = serialize(&imported.document).unwrap();
    assert!(!saved.contains("<comp "));
}
#[test]
fn main_selection_and_omitted_project_content_are_explicit() {
    let s = xml("<a name=\"clabel\">shared\nlabel</a>").replace(
        "</project>",
        "<circuit name=\"other\"/><message>data</message></project>",
    );
    let imported = parse(&s).unwrap();
    assert_eq!(imported.warnings.len(), 2);
    assert_eq!(imported.document.circuits.len(), 2);
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
    d.main_circuit_mut().circuit.add_component(c);
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

#[test]
fn wiring_defaults_and_runtime_export_attributes_are_supported() {
    let imported = parse(&xml(r#"
        <comp lib="w" name="Constant" loc="(10,10)"/>
        <comp lib="w" name="Power" loc="(20,10)"/>
        <comp lib="w" name="Ground" loc="(30,10)"/>
        <comp lib="w" name="Probe" loc="(40,10)"><a name="radix" val="10signed"/><a name="labelloc" val="south"/></comp>
        <comp lib="w" name="Bit Extender" loc="(100,100)"/>
        <comp lib="w" name="Splitter" loc="(200,200)"/>
        <comp lib="w" name="Splitter" loc="(300,200)"><a name="fanout" val="3"/><a name="incoming" val="8"/><a name="appear" val="legacy"/><a name="bit0" val="none"/><a name="bit1" val="0"/><a name="bit2" val="0"/><a name="bit3" val="1"/><a name="bit4" val="1"/><a name="bit5" val="1"/><a name="bit6" val="2"/><a name="bit7" val="2"/></comp>
    "#)).unwrap();
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    let c = &imported.document.main_circuit().circuit.components;
    assert_eq!(c[0].constant, 1);
    assert_eq!(c[1].facing, Facing::North);
    assert_eq!(c[2].facing, Facing::South);
    assert_eq!(c[3].radix, logisim_core::Radix::Signed);
    assert_eq!(c[3].label_facing, Facing::South);
    assert_eq!((c[4].input_bits, c[4].bits), (8, 16));
    assert_eq!(
        c[4].ports(),
        vec![Point::new(100, 100), Point::new(60, 100)]
    );
    assert_eq!(
        c[5].ports(),
        vec![
            Point::new(200, 200),
            Point::new(220, 160),
            Point::new(220, 170),
            Point::new(220, 180),
            Point::new(220, 190)
        ]
    );
    assert_eq!(
        &c[6].splitter.mapping[..8],
        &[
            None,
            Some(0),
            Some(0),
            Some(1),
            Some(1),
            Some(1),
            Some(2),
            Some(2)
        ]
    );
}
#[test]
fn all_wiring_attributes_roundtrip_without_omissions() {
    use logisim_core::{Extension, Radix, SplitAppearance};
    let mut d = Document::default();
    for (index, kind) in [
        Kind::Constant,
        Kind::Power,
        Kind::Ground,
        Kind::Probe,
        Kind::Splitter,
        Kind::BitExtender,
    ]
    .into_iter()
    .enumerate()
    {
        let mut c = Component::new(kind, Point::new(index as i32 * 100, 200));
        match kind {
            Kind::Constant | Kind::Power | Kind::Ground => {
                c.bits = 32;
                c.constant = if kind == Kind::Constant { u32::MAX } else { 1 };
                c.facing = Facing::West;
            }
            Kind::Probe => {
                c.radix = Radix::Octal;
                c.label = "Probe & 总线".into();
                c.label_facing = Facing::East;
                c.font_size = 18;
            }
            Kind::Splitter => {
                c.bits = 32;
                c.facing = Facing::North;
                c.splitter.fanout = 32;
                c.splitter.appearance = SplitAppearance::Center;
                c.splitter.distribute(32, true);
                c.splitter.mapping[0] = None;
            }
            Kind::BitExtender => {
                c.input_bits = 32;
                c.bits = 8;
                c.extension = Extension::Input;
            }
            _ => unreachable!(),
        }
        d.main_circuit_mut().circuit.add_component(c);
    }
    let saved = serialize(&d).unwrap();
    let imported = parse(&saved).unwrap();
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    assert_eq!(d, imported.document);
    assert_eq!(serialize(&imported.document).unwrap(), saved);
}
#[test]
fn invalid_wiring_attributes_are_reported_and_never_exported_as_supported() {
    let imported = parse(&xml(r#"
        <comp lib="w" name="Splitter" loc="(0,0)"><a name="bit0" val="4"/></comp>
        <comp lib="w" name="Splitter" loc="(0,10)"><a name="incoming" val="33"/></comp>
        <comp lib="w" name="Splitter" loc="(0,20)"><a name="bit31" val="0"/></comp>
        <comp lib="w" name="Bit Extender" loc="(0,30)"><a name="type" val="unknown"/></comp>
        <comp lib="w" name="Bit Extender" loc="(0,40)"><a name="facing" val="south"/></comp>
        <comp lib="w" name="Probe" loc="(0,50)"><a name="radix" val="unknown"/></comp>
        <comp lib="w" name="Constant" loc="(0,60)"><a name="value" val="0x100000000"/></comp>
    "#))
    .unwrap();
    assert_eq!(imported.warnings.len(), 7);
    assert!(
        imported
            .document
            .main_circuit()
            .circuit
            .components
            .is_empty()
    );
}

#[test]
fn multiple_circuits_forward_references_and_instance_attributes_roundtrip() {
    let source = r##"<project source="2.16.2.2.jar" version="1.0"><lib name="0" desc="#Wiring"/><main name="top"/>
    <circuit name="top"><comp name="part" loc="(200,100)"><a name="facing" val="south"/><a name="label" val="unit"/><a name="labelloc" val="west"/></comp><wire from="(200,70)" to="(200,40)"/></circuit>
    <circuit name="part"><comp lib="0" name="Pin" loc="(100,100)"><a name="tristate" val="false"/><a name="width" val="4"/></comp><comp lib="0" name="Pin" loc="(200,100)"><a name="output" val="true"/><a name="facing" val="west"/><a name="width" val="4"/></comp><wire from="(100,100)" to="(200,100)"/></circuit></project>"##;
    let imported = parse(source).unwrap();
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    let c = &imported.document.main_circuit().circuit.components[0];
    assert_eq!(c.interface.name, "part");
    assert_eq!(c.ports(), vec![Point::new(200, 70), Point::new(200, 100)]);
    assert_eq!(c.port_width(1), Some(4));
    assert_eq!(c.label_facing, Facing::West);
    let serialized = serialize(&imported.document).unwrap();
    assert!(serialized.contains("<comp loc=\"(200,100)\" name=\"part\">"));
    let reloaded = parse(&serialized).unwrap();
    assert!(reloaded.warnings.is_empty());
    assert_eq!(serialize(&reloaded.document).unwrap(), serialized);
    let mut renamed = imported.document;
    let target = renamed.named("part").unwrap().id;
    renamed.rename(target, "new & part".into()).unwrap();
    renamed.main = target;
    let read = parse(&serialize(&renamed).unwrap()).unwrap();
    assert_eq!(read.document.main_circuit().name, "new & part");
    assert_eq!(read.document.circuits.len(), 2);
}
#[test]
fn recursive_missing_and_custom_interfaces_never_silently_change_coordinates() {
    let source = xml("<comp name=\"sub\" loc=\"(100,100)\"/>").replace(
        "</project>",
        "<circuit name=\"sub\"><comp name=\"main\" loc=\"(0,0)\"/></circuit></project>",
    );
    assert!(parse(&source).unwrap_err().0.contains("recursive"));
    let source = source.replace(
        "<comp name=\"main\" loc=\"(0,0)\"/>",
        "<appear><rect x=\"0\" y=\"0\" width=\"80\" height=\"40\"/></appear>",
    );
    let imported = parse(&source).unwrap();
    assert!(!imported.warnings.is_empty());
    assert!(
        imported
            .document
            .main_circuit()
            .circuit
            .components
            .is_empty()
    );
    assert!(!serialize(&imported.document).unwrap().contains("<appear>"));
    let source = source.replace(
        "<appear><rect x=\"0\" y=\"0\" width=\"80\" height=\"40\"/></appear>",
        "<comp lib=\"w\" name=\"Pin\" loc=\"(0,0)\"/>",
    );
    let imported = parse(&source).unwrap();
    assert!(
        imported
            .document
            .main_circuit()
            .circuit
            .components
            .is_empty()
    );
    assert!(
        imported
            .warnings
            .iter()
            .any(|w| w.contains("interface pins"))
    );
}

fn appearance_project(art: &str) -> String {
    format!(
        r##"<project source="2.16.2.2.jar" version="1.0"><lib name="0" desc="#Wiring"/><main name="top"/>
    <circuit name="top"><comp name="part" loc="(300,200)"><a name="facing" val="east"/></comp></circuit>
    <circuit name="part"><appear>{art}
      <circ-port height="8" pin="100,100" width="8" x="46" y="56"/>
      <circ-port height="10" pin="250,100" width="10" x="75" y="55"/>
      <circ-anchor facing="north" height="6" width="6" x="97" y="67"/>
    </appear><comp lib="0" name="Pin" loc="(100,100)"><a name="width" val="4"/><a name="tristate" val="false"/></comp>
    <comp lib="0" name="Pin" loc="(250,100)"><a name="width" val="4"/><a name="output" val="true"/><a name="facing" val="west"/></comp></circuit></project>"##
    )
}
#[test]
fn custom_vector_artwork_and_styled_text_roundtrip_with_exact_pin_bindings() {
    let source = appearance_project(
        r##"
      <rect fill="#ffffff" height="30" rx="5" ry="5" stroke="#000000" stroke-width="2" width="30" x="50" y="50"/>
      <path d="M61,52 Q65,61 69,52" fill="none" stroke="#808080" stroke-width="2"/>
      <rect height="20" stroke="#000000" width="20" x="70" y="100"/>
      <ellipse cx="130.5" cy="130.0" fill="none" rx="20.5" ry="10.0" stroke="#000000"/>
      <polyline fill="none" points="170,90 190,100 210,90" stroke="#000000"/>
      <polygon fill="none" points="170,150 200,160 170,170" stroke="#000000"/>
      <text font-family="Dialog" font-size="18" font-style="italic" font-weight="bold" text-anchor="middle" x="100" y="201">A &amp; B</text>"##,
    );
    let imported = parse(&source).unwrap();
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    let instance = &imported.document.main_circuit().circuit.components[0];
    assert_eq!(
        instance.ports(),
        vec![Point::new(310, 150), Point::new(310, 180)]
    );
    assert_eq!(instance.port_width(0), Some(4));
    let a = imported
        .document
        .named("part")
        .unwrap()
        .circuit
        .appearance
        .as_ref()
        .unwrap();
    assert_eq!(a.shapes.len(), 7);
    assert_eq!(a.shapes[2].paint.fill, Some([0; 3]));
    let serialized = serialize(&imported.document).unwrap();
    let reloaded = parse(&serialized).unwrap();
    assert!(reloaded.warnings.is_empty());
    assert_eq!(serialize(&reloaded.document).unwrap(), serialized);
    let mut moved = imported.document;
    let part = moved.named("part").unwrap().id;
    let c = &mut moved.circuit_mut(part).unwrap().circuit;
    let pin = c.components[0].id;
    let mut p = c.component(pin).unwrap().clone();
    p.position = Point::new(330, 140);
    c.replace_component(p);
    moved.refresh_interfaces(true);
    let saved = serialize(&moved).unwrap();
    assert!(saved.contains("pin=\"330,140\""));
    assert_eq!(
        parse(&saved)
            .unwrap()
            .document
            .main_circuit()
            .circuit
            .components[0]
            .ports(),
        vec![Point::new(310, 150), Point::new(310, 180)]
    );
}
#[test]
fn unknown_or_invalid_appearance_is_reported_and_never_replaced_with_default_instances() {
    for art in [
        r#"<image href="file:///tmp/image.png"/>"#,
        r#"<path d="M0,0 L10,10"/>"#,
        r#"<rect x="0" y="0" width="50" height="30" transform="rotate(90)"/>"#,
        r#"<ellipse cx="NaN" cy="5" rx="3" ry="4"/>"#,
        r#"<circ-port pin="100,100" x="0" y="0" width="8" height="8"/>"#,
        r#"<circ-anchor facing="west" width="6" height="6" x="7" y="7"/>"#,
    ] {
        let read = parse(&appearance_project(art)).unwrap();
        assert!(!read.warnings.is_empty());
        assert!(read.document.main_circuit().circuit.components.is_empty());
        assert!(!serialize(&read.document).unwrap().contains("<appear>"));
    }
    let bad = appearance_project("").replace("pin=\"100,100\"", "pin=\"100,110\"");
    assert!(!parse(&bad).unwrap().warnings.is_empty());
}

#[test]
fn custom_appearance_save_rejects_ambiguous_internal_pin_positions() {
    let mut p = parse(&appearance_project("")).unwrap().document;
    let child = p.named("part").unwrap().id;
    let c = &mut p.circuit_mut(child).unwrap().circuit;
    c.components[1].position = c.components[0].position;
    assert!(
        serialize(&p)
            .unwrap_err()
            .0
            .contains("unique internal positions")
    );
}
