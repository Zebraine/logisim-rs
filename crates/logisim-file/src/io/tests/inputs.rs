use super::super::*;
use crate::{Document, parse, serialize};
use logisim_core::{Circuit, Point};
fn xml(kind: &str, attrs: &str) -> String {
    format!(
        r##"<project source="2.16.2.2.jar" version="1.0"><lib name="io" desc="#I/O"/><main name="main"/><circuit name="main"><comp lib="io" name="{kind}" loc="(200,100)">{attrs}</comp></circuit></project>"##
    )
}
#[test]
fn input_defaults_and_observed_attributes_roundtrip_without_runtime_state() {
    for (kind, name, bits) in [
        (Kind::Switch, "Switch", 1),
        (Kind::DipSwitch, "DipSwitch", 1),
        (Kind::Slider, "Slider", 8),
    ] {
        let i = parse(&xml(name, "")).unwrap();
        assert!(i.warnings.is_empty(), "{:?}", i.warnings);
        let c = &i.document.main_circuit().circuit.components[0];
        assert_eq!(c.bits, bits);
        assert_eq!(c.io, Component::new(kind, Point::ZERO).io);
        for facing in Facing::ALL {
            for n in [1, 4, 32] {
                for radix in Radix::ALL {
                    for reverse in [false, true] {
                        let mut c = Component::new(kind, Point::new(200, 100));
                        c.facing = facing;
                        c.label_facing = facing;
                        c.label = "输入 & Input".into();
                        c.font.size = 18;
                        c.color = [24, 53, 70];
                        c.io.on_color = [18, 52, 86, 128];
                        match kind {
                            Kind::DipSwitch => c.io.switch_count = n,
                            Kind::Slider => {
                                c.bits = n;
                                c.radix = radix;
                                c.io.slider_reverse = reverse;
                                c.io.slider_value = c.slider_max();
                            }
                            _ => c.bits = n,
                        }
                        let mut circuit = Circuit::default();
                        let id = circuit.add_component(c);
                        let d = Document::single("main", circuit);
                        let s = serialize(&d).unwrap();
                        assert!(s.contains("#12345680"));
                        assert!(!s.contains("offcolor"));
                        assert!(!s.contains("pressed"));
                        if kind == Kind::DipSwitch {
                            assert!(!s.contains("name=\"width\""));
                            assert!(s.contains("NSwitches"));
                        }
                        if kind == Kind::Slider {
                            assert_eq!(s.contains("right_to_left"), reverse);
                        } else {
                            assert!(!s.contains("name=\"value\""));
                        }
                        let i = parse(&s).unwrap();
                        assert!(i.warnings.is_empty(), "{:?}", i.warnings);
                        let mut after = i.document.main_circuit().circuit.components[0].clone();
                        after.id = id;
                        assert_eq!(&after, d.main_circuit().circuit.component(id).unwrap());
                        assert_eq!(serialize(&i.document).unwrap(), s);
                    }
                }
            }
        }
    }
}
#[test]
fn unsupported_input_options_and_invalid_ranges_protect_originals() {
    for (kind, attrs) in [
        ("Switch", r##"<a name="state" val="1"/>"##),
        ("Switch", r##"<a name="width" val="33"/>"##),
        ("DipSwitch", r##"<a name="NSwitches" val="0"/>"##),
        ("DipSwitch", r##"<a name="NSwitches" val="33"/>"##),
        ("DipSwitch", r##"<a name="width" val="4"/>"##),
        ("Slider", r##"<a name="value" val="0x100"/>"##),
        ("Slider", r##"<a name="value" val="-1"/>"##),
        ("Slider", r##"<a name="Direction" val="unobserved"/>"##),
        ("Slider", r##"<a name="radix" val="unknown"/>"##),
        ("Slider", r##"<a name="offcolor" val="#000000"/>"##),
    ] {
        let i = parse(&xml(kind, attrs)).unwrap();
        assert!(!i.warnings.is_empty(), "{kind} {attrs}");
        assert!(i.document.main_circuit().circuit.components.is_empty());
    }
    let mut c = Component::new(Kind::Slider, Point::ZERO);
    c.io.slider_value = 256;
    assert!(write(&mut String::new(), &c).is_err());
    c = Component::new(Kind::DipSwitch, Point::ZERO);
    c.io.switch_count = 33;
    assert!(write(&mut String::new(), &c).is_err());
}
