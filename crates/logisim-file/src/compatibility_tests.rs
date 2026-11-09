use crate::{parse, serialize};
use logisim_core::{Kind, Point};

fn project(main: &str, definitions: &str) -> String {
    format!(
        r##"<project source="2.16.2.2.jar" version="1.0"><lib name="w" desc="#Wiring"/><lib name="p" desc="#Plexers"/><lib name="i" desc="#I/O"/><lib name="b" desc="#Base"/><main name="main"/><circuit name="main">{main}</circuit>{definitions}</project>"##
    )
}

#[test]
fn observed_redundant_fields_and_centered_button_normalize_without_omissions() {
    let source = project(
        r#"<comp lib="p" name="Multiplexer" loc="(100,100)"><a name="size" val="30"/></comp><comp lib="w" name="Probe" loc="(200,100)"><a name="width" val="16"/></comp><comp lib="i" name="Button" loc="(300,100)"><a name="label" val="C"/><a name="labelloc" val="center"/></comp>"#,
        "",
    );
    let imported = parse(&source).unwrap();
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    let components = &imported.document.main_circuit().circuit.components;
    assert_eq!(components.len(), 3);
    assert_eq!(components[1].bits, 1);
    assert!(components[2].label_center);
    let saved = serialize(&imported.document).unwrap();
    assert!(!saved.contains("name=\"size\""));
    assert!(!saved.contains("name=\"width\" val=\"16\""));
    assert!(saved.contains("val=\"center\""));
    assert_eq!(parse(&saved).unwrap().document, imported.document);
    for (from, to) in [
        ("val=\"30\"", "val=\"40\""),
        ("val=\"16\"", "val=\"8\""),
        ("val=\"center\"", "val=\"other\""),
    ] {
        let rejected = parse(&source.replace(from, to)).unwrap();
        assert!(!rejected.warnings.is_empty());
        assert_eq!(rejected.document.main_circuit().circuit.components.len(), 2);
    }
}

#[test]
fn text_transparency_roundtrips_and_opaque_alpha_normalizes() {
    for (color, alpha, canonical) in [
        ("#258a2dc6", 198, "#258a2dc6"),
        ("#258a2d00", 0, "#258a2d00"),
        ("#258a2dff", 255, "#258a2d"),
    ] {
        let input = project(
            &format!(
                r#"<comp lib="b" name="Text" loc="(100,100)"><a name="text" val="RGBA"/><a name="color" val="{color}"/></comp>"#
            ),
            "",
        );
        let imported = parse(&input).unwrap();
        assert!(imported.warnings.is_empty());
        let text = &imported.document.main_circuit().circuit.components[0];
        assert_eq!(text.color, [37, 138, 45]);
        assert_eq!(text.color_alpha, alpha);
        let saved = serialize(&imported.document).unwrap();
        assert!(saved.contains(&format!("val=\"{canonical}\"")));
        assert_eq!(parse(&saved).unwrap().document, imported.document);
    }
}

#[test]
fn old_output_port_marker_preserves_center_binding_and_instance_connections() {
    let definition = r##"<circuit name="child"><appear><rect x="50" y="50" width="40" height="40" stroke="#000000" stroke-width="2" fill="none"/><circ-port x="46" y="66" width="8" height="8" pin="100,100"/><circ-port x="86" y="66" width="8" height="8" pin="200,100"/><circ-anchor x="87" y="67" width="6" height="6" facing="east"/></appear><comp lib="w" name="Pin" loc="(100,100)"><a name="tristate" val="false"/></comp><comp lib="w" name="Pin" loc="(200,100)"><a name="output" val="true"/><a name="facing" val="west"/></comp><wire from="(100,100)" to="(200,100)"/></circuit>"##;
    let main = r#"<comp name="child" loc="(200,540)"/><wire from="(200,540)" to="(300,540)"/>"#;
    let imported = parse(&project(main, definition)).unwrap();
    assert!(imported.warnings.is_empty());
    let circuit = &imported.document.main_circuit().circuit;
    let instance = &circuit.components[0];
    assert!(matches!(instance.kind, Kind::Subcircuit(_)));
    assert_eq!(
        instance.ports(),
        [Point::new(160, 540), Point::new(200, 540)]
    );
    let saved = serialize(&imported.document).unwrap();
    assert!(saved.contains("x=\"85\" y=\"65\""));
    let restored = parse(&saved).unwrap();
    assert_eq!(
        restored.document.main_circuit().circuit.components[0].ports(),
        instance.ports()
    );
    assert_eq!(serialize(&restored.document).unwrap(), saved);
    let binding_positions = |p: &logisim_core::Project| {
        let child = &p.circuits[1].circuit;
        child
            .appearance
            .as_ref()
            .unwrap()
            .ports
            .iter()
            .map(|(id, at)| (child.component(*id).unwrap().position, *at))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        binding_positions(&restored.document),
        binding_positions(&imported.document)
    );
    for invalid in [
        definition.replace(
            "x=\"86\" y=\"66\" width=\"8\" height=\"8\"",
            "x=\"86\" y=\"66\" width=\"9\" height=\"9\"",
        ),
        definition.replace("pin=\"200,100\"", "pin=\"999,100\""),
    ] {
        let rejected = parse(&project(main, &invalid)).unwrap();
        assert!(!rejected.warnings.is_empty());
        assert!(
            rejected
                .document
                .main_circuit()
                .circuit
                .components
                .is_empty()
        );
        assert_eq!(
            rejected.document.main_circuit().circuit.wires,
            circuit.wires
        );
    }
}

#[test]
fn unsigned_decimal_constants_match_observed_values_and_width_truncation() {
    for (bits, raw, expected) in [
        (32, "0", 0),
        (32, "1", 1),
        (32, "2", 2),
        (32, "10", 10),
        (32, "15", 15),
        (32, "255", 255),
        (32, "4294967295", u32::MAX),
        (4, "15", 15),
        (4, "16", 0),
    ] {
        let source = project(
            &format!(
                r#"<comp lib="w" name="Constant" loc="(0,0)"><a name="width" val="{bits}"/><a name="value" val="{raw}"/></comp>"#
            ),
            "",
        );
        let imported = parse(&source).unwrap();
        assert!(imported.warnings.is_empty());
        assert_eq!(
            imported.document.main_circuit().circuit.components[0].constant,
            expected
        );
        assert_eq!(
            parse(&serialize(&imported.document).unwrap())
                .unwrap()
                .document,
            imported.document
        );
    }
    for raw in ["00", "010", "+1", "-1", "0b10", "4294967296"] {
        let imported = parse(&project(
            &format!(
                r#"<comp lib="w" name="Constant" loc="(0,0)"><a name="value" val="{raw}"/></comp>"#
            ),
            "",
        ))
        .unwrap();
        assert!(!imported.warnings.is_empty());
        assert!(
            imported
                .document
                .main_circuit()
                .circuit
                .components
                .is_empty()
        );
    }
}

#[test]
fn styled_fonts_and_circuit_labels_roundtrip_without_changing_ports() {
    for (style, bold, italic) in [
        ("plain", false, false),
        ("bold", true, false),
        ("italic", false, true),
        ("bolditalic", true, true),
    ] {
        let input = project(
            &format!(
                r##"<comp lib="b" name="Text" loc="(30,40)"><a name="text" val="用户文字"/><a name="font" val="Microsoft YaHei UI {style} 16"/></comp><comp name="child" loc="(200,100)"/>"##
            ),
            &format!(
                r##"<circuit name="child"><a name="clabel" val="ALU"/><a name="clabelfont" val="SansSerif {style} 18"/><a name="clabelup" val="north"/><a name="clabelcolor" val="#2255aa"/><comp lib="w" name="Pin" loc="(100,100)"/></circuit>"##
            ),
        );
        let imported = parse(&input).unwrap();
        assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
        let main = &imported.document.main_circuit().circuit;
        assert_eq!(main.components[0].font.family, "Microsoft YaHei UI");
        assert_eq!(
            (main.components[0].font.bold, main.components[0].font.italic),
            (bold, italic)
        );
        assert_eq!(main.components[1].interface.label.text, "ALU");
        assert_eq!(
            main.components[1].interface.label.up,
            logisim_core::Facing::North
        );
        assert_eq!(main.components[1].interface.label.color, [34, 85, 170]);
        let saved = serialize(&imported.document).unwrap();
        let restored = parse(&saved).unwrap();
        assert!(restored.warnings.is_empty());
        assert_eq!(restored.document, imported.document);
        for invalid in [
            input.replace("clabelup", "unimplemented"),
            input.replace(" 18", " 0"),
            input.replace("#2255aa", "#bad"),
        ] {
            assert!(!parse(&invalid).unwrap().warnings.is_empty());
        }
    }
}

#[test]
fn rebuilt_appearance_ports_bind_to_new_internal_pin_positions_after_roundtrip() {
    use logisim_core::{
        Facing, Project,
        generation::{Analysis, GateBasis, RebuildTarget},
    };
    let mut a = Analysis::default();
    for name in ["A", "B"] {
        a.add_input(name.into()).unwrap();
    }
    a.add_output("Y".into()).unwrap();
    a.apply_expression(0, "A ^ B").unwrap();
    let mut p = Project::single("child", a.generate().unwrap());
    let mut appearance = p.main_circuit().editable_appearance();
    appearance.facing = Facing::North;
    p.main_circuit_mut().circuit.appearance = Some(appearance);
    let child = p.main;
    let host = p.add("host".into()).unwrap();
    let mut instance = p.instance(child, Point::new(400, 400)).unwrap();
    instance.facing = Facing::West;
    p.circuit_mut(host).unwrap().circuit.add_component(instance);
    let target = RebuildTarget::bind(p.main_circuit(), &a).unwrap();
    for changed in [false, true] {
        let mut analysis = a.clone();
        if changed {
            analysis.add_input("C".into()).unwrap();
        }
        let mut rebuilt = p.clone();
        rebuilt.main_circuit_mut().circuit = target
            .rebuild(
                p.main_circuit(),
                analysis.generate_with_basis(GateBasis::Nand).unwrap(),
            )
            .unwrap();
        rebuilt.refresh_interfaces(false);
        let saved = serialize(&rebuilt).unwrap();
        let read = parse(&saved).unwrap();
        assert!(read.warnings.is_empty(), "{:?}", read.warnings);
        let ports = |p: &Project| {
            let mut result: Vec<_> = p
                .named("child")
                .unwrap()
                .interface()
                .ports
                .into_iter()
                .map(|p| (p.label, p.offset, p.output, p.bits))
                .collect();
            result.sort();
            result
        };
        assert_eq!(ports(&read.document), ports(&rebuilt));
        let positions = |p: &Project| {
            let d = &p.named("child").unwrap().circuit;
            let mut result: Vec<_> = d
                .appearance
                .as_ref()
                .unwrap()
                .ports
                .iter()
                .map(|(id, at)| {
                    let pin = d.component(*id).unwrap();
                    (pin.label.clone(), pin.position, *at)
                })
                .collect();
            result.sort();
            result
        };
        assert_eq!(positions(&read.document), positions(&rebuilt));
        assert_eq!(serialize(&read.document).unwrap(), saved);
    }
}
