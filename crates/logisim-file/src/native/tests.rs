use super::*;
#[test]
fn required_ids_and_empty_document_envelope() {
    let mut p = Project::default();
    assert!(serialize_native(&p).is_err());
    p.main_circuit_mut().circuit.agent_ids.circuit = Some(42);
    let bytes = serialize_native(&p).unwrap();
    assert_eq!(p, parse_native(&bytes).unwrap());
    assert_eq!(
        bytes,
        serialize_native(&parse_native(&bytes).unwrap()).unwrap()
    );
}

#[test]
fn persistent_targets_survive_reordering_and_reject_bad_references() {
    let mut p = Project::default();
    let sub = p.add("child".into()).unwrap();
    let pin = p
        .circuit_mut(sub)
        .unwrap()
        .circuit
        .add_component(logisim_core::Component::new(
            logisim_core::Kind::Pin,
            Point::ZERO,
        ));
    let a = p.circuit(sub).unwrap().editable_appearance();
    let c = &mut p.circuit_mut(sub).unwrap().circuit;
    c.appearance = Some(a);
    c.agent_ids.circuit = Some(1);
    c.agent_ids.items.insert(pin, 2);
    let instance = p.instance(sub, Point::new(200, 100)).unwrap();
    let c = &mut p.main_circuit_mut().circuit;
    c.agent_ids.circuit = Some(3);
    let id = c.add_component(instance);
    c.agent_ids.items.insert(id, 4);
    let bytes = serialize_native(&p).unwrap();
    let reopened = parse_native(&bytes).unwrap();
    assert_ne!(p.main, reopened.main);
    assert_eq!(
        reopened
            .circuits
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["main", "child"]
    );
    assert_eq!(bytes, serialize_native(&reopened).unwrap());
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for (pointer, bad) in [
        ("/version", "7"),
        ("/format", "other"),
        ("/main", "123"),
        (
            "/circuits/0/appearance/ports/0/pin",
            "c-00000000000000000000000000000004",
        ),
        (
            "/circuits/1/components/0/agent_id",
            "c-00000000000000000000000000000002",
        ),
        (
            "/circuits/1/components/0/kind/subcircuit",
            "k-00000000000000000000000000000009",
        ),
    ] {
        let mut v = value.clone();
        *v.pointer_mut(pointer).unwrap() = serde_json::json!(bad);
        assert!(
            parse_native(&serde_json::to_vec(&v).unwrap()).is_err(),
            "{pointer}"
        );
    }
}

#[test]
fn course_and_plugin_projects_survive_both_codecs() {
    for xml in [
        include_str!("../../../../examples/course/single-cycle.circ"),
        include_str!("../../../../examples/agent/register-file.circ"),
    ] {
        let mut import = crate::parse(xml).unwrap();
        assert!(!import.partial, "{:?}", import.warnings);
        IdentityAssignments::default()
            .complete(&mut import.document)
            .unwrap();
        let bytes = serialize_native(&import.document).unwrap();
        let restored = parse_native(&bytes).unwrap();
        assert_eq!(serialize_native(&restored).unwrap(), bytes);
        crate::ProjectFormat::Circ.export(&restored).unwrap();
    }
}

#[test]
fn legacy_export_reports_native_only_font() {
    let mut p = Project::default();
    let mut stick = logisim_core::Component::new(logisim_core::Kind::Joystick, Point::ZERO);
    stick.font.size = 24;
    p.main_circuit_mut().circuit.add_component(stick);
    IdentityAssignments::default().complete(&mut p).unwrap();
    let restored = parse_native(&serialize_native(&p).unwrap()).unwrap();
    assert_eq!(restored.main_circuit().circuit.components[0].font.size, 24);
    assert!(
        crate::ProjectFormat::Circ
            .export(&restored)
            .unwrap_err()
            .0
            .contains("component c-")
    );
}

#[test]
fn v1_envelope_and_json_budgets_reject_unknown_semantics() {
    assert!(
        parse_native(br#"{"version":42,"v42_data":[]}"#)
            .unwrap_err()
            .0
            .contains("version 42")
    );
    assert!(
        parse_native(br#"{"format":"other"}"#)
            .unwrap_err()
            .0
            .contains("format")
    );
    let mut p = Project::default();
    IdentityAssignments::default().complete(&mut p).unwrap();
    let json = String::from_utf8(serialize_native(&p).unwrap()).unwrap();
    for extra in [
        r#""version":1"#,
        r#""runtime":null"#,
        r#""extra":{"x":7,"x":8}"#,
    ] {
        assert!(parse_native(json.replacen('{', &format!("{{{extra},"), 1).as_bytes()).is_err());
    }
    let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
    for value in [serde_json::json!(-1), serde_json::json!(1)] {
        v["circuits"][0]["display_order"] = value;
        assert!(parse_native(&serde_json::to_vec(&v).unwrap()).is_err());
    }
    for (payload, expected) in [
        (
            format!(r#"{{"x":"{}"}}"#, "a".repeat(262145)),
            "string length",
        ),
        (
            format!(r#"{{"x":[{}0]}}"#, "0,".repeat(65536)),
            "array length",
        ),
        (
            format!(
                r#"{{"x":[{}]}}"#,
                vec![format!("[{}0]", "0,".repeat(999)); 1000].join(",")
            ),
            "node limit",
        ),
    ] {
        assert!(
            parse_native(payload.as_bytes())
                .unwrap_err()
                .0
                .contains(expected)
        );
    }
    assert!(parse_native(&vec![0; MAX_BYTES + 1]).is_err());
}

#[test]
fn builtin_families_and_programmed_memory_planes_roundtrip() {
    use logisim_core::{
        Arithmetic, Component, FlipFlop, Gate, Kind::*, Latch, Parity, PlaInput, Plexer,
    };
    let mut kinds = vec![
        ControlledBuffer,
        ControlledInverter,
        PullResistor,
        Transistor,
        TransmissionGate,
        Pin,
        Tunnel,
        Text,
        Constant,
        Power,
        Ground,
        Probe,
        Splitter,
        BitExtender,
        Clock,
        Register,
        Counter,
        ShiftRegister,
        Random,
        Button,
        Switch,
        DipSwitch,
        Slider,
        Joystick,
        Keyboard,
        Tty,
        Led,
        SevenSegment,
        HexDisplay,
        RgbLed,
        DotMatrix,
        Rom,
        Ram,
        PlaRom,
    ];
    kinds.extend(Gate::ALL.map(logisim_core::Kind::Gate));
    kinds.extend(Parity::ALL.map(logisim_core::Kind::Parity));
    kinds.extend(Latch::ALL.map(logisim_core::Kind::Latch));
    kinds.extend(FlipFlop::ALL.map(logisim_core::Kind::FlipFlop));
    kinds.extend(Plexer::ALL.map(logisim_core::Kind::Plexer));
    kinds.extend(Arithmetic::ALL.map(logisim_core::Kind::Arithmetic));
    let mut p = Project::default();
    let c = &mut p.main_circuit_mut().circuit;
    for (i, kind) in kinds.into_iter().enumerate() {
        let mut v = Component::new(kind, Point::new(100 + i as i32 * 180, 100));
        if kind == Rom {
            v.memory.contents.fill(4, 128, 0x3a);
            v.memory.contents.fill(130, 132, 0xfc);
        }
        if kind == PlaRom {
            v.pla.set_input(0, 0, PlaInput::Inverted);
            v.pla.set_output(0, 1, true);
        }
        if kind == Pin {
            v.label = "输入 & clock".into();
            v.font.family = "Serif".into();
            v.font.italic = true;
            v.color = [16, 24, 36];
        }
        c.add_component(v);
    }
    IdentityAssignments::default().complete(&mut p).unwrap();
    let bytes = serialize_native(&p).unwrap();
    let loaded = parse_native(&bytes).unwrap();
    assert_eq!(p, loaded);
    let xml = crate::ProjectFormat::Circ.export(&loaded).unwrap();
    let imported = crate::ProjectFormat::Circ.parse(&xml).unwrap();
    assert!(!imported.partial);
    assert_eq!(serialize_native(&imported.document).unwrap(), bytes);
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let parts = v["circuits"][0]["components"].as_array_mut().unwrap();
    let rom = parts.iter().find(|p| p["kind"] == "rom").unwrap();
    assert_eq!(
        rom["memory"]["contents"],
        serde_json::json!([[4, 128, 58], [130, 132, 252]])
    );
    let ram = parts.iter_mut().find(|p| p["kind"] == "ram").unwrap();
    ram["memory"]["contents"] = serde_json::json!([[0, 1, 1]]);
    assert!(
        parse_native(&serde_json::to_vec(&v).unwrap())
            .unwrap_err()
            .0
            .contains("RAM contents")
    );
}
