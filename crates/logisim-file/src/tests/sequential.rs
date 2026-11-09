use crate::*;
use logisim_core::{Component, Facing, Kind, Point};

use super::support::*;

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
