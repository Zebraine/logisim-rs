use crate::*;
use logisim_core::{Component, Facing, Kind, Point};

use super::support::*;

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
