use crate::*;
use logisim_core::{Component, Facing, Kind, Point};

use super::support::*;

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
                c.font.size = 18;
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
