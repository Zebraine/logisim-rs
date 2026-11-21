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
    let value = serde_json::to_value(Native::encode(&p).unwrap()).unwrap();
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
            parse_legacy_json(&serde_json::to_vec(&v).unwrap()).is_err(),
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
fn showcase_pairs_preserve_documents_and_ids_through_explicit_legacy_export() {
    use crate::ProjectFormat::{Circ, Lrs};
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/agent");
    for (name, parts, wires) in [("adder", 4, 5), ("counter", 7, 12), ("register-file", 7, 9)] {
        let bytes = std::fs::read(dir.join(format!("{name}.lrs"))).unwrap();
        let native = Lrs.parse(&bytes).unwrap().document;
        let xml = std::fs::read(dir.join(format!("{name}.circ"))).unwrap();
        let legacy = Circ.parse(&xml).unwrap();
        assert!(!legacy.partial && legacy.warnings.is_empty(), "{name}");
        // The canonical document covers persistent IDs, geometry, styles and dependencies.
        assert_eq!(Lrs.export(&legacy.document).unwrap(), bytes, "{name}");
        let exported = Circ.export(&native).unwrap();
        let reopened = Circ.parse(&exported).unwrap();
        assert!(!reopened.partial && reopened.warnings.is_empty(), "{name}");
        assert_eq!(Lrs.export(&reopened.document).unwrap(), bytes, "{name}");

        let circuit = &native.main_circuit().circuit;
        assert_eq!(native.circuits.len(), 1);
        assert_eq!(
            (circuit.components.len(), circuit.wires.len()),
            (parts, wires)
        );
        if name == "register-file" {
            let [source] = native.plugin_sources.as_slice() else {
                panic!("register-file requires exactly one course module")
            };
            assert_eq!(source.path, "../../plugins/course/course.wasm");
            assert!(source.fallback.is_empty() && dir.join(&source.path).is_file());
        } else {
            assert!(native.plugin_sources.is_empty());
        }
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
        parse_legacy_json(br#"{"version":42,"v42_data":[]}"#)
            .unwrap_err()
            .0
            .contains("version 42")
    );
    assert!(
        parse_legacy_json(br#"{"format":"other"}"#)
            .unwrap_err()
            .0
            .contains("format")
    );
    let mut p = Project::default();
    IdentityAssignments::default().complete(&mut p).unwrap();
    let json = serde_json::to_string(&Native::encode(&p).unwrap()).unwrap();
    for extra in [
        r#""version":1"#,
        r#""runtime":null"#,
        r#""extra":{"x":7,"x":8}"#,
    ] {
        assert!(
            parse_legacy_json(json.replacen('{', &format!("{{{extra},"), 1).as_bytes()).is_err()
        );
    }
    let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
    for value in [serde_json::json!(-1), serde_json::json!(1)] {
        v["circuits"][0]["display_order"] = value;
        assert!(parse_legacy_json(&serde_json::to_vec(&v).unwrap()).is_err());
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
            parse_legacy_json(payload.as_bytes())
                .unwrap_err()
                .0
                .contains(expected)
        );
    }
    assert!(parse_legacy_json(&vec![0; MAX_BYTES + 1]).is_err());
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
    let mut v = serde_json::to_value(Native::encode(&loaded).unwrap()).unwrap();
    let parts = v["circuits"][0]["components"].as_array_mut().unwrap();
    let rom = parts.iter().find(|p| p["kind"] == "rom").unwrap();
    assert_eq!(
        rom["memory"]["contents"],
        serde_json::json!([[4, 128, 58], [130, 132, 252]])
    );
    let ram = parts.iter_mut().find(|p| p["kind"] == "ram").unwrap();
    ram["memory"]["contents"] = serde_json::json!([[0, 1, 1]]);
    assert!(
        parse_legacy_json(&serde_json::to_vec(&v).unwrap())
            .unwrap_err()
            .0
            .contains("RAM contents")
    );
}

#[test]
fn binary_header_versions_keys_types_and_limits_are_checked_before_open() {
    use serde_cbor::Value as V;
    let mut p = Project::default();
    p.main_circuit_mut().circuit.agent_ids.circuit = Some(9);
    let good = serialize_native(&p).unwrap();
    assert_eq!(&good[..6], b"LRS\0\0\x01");
    let header = &good[..6];
    let invalid = |payload: &[u8]| parse_native(&[header, payload].concat()).unwrap_err().0;
    for (payload, reason) in [
        (&[0xa1, 0, 0x7a, 0, 4, 0, 1][..], "string length"),
        (
            &[0xa1, 0, 0x9b, 255, 255, 255, 255, 255, 255, 255, 255],
            "container length",
        ),
        (&[0xa1, 0, 0xbf], "indefinite"),
        (&[0xa1, 0, 0xa2, 0x61, b'p', 1, 0x61, b'p', 2], "Duplicate"),
        (&[0xa2, 0, 1, 0x18, 0, 2], "Duplicate"),
        (&[0xa1, 0, 0xc1, 0], "type"),
        (&[0xa1, 0, 0xf7], "simple"),
        (&[0xa1, 0, 0x61], "Truncated"),
    ] {
        assert!(invalid(payload).contains(reason), "{reason}");
    }
    let mut huge = vec![0xa1, 0, 0x98, 32];
    for _ in 0..32 {
        huge.extend([0x99, 0x80, 0]);
        huge.resize(huge.len() + 32768, 0);
    }
    assert!(invalid(&huge).contains("node limit"));
    let mut nested = vec![0xa1, 0];
    nested.extend([0x81; 128]);
    nested.push(0);
    assert!(invalid(&nested).contains("depth"));
    assert!(parse_native(&[good.as_slice(), &[0]].concat()).is_err());
    assert!(parse_native(&vec![0; MAX_BYTES + 1]).is_err());
    assert!(parse_native(b"LRS\0\0").is_err());
    assert!(
        parse_native(b"LRS\0\0\x02")
            .unwrap_err()
            .0
            .contains("version 2")
    );
    for (key, value) in [
        (0, V::Text("other".into())),
        (1, V::Integer(2)),
        (2, V::Integer(9)),
        (2, V::Text("missing main".into())),
        (5, V::Bool(true)),
    ] {
        let V::Map(mut fields) = serde_cbor::from_slice(&good[6..]).unwrap() else {
            unreachable!()
        };
        fields.insert(V::Integer(key), value);
        let bytes = serde_cbor::to_vec(&V::Map(fields)).unwrap();
        assert!(
            parse_native(&[header, &bytes].concat()).is_err(),
            "field {key}"
        );
    }
    let json = serde_json::to_vec(&Native::encode(&p).unwrap()).unwrap();
    assert!(
        parse_native(&json)
            .unwrap_err()
            .0
            .contains("Legacy JSON native")
    );
    assert_eq!(
        serialize_native(&parse_legacy_json(&json).unwrap()).unwrap(),
        good
    );
    assert!(parse_legacy_json(&good).is_err());
    assert!(
        crate::ProjectFormat::Circ
            .parse(&good)
            .unwrap_err()
            .0
            .contains(".lrs")
    );
}
