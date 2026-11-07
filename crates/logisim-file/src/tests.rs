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
    let body = r#"<comp lib="w" name="Unknown Wiring Component" loc="(0,0)"/><comp lib="g" name="AND Gate" loc="(100,0)"><a name="negated-unknown" val="true"/></comp><comp lib="w" name="Pin" loc="(10,0)"><a name="pull" val="invalid"/></comp><comp name="sub" loc="(20,0)"/><comp lib="g" name="OR Gate" loc="(100,100)"/>"#;
    let imported = parse(&xml(body)).unwrap();
    assert_eq!(imported.warnings.len(), 5);
    assert_eq!(imported.document.main_circuit().circuit.components.len(), 1);
    let saved = serialize(&imported.document).unwrap();
    assert!(!saved.contains("Unknown Wiring Component"));
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
    let input=xml(r#"<comp lib="m" name="D Flip-Flop" loc="(0,0)"><a name="trigger" val="high"/></comp><comp lib="w" name="Clock" loc="(100,0)"><a name="highDuration" val="0"/></comp><comp lib="m" name="Register" loc="(200,0)"><a name="initialValue" val="123"/></comp><comp lib="m" name="RAM" loc="(300,0)"><a name="initialValue" val="123"/></comp>"#).replace("<main","<lib name=\"m\" desc=\"#Memory\"/><main");
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
        "<comp lib=\"w\" name=\"Pin\" loc=\"(0,0)\"><a name=\"pull\" val=\"invalid\"/></comp>",
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

#[test]
fn wire_import_merges_after_all_terminals_and_preserves_junctions_in_saved_geometry() {
    let wires = r#"<wire from="(0,0)" to="(50,0)"/><wire from="(50,0)" to="(100,0)"/><wire from="(20,0)" to="(70,0)"/><wire from="(50,-50)" to="(50,50)"/><wire from="(200,0)" to="(250,0)"/><wire from="(250,0)" to="(300,0)"/>"#;
    let pin = r#"<comp lib="w" name="Pin" loc="(250,0)"><a name="tristate" val="false"/></comp>"#;
    let mut shapes = vec![];
    for body in [format!("{wires}{pin}"), format!("{pin}{wires}")] {
        let imported = parse(&xml(&body)).unwrap();
        assert!(imported.warnings.is_empty());
        let c = &imported.document.main_circuit().circuit;
        assert_eq!(c.wires.len(), 6);
        assert_eq!(c.junctions(), vec![Point::new(50, 0), Point::new(250, 0)]);
        let mut geometry: Vec<_> = c.wires.iter().map(|w| (w.a, w.b)).collect();
        geometry.sort();
        shapes.push(geometry);
        let saved = serialize(&imported.document).unwrap();
        let restored = parse(&saved).unwrap();
        assert_eq!(serialize(&restored.document).unwrap(), saved);
    }
    assert_eq!(shapes[0], shapes[1]);
}

#[test]
fn plexers_runtime_defaults_and_roundtrip_preserve_terminal_roles_and_policies() {
    use logisim_core::{Plexer, PlexerRole};
    let wrap = |body: &str| xml(body).replace("<main", r##"<lib name="p" desc="#Plexers"/><main"##);
    let imported=parse(&wrap(r#"<comp lib="p" name="Multiplexer" loc="(100,100)"/><comp lib="p" name="Multiplexer" loc="(200,100)"><a name="enable" val="false"/></comp><comp lib="p" name="Priority Encoder" loc="(300,100)"/>"#)).unwrap();
    assert!(imported.warnings.is_empty());
    let cs = &imported.document.main_circuit().circuit.components;
    assert!(cs[0].plexing.enable);
    assert!(!cs[1].plexing.enable);
    assert_eq!(cs[2].plexing.select_bits, 3);
    assert!(
        cs[0]
            .plexer_terminals()
            .iter()
            .any(|p| p.role == PlexerRole::Enable)
    );
    assert!(
        !Component::new(Kind::Plexer(Plexer::Multiplexer), Point::ZERO)
            .plexing
            .enable
    );
    let mut document = Document::default();
    for kind in [
        Plexer::Multiplexer,
        Plexer::Demultiplexer,
        Plexer::Decoder,
        Plexer::PriorityEncoder,
    ] {
        for select in 1..=5 {
            for facing in Facing::ALL {
                for altered in [false, true] {
                    let mut c =
                        Component::new(Kind::Plexer(kind), Point::new(select as i32 * 100, 100));
                    c.facing = facing;
                    c.bits = if kind.has_data_bits() { 32 } else { 1 };
                    c.plexing.select_bits = select;
                    c.plexing.disabled_zero = altered;
                    if kind == Plexer::PriorityEncoder {
                        c.plexing.no_input_zero = altered;
                    } else {
                        c.plexing.top_right = altered;
                        c.plexing.enable = altered;
                        if kind != Plexer::Multiplexer {
                            c.plexing.three_state = altered;
                        }
                    }
                    document.main_circuit_mut().circuit.add_component(c);
                }
            }
        }
    }
    let saved = serialize(&document).unwrap();
    let restored = parse(&saved).unwrap();
    assert!(restored.warnings.is_empty(), "{:?}", restored.warnings);
    for (a, b) in document
        .main_circuit()
        .circuit
        .components
        .iter()
        .zip(&restored.document.main_circuit().circuit.components)
    {
        assert_eq!(a, b);
        assert_eq!(a.ports(), b.ports());
    }
    assert_eq!(serialize(&restored.document).unwrap(), saved);
}
#[test]
fn unsupported_plexers_and_parameters_trigger_import_protection() {
    let bodies = [
        ("Multiplexer", r#"<a name="select" val="0"/>"#),
        ("Decoder", r#"<a name="select" val="6"/>"#),
        ("Multiplexer", r#"<a name="width" val="33"/>"#),
        ("Multiplexer", r#"<a name="enable" val="maybe"/>"#),
        ("Decoder", r#"<a name="selloc" val="unknown"/>"#),
        ("Decoder", r#"<a name="width" val="8"/>"#),
        (
            "Priority Encoder",
            r#"<a name="noInputState" val="unknown"/>"#,
        ),
        ("Priority Encoder", r#"<a name="enable" val="false"/>"#),
        ("Multiplexer", r#"<a name="disabled" val="1"/>"#),
        ("Bit Selector", ""),
    ];
    let body = bodies
        .into_iter()
        .map(|(name, attrs)| {
            format!(r#"<comp lib="p" name="{name}" loc="(100,100)">{attrs}</comp>"#)
        })
        .collect::<String>();
    let input = xml(&body).replace("<main", r##"<lib name="p" desc="#Plexers"/><main"##);
    let imported = parse(&input).unwrap();
    assert_eq!(imported.warnings.len(), 10);
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
fn arithmetic_runtime_defaults_modes_widths_and_roundtrip() {
    use logisim_core::{Arithmetic, Shift};
    let wrap =
        |body: &str| xml(body).replace("<main", r##"<lib name="arith" desc="#Arithmetic"/><main"##);
    for kind in Arithmetic::ALL {
        let imported = parse(&wrap(&format!(
            r#"<comp lib="arith" name="{}" loc="(100,100)"/>"#,
            kind.file_name()
        )))
        .unwrap();
        assert!(imported.warnings.is_empty());
        let c = &imported.document.main_circuit().circuit.components[0];
        assert_eq!(c.bits, 8);
        assert!(c.arithmetic.signed);
        assert_eq!(c.arithmetic.shift, Shift::LogicalLeft);
        for bits in [1, 3, 4, 5, 8, 32] {
            for shift in Shift::ALL {
                for signed in [true, false] {
                    let mut doc = imported.document.clone();
                    let c = &mut doc.main_circuit_mut().circuit.components[0];
                    c.bits = bits;
                    if kind == Arithmetic::Comparator {
                        c.arithmetic.signed = signed;
                    }
                    if kind == Arithmetic::Shifter {
                        c.arithmetic.shift = shift;
                    }
                    let c = c.clone();
                    let encoded = serialize(&doc).unwrap();
                    let loaded = parse(&encoded).unwrap();
                    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
                    let restored = &loaded.document.main_circuit().circuit.components[0];
                    assert_eq!(c, *restored);
                    for i in 0..c.ports().len() {
                        assert_eq!(c.port_width(i), restored.port_width(i));
                        assert_eq!(c.drives_port(i), restored.drives_port(i));
                    }
                    assert_eq!(encoded, serialize(&loaded.document).unwrap());
                }
            }
        }
    }
    let i=parse(&wrap(r#"<comp lib="arith" name="Comparator" loc="(0,0)"><a name="mode" val="unsigned"/></comp><comp lib="arith" name="Shifter" loc="(100,0)"><a name="width" val="5"/><a name="shift" val="rr"/></comp>"#)).unwrap();
    assert!(i.warnings.is_empty());
    assert!(
        !i.document.main_circuit().circuit.components[0]
            .arithmetic
            .signed
    );
    assert_eq!(
        i.document.main_circuit().circuit.components[1]
            .arithmetic
            .shift,
        Shift::RotateRight
    );
}

#[test]
fn arithmetic_unknown_attributes_modes_components_and_invalid_widths_are_protected() {
    let wrap =
        |body: &str| xml(body).replace("<main", r##"<lib name="a" desc="#Arithmetic"/><main"##);
    for (kind, attrs) in [
        ("Adder", r#"<a name="width" val="0"/>"#),
        ("Subtractor", r#"<a name="width" val="33"/>"#),
        ("Adder", r#"<a name="facing" val="west"/>"#),
        ("Adder", r#"<a name="mode" val="unsigned"/>"#),
        ("Comparator", r#"<a name="mode" val="unobserved"/>"#),
        ("Shifter", r#"<a name="shift" val="unobserved"/>"#),
        ("Shifter", r#"<a name="label" val="unsupported"/>"#),
        ("Multiplier", r#"<a name="width" val="33"/>"#),
        ("Divider", r#"<a name="mode" val="unsigned"/>"#),
        ("Negator", r#"<a name="facing" val="west"/>"#),
        ("Bit Adder", ""),
        ("BitAdder", r#"<a name="inputs" val="0"/>"#),
        ("BitAdder", r#"<a name="inputs" val="33"/>"#),
        ("BitAdder", r#"<a name="type" val="high1"/>"#),
        ("BitFinder", r#"<a name="type" val="unobserved"/>"#),
        ("BitFinder", r#"<a name="inputs" val="2"/>"#),
    ] {
        let i = parse(&wrap(&format!(
            r#"<comp lib="a" name="{kind}" loc="(0,0)">{attrs}</comp>"#
        )))
        .unwrap();
        assert!(!i.warnings.is_empty(), "{kind} {attrs}");
        assert!(i.document.main_circuit().circuit.components.is_empty());
        assert!(!serialize(&i.document).unwrap().contains("<comp "));
    }
    let mut c = Component::new(
        Kind::Arithmetic(logisim_core::Arithmetic::Adder),
        Point::ZERO,
    );
    c.facing = Facing::South;
    let mut doc = Document::default();
    doc.main_circuit_mut().circuit.add_component(c);
    assert!(serialize(&doc).is_err());
}

#[test]
fn bit_arithmetic_runtime_names_input_counts_and_search_modes_roundtrip() {
    use logisim_core::{Arithmetic, BitSearch};
    let wrap =
        |body: &str| xml(body).replace("<main", r##"<lib name="a" desc="#Arithmetic"/><main"##);
    for width in [1, 3, 4, 5, 8, 32] {
        for n in 1..=32 {
            let imported = parse(&wrap(&format!(r#"<comp lib="a" name="BitAdder" loc="(200,200)"><a name="width" val="{width}"/><a name="inputs" val="{n}"/></comp>"#))).unwrap();
            assert!(imported.warnings.is_empty());
            let c = &imported.document.main_circuit().circuit.components[0];
            assert_eq!(c.arithmetic.bit_inputs, n);
            assert_eq!(c.ports().len(), usize::from(n) + 1);
            let encoded = serialize(&imported.document).unwrap();
            assert!(encoded.contains("name=\"BitAdder\""));
            let restored = parse(&encoded).unwrap();
            assert!(restored.warnings.is_empty());
            assert_eq!(*c, restored.document.main_circuit().circuit.components[0]);
        }
        for (mode, attr) in [
            (BitSearch::LowestOne, ""),
            (BitSearch::HighestOne, r#"<a name="type" val="high1"/>"#),
            (BitSearch::LowestZero, r#"<a name="type" val="low0"/>"#),
            (BitSearch::HighestZero, r#"<a name="type" val="high0"/>"#),
        ] {
            let imported=parse(&wrap(&format!(r#"<comp lib="a" name="BitFinder" loc="(200,200)"><a name="width" val="{width}"/>{attr}</comp>"#))).unwrap();
            assert!(imported.warnings.is_empty());
            let c = &imported.document.main_circuit().circuit.components[0];
            assert_eq!(c.arithmetic.search, mode);
            assert_eq!(c.port_width(2), Some(1));
            assert!(c.drives_port(2));
            let encoded = serialize(&imported.document).unwrap();
            assert!(encoded.contains("name=\"BitFinder\""));
            let restored = parse(&encoded).unwrap();
            assert!(restored.warnings.is_empty());
            assert_eq!(*c, restored.document.main_circuit().circuit.components[0]);
        }
    }
    let mut document = Document::default();
    let id = document
        .main_circuit_mut()
        .circuit
        .add_component(Component::new(
            Kind::Arithmetic(Arithmetic::BitAdder),
            Point::ZERO,
        ));
    for n in [0, 33] {
        document
            .main_circuit_mut()
            .circuit
            .components
            .iter_mut()
            .find(|c| c.id == id)
            .unwrap()
            .arithmetic
            .bit_inputs = n;
        assert!(serialize(&document).is_err());
    }
}

#[test]
fn extra_plexers_runtime_defaults_and_all_properties_roundtrip() {
    use logisim_core::Plexer;
    let wrap = |body: &str| xml(body).replace("<main", r##"<lib name="p" desc="#Plexers"/><main"##);
    let defaults=parse(&wrap(r#"<comp lib="p" name="BitSelector" loc="(100,100)"/><comp lib="p" name="DisplayDecoder" loc="(200,100)"/>"#)).unwrap();
    assert!(defaults.warnings.is_empty());
    let cs = &defaults.document.main_circuit().circuit.components;
    assert_eq!(cs[0].bits, 8);
    assert_eq!(cs[0].plexing.group_bits, 1);
    assert!(cs[1].plexing.multibit);
    let mut doc = Document::default();
    for facing in Facing::ALL {
        for multibit in [true, false] {
            let mut c = Component::new(Kind::Plexer(Plexer::DisplayDecoder), Point::new(200, 200));
            c.facing = facing;
            c.plexing.multibit = multibit;
            doc.main_circuit_mut().circuit.add_component(c);
        }
        for bits in [1, 3, 8, 32] {
            for group in [1, 3, 7, 32] {
                for top in [false, true] {
                    let mut c =
                        Component::new(Kind::Plexer(Plexer::BitSelector), Point::new(400, 200));
                    c.facing = facing;
                    c.bits = bits;
                    c.plexing.group_bits = group;
                    c.plexing.top_right = top;
                    doc.main_circuit_mut().circuit.add_component(c);
                }
            }
        }
    }
    let encoded = serialize(&doc).unwrap();
    assert!(encoded.contains("name=\"BitSelector\""));
    assert!(encoded.contains("name=\"DisplayDecoder\""));
    let restored = parse(&encoded).unwrap();
    assert!(restored.warnings.is_empty());
    assert_eq!(
        doc.main_circuit().circuit,
        restored.document.main_circuit().circuit
    );
    for (kind, attrs) in [
        ("BitSelector", r#"<a name="group" val="0"/>"#),
        ("BitSelector", r#"<a name="group" val="33"/>"#),
        ("BitSelector", r#"<a name="enable" val="false"/>"#),
        ("BitSelector", r#"<a name="select" val="3"/>"#),
        ("DisplayDecoder", r#"<a name="width" val="4"/>"#),
        ("DisplayDecoder", r#"<a name="multibit" val="unknown"/>"#),
        ("DisplayDecoder", r#"<a name="disabled" val="0"/>"#),
    ] {
        let i = parse(&wrap(&format!(
            r#"<comp lib="p" name="{kind}" loc="(0,0)">{attrs}</comp>"#
        )))
        .unwrap();
        assert_eq!(i.warnings.len(), 1);
        assert!(i.document.main_circuit().circuit.components.is_empty());
    }
    for group in [0, 33] {
        let c = doc
            .main_circuit_mut()
            .circuit
            .components
            .iter_mut()
            .find(|c| c.kind == Kind::Plexer(Plexer::BitSelector))
            .unwrap();
        c.plexing.group_bits = group;
        assert!(serialize(&doc).is_err());
    }
}

#[test]
fn counter_and_universal_register_runtime_schema_roundtrip_and_unsupported_protection() {
    use logisim_core::{CounterOverflow, Trigger};
    let wrap = |body: &str| xml(body).replace("<main", "<lib name=\"m\" desc=\"#Memory\"/><main");
    let defaults = parse(&wrap(r#"<comp lib="m" name="Counter" loc="(200,200)"/><comp lib="m" name="Universal Register" loc="(300,200)"/>"#)).unwrap();
    assert!(defaults.warnings.is_empty());
    let cs = &defaults.document.main_circuit().circuit.components;
    assert_eq!(
        (cs[0].kind, cs[0].bits, cs[0].sequential.counter_max),
        (Kind::Counter, 8, 255)
    );
    assert_eq!((cs[1].kind, cs[1].bits), (Kind::ShiftRegister, 8));
    let mut doc = Document::single("main", logisim_core::Circuit::default());
    for bits in [1, 4, 32] {
        for action in CounterOverflow::ALL {
            for old in [false, true] {
                for trigger in [Trigger::Rising, Trigger::Falling] {
                    let mut c = Component::new(Kind::Counter, Point::new(200, 200));
                    c.bits = bits;
                    c.sequential.counter_max = if bits == 4 {
                        5
                    } else {
                        u32::MAX >> (32 - bits)
                    };
                    c.sequential.counter_old = old;
                    c.sequential.counter_overflow = action;
                    c.sequential.trigger = trigger;
                    c.label = "计数 Count <&>".into();
                    doc.main_circuit_mut().circuit.add_component(c);
                }
            }
        }
        for trigger in [Trigger::Rising, Trigger::Falling] {
            let mut c = Component::new(Kind::ShiftRegister, Point::new(400, 200));
            c.bits = bits;
            c.sequential.trigger = trigger;
            c.label = "移位 Shift".into();
            doc.main_circuit_mut().circuit.add_component(c);
        }
    }
    let encoded = serialize(&doc).unwrap();
    assert!(encoded.contains("name=\"Universal Register\""));
    assert!(encoded.contains("name=\"length\" val=\"32\""));
    assert!(!encoded.contains("val=\"wrap\"") && !encoded.contains("val=\"new\""));
    let restored = parse(&encoded).unwrap();
    assert!(restored.warnings.is_empty(), "{:?}", restored.warnings);
    assert_eq!(
        doc.main_circuit().circuit,
        restored.document.main_circuit().circuit
    );
    for (name, attr, value) in [
        ("Counter", "max", "0x100"),
        ("Counter", "max", "-1"),
        ("Counter", "ongoal", "unverified"),
        ("Counter", "behavior", "unverified"),
        ("Counter", "trigger", "high"),
        ("Counter", "facing", "west"),
        ("Universal Register", "length", "0"),
        ("Universal Register", "length", "33"),
        ("Universal Register", "width", "4"),
        ("Universal Register", "trigger", "low"),
    ] {
        let loaded = parse(&wrap(&format!(
            r#"<comp lib="m" name="{name}" loc="(0,0)"><a name="{attr}" val="{value}"/></comp>"#
        )))
        .unwrap();
        assert_eq!(loaded.warnings.len(), 1, "{name} {attr}={value}");
        assert!(loaded.document.main_circuit().circuit.components.is_empty());
    }
    let c = &mut doc.main_circuit_mut().circuit.components[0];
    c.sequential.counter_max = 2; // 1-bit counter cannot represent this maximum.
    assert!(serialize(&doc).is_err());
    for (bits, trigger, facing) in [
        (0, Trigger::Rising, Facing::East),
        (33, Trigger::Rising, Facing::East),
        (4, Trigger::High, Facing::East),
        (4, Trigger::Rising, Facing::West),
    ] {
        let mut c = Component::new(Kind::ShiftRegister, Point::ZERO);
        c.bits = bits;
        c.facing = facing;
        c.sequential.trigger = trigger;
        let mut d = Document::single("main", logisim_core::Circuit::default());
        d.main_circuit_mut().circuit.add_component(c);
        assert!(serialize(&d).is_err());
    }
}

#[test]
fn pin_tristate_pull_and_world_label_locations_roundtrip() {
    use logisim_core::{Facing, PinPull};
    let mut body = String::new();
    for (i, (tri, pull, location)) in [
        ("true", "none", "north"),
        ("false", "up", "east"),
        ("true", "down", "south"),
        ("false", "none", "west"),
    ]
    .iter()
    .enumerate()
    {
        body.push_str(&format!(r#"<comp lib="w" name="Pin" loc="({},0)"><a name="tristate" val="{tri}"/><a name="pull" val="{pull}"/><a name="labelloc" val="{location}"/><a name="width" val="32"/></comp>"#, i*100));
    }
    body.push_str(r#"<comp lib="w" name="Pin" loc="(0,100)"/>"#);
    let imported = parse(&xml(&body)).unwrap();
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    let components = &imported.document.main_circuit().circuit.components;
    assert_eq!(components.len(), 5);
    assert_eq!(components[1].pin.pull, PinPull::Up);
    assert_eq!(components[2].pin.pull, PinPull::Down);
    assert_eq!(components[3].label_facing, Facing::West);
    assert!(components[4].pin.three_state);
    let saved = serialize(&imported.document).unwrap();
    let restored = parse(&saved).unwrap();
    assert!(restored.warnings.is_empty());
    assert_eq!(restored.document, imported.document);
}
