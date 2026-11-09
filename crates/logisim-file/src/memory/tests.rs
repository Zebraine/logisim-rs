use super::*;
use crate::{Document, parse, serialize};
use logisim_core::{Circuit, Point};
fn xml(attributes: &str, kind: &str) -> String {
    format!(
        r##"<project source="2.16.2.2.jar" version="1.0"><lib name="m" desc="#Memory"/><main name="main"/><circuit name="main"><comp lib="m" name="{kind}" loc="(200,100)">{attributes}</comp></circuit></project>"##
    )
}
#[test]
fn runtime_defaults_rle_and_memory_interfaces_roundtrip() {
    for kind in [Kind::Rom, Kind::Ram] {
        let imported = parse(&xml("", kind.name())).unwrap();
        assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
        let c = &imported.document.main_circuit().circuit.components[0];
        assert_eq!(
            (
                c.bits,
                c.memory.address_bits,
                c.memory.simple,
                c.memory.select_high
            ),
            (8, 8, true, false)
        );
        for interface in MemoryInterface::ALL {
            for simple in [false, true] {
                for high in [false, true] {
                    if kind == Kind::Rom && interface != MemoryInterface::SharedSync {
                        continue;
                    }
                    let mut c = Component::new(kind, Point::new(200, 100));
                    c.bits = 32;
                    c.memory.address_bits = 24;
                    c.memory.interface = interface;
                    c.memory.simple = simple;
                    c.memory.select_high = high;
                    c.label = "程序 & 数据".into();
                    if kind == Kind::Rom {
                        c.memory.contents.fill(0, 4, 0xabcdef01);
                        c.memory.contents.set(0xff_ffff, 0xdead_beef);
                    }
                    let mut circuit = Circuit::default();
                    let id = circuit.add_component(c);
                    let doc = Document::single("main", circuit);
                    let text = serialize(&doc).unwrap();
                    if kind == Kind::Rom {
                        assert!(text.contains("4*abcdef01 16777211*0 deadbeef"));
                    } else {
                        assert!(!text.contains("contents"));
                    }
                    let imported = parse(&text).unwrap();
                    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
                    let before = doc.main_circuit().circuit.component(id).unwrap();
                    let mut after = imported.document.main_circuit().circuit.components[0].clone();
                    after.id = id;
                    assert_eq!(before, &after);
                    assert_eq!(serialize(&imported.document).unwrap(), text);
                }
            }
        }
    }
    let contents = parse_contents(
        "addr/data: 24 32\n80000001 ffffffff 12345678 16777212*0 deadbeef",
        24,
        32,
    )
    .unwrap();
    assert_eq!(
        (
            contents.get(0),
            contents.get(1),
            contents.get(2),
            contents.get(3),
            contents.get(0xff_ffff)
        ),
        (0x80000001, u32::MAX, 0x12345678, 0, 0xdeadbeef)
    );
    assert_eq!(
        parse_contents("addr/data: 24 32\n16777216*ffffffff", 24, 32)
            .unwrap()
            .boundaries()
            .count(),
        2
    );
}
#[test]
fn invalid_or_unimplemented_memory_data_is_omitted_and_protected() {
    for (kind, attrs) in [
        ("ROM", r#"<a name="addrWidth" val="1"/>"#),
        ("RAM", r#"<a name="dataWidth" val="33"/>"#),
        ("RAM", r#"<a name="bus" val="unknown"/>"#),
        ("RAM", r#"<a name="trigger" val="falling"/>"#),
        (
            "RAM",
            r#"<a name="contents">addr/data: 8 8
ff</a>"#,
        ),
        (
            "ROM",
            r#"<a name="contents">addr/data: 9 8
ff</a>"#,
        ),
        (
            "ROM",
            r#"<a name="contents">addr/data: 8 8
257*0</a>"#,
        ),
        (
            "ROM",
            r#"<a name="contents">addr/data: 8 8
0*ff</a>"#,
        ),
        (
            "ROM",
            r#"<a name="contents">addr/data: 8 8
100</a>"#,
        ),
        (
            "ROM",
            r#"<a name="contents">addr/data: 8 8
</a>"#,
        ),
        (
            "ROM",
            r#"<a name="contents">addr/data: 8 8
+1</a>"#,
        ),
    ] {
        let imported = parse(&xml(attrs, kind)).unwrap();
        assert!(!imported.warnings.is_empty(), "{attrs}");
        assert!(
            imported
                .document
                .main_circuit()
                .circuit
                .components
                .is_empty()
        );
    }
    let mut c = Component::new(Kind::Ram, Point::ZERO);
    c.memory.contents.set(0, 1);
    let mut circuit = Circuit::default();
    circuit.add_component(c);
    assert!(serialize(&Document::single("main", circuit)).is_err());
}
