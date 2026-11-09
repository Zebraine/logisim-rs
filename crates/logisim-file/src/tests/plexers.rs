use crate::*;
use logisim_core::{Component, Facing, Kind, Point};

use super::support::*;

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
