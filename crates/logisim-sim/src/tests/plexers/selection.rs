//! Expected values from the runtime matrices and enable/edit/reset trace.
use super::support::*;
use crate::*;
use PlexerRole::*;
use logisim_core::{Component, Facing, Kind, Plexer, PlexerRole, Point};
fn value(text: &str) -> Value {
    Value::from_bits(text.chars().rev().map(|c| match c {
        '0' => Bit::Zero,
        '1' => Bit::One,
        'X' => Bit::Unknown,
        'E' => Bit::Error,
        _ => unreachable!(),
    }))
}
fn selector(
    width: u8,
    group: u8,
    facing: Facing,
    top: bool,
) -> (Circuit, Id, Vec<(PlexerRole, Id)>) {
    let mut c = Component::new(Kind::Plexer(Plexer::BitSelector), Point::ZERO);
    c.bits = width;
    c.plexing.group_bits = group;
    c.facing = facing;
    c.plexing.top_right = top;
    fixture(c)
}
#[test]
fn bit_selector_selects_groups_and_zero_pads_at_width_boundaries() {
    for facing in Facing::ALL {
        for top in [false, true] {
            for (width, group, data, select, want) in [
                (8, 1, 0xad, 0, 1),
                (8, 1, 0xad, 1, 0),
                (8, 1, 0xad, 2, 1),
                (8, 1, 0xad, 3, 1),
                (8, 1, 0xad, 4, 0),
                (8, 1, 0xad, 5, 1),
                (8, 1, 0xad, 6, 0),
                (8, 1, 0xad, 7, 1),
                (8, 3, 0xad, 0, 5),
                (8, 3, 0xad, 1, 5),
                (8, 3, 0xad, 2, 2),
                (8, 3, 0xad, 3, 0),
                (5, 3, 31, 0, 7),
                (5, 3, 31, 1, 3),
                (8, 32, 0xad, 0, 0xad),
                (8, 32, 0xad, 1, 0),
                (1, 1, 1, 0, 1),
                (1, 1, 1, 1, 0),
                (32, 1, 0x80000001, 0, 1),
                (32, 1, 0x80000001, 1, 0),
                (32, 1, 0x80000001, 30, 0),
                (32, 1, 0x80000001, 31, 1),
                (32, 7, 0xdeadbeef, 0, 0x6f),
                (32, 7, 0xdeadbeef, 1, 0x7d),
                (32, 7, 0xdeadbeef, 4, 13),
                (32, 7, 0xdeadbeef, 5, 0),
                (32, 7, 0xdeadbeef, 7, 0),
                (32, 32, 0xdeadbeef, 0, 0xdeadbeef),
            ] {
                let (c, id, inputs) = selector(width, group, facing, top);
                let mut s = Simulator::default();
                s.synchronize(&c);
                settle(&mut s);
                drive(&mut s, &inputs, Input(0), Value::known(width, data));
                drive(
                    &mut s,
                    &inputs,
                    Select,
                    Value::known(c.component(id).unwrap().bit_selector_select_bits(), select),
                );
                settle(&mut s);
                assert_eq!(output(&s, id, Output(0)), Value::known(group, want));
            }
        }
    }
}
#[test]
fn selector_undefined_selection_is_unknown_but_selected_error_bits_pass_through() {
    let (c, id, inputs) = selector(8, 3, Facing::East, false);
    let mut s = Simulator::default();
    s.synchronize(&c);
    settle(&mut s);
    drive(&mut s, &inputs, Input(0), value("E10X0110"));
    for (sel, want) in [
        ("00", "110"),
        ("01", "0X0"),
        ("10", "0E1"),
        ("11", "000"),
        ("XX", "XXX"),
        ("EE", "XXX"),
        ("XE", "XXX"),
        ("0X", "XXX"),
    ] {
        drive(&mut s, &inputs, Select, value(sel));
        settle(&mut s);
        assert_eq!(output(&s, id, Output(0)).binary(), want);
    }
}
fn display(multi: bool, facing: Facing) -> (Circuit, Id, Vec<(PlexerRole, Id)>) {
    let mut c = Component::new(Kind::Plexer(Plexer::DisplayDecoder), Point::ZERO);
    c.plexing.multibit = multi;
    c.facing = facing;
    fixture(c)
}
fn data(s: &mut Simulator, ins: &[(PlexerRole, Id)], multi: bool, v: Value) {
    if multi {
        drive(s, ins, Input(0), v);
    } else {
        for i in 0..4 {
            drive(s, ins, Input(i), Value::filled(1, v.bit(i)));
        }
    }
}
fn controls(s: &mut Simulator, ins: &[(PlexerRole, Id)], values: [Bit; 4]) {
    for (role, v) in [Enable, LampTest, Blanking, ZeroBlanking]
        .into_iter()
        .zip(values)
    {
        drive(s, ins, role, Value::filled(1, v));
    }
}
fn segments(s: &Simulator, id: Id) -> Value {
    Value::from_bits((0..7).map(|i| output(s, id, Output(i)).bit(0)))
}
#[test]
fn display_all_digits_in_both_input_formats_and_four_directions() {
    let expected = [
        "0111111", "0000110", "1011011", "1001111", "1100110", "1101101", "1111101", "0000111",
        "1111111", "1100111", "1110111", "1111100", "0111001", "1011110", "1111001", "1110001",
    ];
    for multi in [true, false] {
        for facing in Facing::ALL {
            let (c, id, ins) = display(multi, facing);
            let mut s = Simulator::default();
            s.synchronize(&c);
            settle(&mut s);
            controls(&mut s, &ins, [Bit::One; 4]);
            for (d, want) in expected.into_iter().enumerate() {
                data(&mut s, &ins, multi, Value::known(4, d as u32));
                settle(&mut s);
                assert_eq!(segments(&s, id).binary(), want, "{multi} {facing:?} {d}");
            }
        }
    }
}
#[test]
fn display_bus_errors_and_discrete_errors_have_distinct_observed_behavior() {
    for multi in [true, false] {
        let (c, id, ins) = display(multi, Facing::East);
        let mut s = Simulator::default();
        s.synchronize(&c);
        settle(&mut s);
        controls(&mut s, &ins, [Bit::One; 4]);
        for (input, discrete) in [
            ("XXXX", "XXXXXXX"),
            ("EEEE", "0111111"),
            ("000X", "XXXXXXX"),
            ("000E", "0111111"),
            ("X000", "XXXXXXX"),
            ("E000", "0111111"),
            ("00X1", "XXXXXXX"),
            ("00E1", "0000110"),
            ("01X1", "XXXXXXX"),
            ("01E1", "1101101"),
            ("XX11", "XXXXXXX"),
            ("EE11", "1001111"),
            ("00XE", "XXXXXXX"),
            ("00EX", "XXXXXXX"),
            ("1X11", "XXXXXXX"),
            ("1E11", "1111100"),
        ] {
            data(&mut s, &ins, multi, value(input));
            settle(&mut s);
            assert_eq!(
                segments(&s, id).binary(),
                if multi { "XXXXXXX" } else { discrete },
                "{multi} {input}"
            );
        }
    }
}
#[test]
fn display_control_priority_ignores_indeterminate_controls_and_holds_when_disabled() {
    use Bit::*;
    let (c, id, ins) = display(true, Facing::East);
    let mut s = Simulator::default();
    s.synchronize(&c);
    settle(&mut s);
    for (input, ctrl, want) in [
        ("0101", [Zero, One, One, One], "1101101"),
        ("0101", [Unknown, One, One, One], "1101101"),
        ("0101", [Error, One, One, One], "1101101"),
        ("0101", [One, Zero, One, One], "1111111"),
        ("0101", [One, Unknown, One, One], "1101101"),
        ("0101", [One, Error, One, One], "1101101"),
        ("0101", [One, One, Zero, One], "0000000"),
        ("0101", [One, One, Unknown, One], "1101101"),
        ("0101", [One, One, Error, One], "1101101"),
        ("0101", [One, One, One, Zero], "1101101"),
        ("0000", [One, One, One, Zero], "0000000"),
        ("0000", [One, One, One, Unknown], "0111111"),
        ("0000", [One, One, One, Error], "0111111"),
        ("XXXX", [One, Zero, Zero, Zero], "0000000"),
        ("EEEE", [One, Zero, Zero, Zero], "0000000"),
        ("XXXX", [One, Zero, One, Zero], "1111111"),
        ("EEEE", [One, Zero, One, Zero], "1111111"),
        ("XXXX", [One, One, One, Zero], "XXXXXXX"),
        ("EEEE", [One, One, One, Zero], "XXXXXXX"),
        ("XXXX", [Zero, Zero, Zero, Zero], "1101101"),
        ("XXXX", [Unknown, Zero, Zero, Zero], "0000000"),
        ("XXXX", [Error, Zero, Zero, Zero], "0000000"),
        ("0000", [One, Zero, Unknown, One], "1111111"),
        ("0000", [One, Zero, Error, One], "1111111"),
        ("0000", [One, Unknown, Zero, One], "0000000"),
        ("0000", [One, Error, Zero, One], "0000000"),
    ] {
        data(&mut s, &ins, true, Value::known(4, 5));
        controls(&mut s, &ins, [One; 4]);
        settle(&mut s);
        data(&mut s, &ins, true, value(input));
        controls(&mut s, &ins, ctrl);
        settle(&mut s);
        assert_eq!(segments(&s, id).binary(), want, "{input} {ctrl:?}");
    }
}
#[test]
fn held_segments_survive_source_and_format_edits_and_reset_to_unknown() {
    let (mut c, id, ins) = display(true, Facing::East);
    let mut s = Simulator::default();
    s.synchronize(&c);
    settle(&mut s);
    assert_eq!(segments(&s, id).binary(), "XXXXXXX");
    data(&mut s, &ins, true, Value::known(4, 5));
    controls(&mut s, &ins, [Bit::One; 4]);
    settle(&mut s);
    drive(&mut s, &ins, Enable, Value::known(1, 0));
    settle(&mut s);
    let source_id = ins.iter().find(|(role, _)| *role == Input(0)).unwrap().1;
    let source = c.components.iter_mut().find(|c| c.id == source_id).unwrap();
    source.kind = Kind::Constant;
    source.constant = 2;
    s.synchronize(&c);
    settle(&mut s);
    assert_eq!(segments(&s, id).binary(), "1101101");
    drive(&mut s, &ins, Enable, Value::known(1, 1));
    settle(&mut s);
    assert_eq!(segments(&s, id).binary(), "1011011");
    drive(&mut s, &ins, Enable, Value::known(1, 0));
    settle(&mut s);
    c.components
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .plexing
        .multibit = false;
    s.synchronize(&c);
    settle(&mut s);
    assert_eq!(segments(&s, id).binary(), "1011011");
    s.reset();
    settle(&mut s);
    assert_eq!(segments(&s, id).binary(), "XXXXXXX");
}

#[test]
fn display_instances_hold_independent_segments_across_navigation() {
    use logisim_core::Project;
    let mut project = Project::default();
    let sub = project.add("display".into()).unwrap();
    let (c, decoder, inputs) = display(true, Facing::East);
    project.circuit_mut(sub).unwrap().circuit = c;
    let mut instances = vec![];
    for x in [300, 700] {
        let instance = project.instance(sub, Point::new(x, 300)).unwrap();
        let mut data_pin = 0;
        let mut enable_pin = 0;
        for (role, pin) in &inputs {
            let index = instance
                .interface
                .ports
                .iter()
                .position(|p| p.pin == *pin)
                .unwrap();
            let at = instance.ports()[index];
            let mut source = Component::new(
                if matches!(role, Input(0) | Enable) {
                    Kind::Pin
                } else {
                    Kind::Constant
                },
                at,
            );
            source.bits = if *role == Input(0) { 4 } else { 1 };
            let id = project.main_circuit_mut().circuit.add_component(source);
            if *role == Input(0) {
                data_pin = id;
            }
            if *role == Enable {
                enable_pin = id;
            }
        }
        let id = project.main_circuit_mut().circuit.add_component(instance);
        instances.push((id, data_pin, enable_pin));
    }
    let mut s = ProjectSimulator::default();
    s.synchronize(&project, project.main);
    s.advance(256);
    for ((_, data, en), value) in instances.iter().zip([5, 9]) {
        s.set_input(*data, value);
        s.set_input(*en, 1);
    }
    s.advance(256);
    assert!(s.error().is_none());
    assert_eq!(
        segments(s.view(&[instances[0].0]).unwrap(), decoder).binary(),
        "1101101"
    );
    assert_eq!(
        segments(s.view(&[instances[1].0]).unwrap(), decoder).binary(),
        "1100111"
    );
    s.set_input(instances[0].2, 0);
    s.advance(256);
    for (_, data, _) in &instances {
        s.set_input(*data, 2);
    }
    s.advance(256);
    s.synchronize(&project, sub);
    s.advance(256);
    s.synchronize(&project, project.main);
    s.advance(256);
    assert_eq!(
        segments(s.view(&[instances[0].0]).unwrap(), decoder).binary(),
        "1101101"
    );
    assert_eq!(
        segments(s.view(&[instances[1].0]).unwrap(), decoder).binary(),
        "1011011"
    );
}
