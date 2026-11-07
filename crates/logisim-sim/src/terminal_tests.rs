use crate::{Bit, ProjectSimulator, Simulator, Value};
use logisim_core::{Circuit, Component, Id, Kind, Point, Project, Trigger};
fn settle(s: &mut Simulator) {
    s.advance(128);
    assert_eq!(s.pending_count(), 0);
    assert!(!s.limited());
}
fn fixture(kind: Kind, trigger: Trigger, bits: u8) -> (Circuit, Id, Vec<Id>, Simulator) {
    let mut c = Circuit::default();
    let mut terminal = Component::new(kind, Point::new(300, 300));
    terminal.bits = bits;
    terminal.terminal.trigger = trigger;
    terminal.terminal.buffer_len = 5;
    terminal.terminal.rows = 2;
    terminal.terminal.columns = 4;
    let ports = terminal.ports();
    let id = c.add_component(terminal.clone());
    let start = if kind == Kind::Keyboard { 2 } else { 0 };
    let inputs = (start..ports.len())
        .map(|i| {
            let mut p = Component::new(Kind::Pin, ports[i]);
            p.bits = terminal.port_width(i).unwrap();
            c.add_component(p)
        })
        .collect::<Vec<_>>();
    let mut s = Simulator::default();
    s.synchronize(&c);
    settle(&mut s);
    (c, id, inputs, s)
}
fn inject(s: &mut Simulator, id: Id, value: Value) {
    s.inputs.insert(id, value);
    s.schedule(id, value);
    settle(s);
}
fn pulse(s: &mut Simulator, id: Id) {
    s.set_input(id, 1);
    settle(s);
    s.set_input(id, 0);
    settle(s);
}
#[test]
fn keyboard_fifo_capacity_encoding_controls_and_both_clock_edges() {
    for bits in [7, 16] {
        for trigger in [Trigger::Rising, Trigger::Falling] {
            let (_, id, p, mut s) = fixture(Kind::Keyboard, trigger, bits);
            for value in [0xe9, 0x4e2d, 0xf600, 10, 8, 65] {
                s.type_character(id, value);
            }
            settle(&mut s);
            assert_eq!(
                s.keyboard_buffer(id).unwrap(),
                vec![0xe9, 0x4e2d, 0xf600, 10, 8]
            );
            assert_eq!(s.port_value(id, 0), Some(Value::known(bits, 0xe9)));
            pulse(&mut s, p[0]);
            assert_eq!(s.keyboard_buffer(id).unwrap().len(), 5); // EN=0
            for enable in [Bit::One, Bit::Unknown, Bit::Error] {
                inject(&mut s, p[1], Value::filled(1, enable));
                let before = s.keyboard_buffer(id).unwrap();
                s.set_input(p[0], 1);
                settle(&mut s);
                assert_eq!(
                    s.keyboard_buffer(id).unwrap().len(),
                    before.len() - usize::from(trigger == Trigger::Rising)
                );
                s.set_input(p[0], 0);
                settle(&mut s);
                assert_eq!(s.keyboard_buffer(id).unwrap(), before[1..]);
            }
            inject(&mut s, p[2], Value::filled(1, Bit::Error));
            assert_eq!(s.keyboard_buffer(id).unwrap(), vec![10, 8]);
            inject(&mut s, p[2], Value::filled(1, Bit::Unknown));
            assert_eq!(s.keyboard_buffer(id).unwrap(), vec![10, 8]);
            s.set_input(p[2], 1);
            settle(&mut s);
            s.type_character(id, 65);
            settle(&mut s);
            assert!(s.keyboard_buffer(id).unwrap().is_empty());
            assert_eq!(s.port_value(id, 0), Some(Value::known(bits, 0)));
            assert_eq!(s.port_value(id, 1), Some(Value::known(1, 0)));
        }
    }
}
#[test]
fn terminals_ignore_unknown_clock_transitions_and_accept_only_exact_edges() {
    for kind in [Kind::Keyboard, Kind::Tty] {
        for trigger in [Trigger::Rising, Trigger::Falling] {
            let (_, id, p, mut s) = fixture(kind, trigger, 7);
            let (clock, enable) = if kind == Kind::Keyboard {
                (p[0], p[1])
            } else {
                (p[1], p[2])
            };
            if kind == Kind::Keyboard {
                s.type_character(id, 65);
                s.type_character(id, 66);
            } else {
                s.set_input(p[0], 65);
            }
            s.set_input(enable, 1);
            settle(&mut s);
            for bit in [
                Bit::Unknown,
                Bit::One,
                Bit::Error,
                Bit::Zero,
                Bit::Unknown,
                Bit::Zero,
                Bit::Error,
                Bit::One,
            ] {
                inject(&mut s, clock, Value::filled(1, bit));
                if kind == Kind::Keyboard {
                    assert_eq!(s.keyboard_buffer(id).unwrap(), vec![65, 66]);
                } else {
                    assert!(s.tty(id).unwrap().lines.iter().all(Vec::is_empty));
                }
            }
        }
    }
}
#[test]
fn tty_wrap_scroll_control_characters_utf16_and_unknown_data() {
    let (_, id, p, mut s) = fixture(Kind::Tty, Trigger::Rising, 16);
    s.set_input(p[2], 1);
    settle(&mut s);
    let write = |s: &mut Simulator, v| {
        s.set_input(p[0], v);
        settle(s);
        pulse(s, p[1]);
    };
    for ch in b"ABCDEFGH" {
        write(&mut s, u32::from(*ch));
    }
    assert_eq!(
        s.tty(id).unwrap().lines,
        vec![vec![65, 66, 67, 68], vec![69, 70, 71, 72]]
    );
    assert_eq!((s.tty(id).unwrap().row, s.tty(id).unwrap().column), (1, 4));
    write(&mut s, 73);
    assert_eq!(
        s.tty(id).unwrap().lines,
        vec![vec![69, 70, 71, 72], vec![73]]
    );
    write(&mut s, 8);
    write(&mut s, 8);
    assert_eq!(s.tty(id).unwrap().lines, vec![vec![69, 70, 71, 72], vec![]]);
    write(&mut s, 13);
    assert!(s.tty(id).unwrap().lines.iter().all(Vec::is_empty));
    write(&mut s, 0x4e2d);
    write(&mut s, 0xd83d);
    write(&mut s, 0xde00);
    assert_eq!(
        String::from_utf16(&s.tty(id).unwrap().lines[1]).unwrap(),
        "中😀"
    );
    for v in (0..=31)
        .chain(127..=159)
        .filter(|v| ![8, 10, 12, 13].contains(v))
    {
        write(&mut s, v);
    }
    assert_eq!(s.tty(id).unwrap().column, 3);
    inject(&mut s, p[0], Value::filled(16, Bit::Unknown));
    pulse(&mut s, p[1]);
    assert_eq!(
        s.tty(id).unwrap().lines[1],
        vec![0x4e2d, 0xd83d, 0xde00, 63]
    );
    inject(
        &mut s,
        p[0],
        Value::from_bits([Bit::Error].into_iter().chain([Bit::Zero; 15])),
    );
    pulse(&mut s, p[1]);
    assert_eq!(s.tty(id).unwrap().lines[1], vec![63]);
    write(&mut s, 12);
    assert_eq!((s.tty(id).unwrap().row, s.tty(id).unwrap().column), (0, 0));
    assert!(s.tty(id).unwrap().lines.iter().all(Vec::is_empty));
}
#[test]
fn tty_enable_clear_priority_pause_and_reset() {
    let (_, id, p, mut s) = fixture(Kind::Tty, Trigger::Falling, 7);
    s.set_input(p[0], 65);
    settle(&mut s);
    pulse(&mut s, p[1]);
    assert!(s.tty(id).unwrap().lines[0].is_empty());
    for bit in [Bit::One, Bit::Unknown, Bit::Error] {
        inject(&mut s, p[2], Value::filled(1, bit));
        pulse(&mut s, p[1]);
    }
    assert_eq!(s.tty(id).unwrap().lines[0], vec![65; 3]);
    for bit in [Bit::Unknown, Bit::Error] {
        inject(&mut s, p[3], Value::filled(1, bit));
        assert_eq!(s.tty(id).unwrap().lines[0], vec![65; 3]);
    }
    s.set_input(p[3], 1);
    settle(&mut s);
    pulse(&mut s, p[1]);
    assert!(s.tty(id).unwrap().lines[0].is_empty());
    s.set_input(p[3], 0);
    settle(&mut s);
    s.set_enabled(false);
    s.set_input(p[1], 1);
    s.advance(128);
    assert!(s.tty(id).unwrap().lines[0].is_empty());
    s.step();
    s.set_input(p[1], 0);
    s.step();
    assert_eq!(s.tty(id).unwrap().lines[0], vec![65]);
    s.reset();
    assert!(s.tty(id).unwrap().lines.iter().all(Vec::is_empty));
}
#[test]
fn state_survives_edits_but_not_reset_or_kind_replacement() {
    let (mut c, id, _, mut s) = fixture(Kind::Keyboard, Trigger::Rising, 7);
    for v in [65, 66, 67] {
        s.type_character(id, v);
    }
    settle(&mut s);
    c.components.first_mut().unwrap().terminal.buffer_len = 1;
    c.components.first_mut().unwrap().bits = 16;
    s.synchronize(&c);
    settle(&mut s);
    assert_eq!(s.keyboard_buffer(id).unwrap(), vec![65]);
    assert_eq!(s.port_value(id, 0), Some(Value::known(16, 65)));
    s.set_enabled(false);
    s.type_character(id, 66);
    assert_eq!(s.keyboard_buffer(id).unwrap(), vec![65]);
    s.reset();
    assert!(s.keyboard_buffer(id).unwrap().is_empty());
    c.components.retain(|c| c.id != id);
    s.synchronize(&c);
    assert!(s.keyboard_buffer(id).is_none());
}
#[test]
fn keyboard_instances_and_cached_roots_keep_separate_buffers() {
    let mut p = Project::default();
    let root = p.main;
    let child = p.add("characters".into()).unwrap();
    let k = p
        .circuit_mut(child)
        .unwrap()
        .circuit
        .add_component(Component::new(Kind::Keyboard, Point::ZERO));
    let ids: Vec<_> = [0, 300]
        .into_iter()
        .map(|x| {
            p.circuit_mut(root)
                .unwrap()
                .circuit
                .add_component(Component::new(Kind::Subcircuit(child), Point::new(x, 200)))
        })
        .collect();
    p.refresh_interfaces(false);
    let mut s = ProjectSimulator::default();
    s.synchronize(&p, root);
    s.advance(128);
    s.type_character(&[ids[0]], k, 65);
    s.advance(128);
    assert_eq!(
        s.view(&[ids[0]]).unwrap().keyboard_buffer(k).unwrap(),
        vec![65]
    );
    assert!(
        s.view(&[ids[1]])
            .unwrap()
            .keyboard_buffer(k)
            .unwrap()
            .is_empty()
    );
    s.synchronize(&p, child);
    s.type_character(&[], k, 66);
    s.advance(128);
    s.synchronize(&p, root);
    s.advance(128);
    assert_eq!(
        s.view(&[ids[0]]).unwrap().keyboard_buffer(k).unwrap(),
        vec![65]
    );
    s.reset();
    s.advance(128);
    assert!(
        s.view(&[ids[0]])
            .unwrap()
            .keyboard_buffer(k)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn same_clock_tty_samples_keyboard_front_before_consumption() {
    let (mut c, k, p, mut s) = fixture(Kind::Keyboard, Trigger::Rising, 7);
    let mut tty = Component::new(Kind::Tty, Point::new(440, 320));
    tty.terminal.rows = 2;
    tty.terminal.columns = 4;
    let t = c.add_component(tty);
    c.add_wire(Point::new(300, 300), Point::new(300, 320));
    c.add_wire(Point::new(300, 320), Point::new(440, 320));
    s.synchronize(&c);
    settle(&mut s);
    s.set_input(p[1], 1);
    s.type_character(k, 65);
    s.type_character(k, 66);
    settle(&mut s);
    pulse(&mut s, p[0]);
    assert_eq!(s.keyboard_buffer(k).unwrap(), vec![66]);
    assert_eq!(s.tty(t).unwrap().lines[0], vec![65]);
    pulse(&mut s, p[0]);
    assert!(s.keyboard_buffer(k).unwrap().is_empty());
    assert_eq!(s.tty(t).unwrap().lines[0], vec![65, 66]);
}
#[test]
fn terminal_resize_keeps_visible_tail_and_colors_do_not_reset_screen() {
    let (mut c, id, p, mut s) = fixture(Kind::Tty, Trigger::Rising, 7);
    s.set_input(p[2], 1);
    settle(&mut s);
    for v in b"ABCDEFGH" {
        s.set_input(p[0], u32::from(*v));
        settle(&mut s);
        pulse(&mut s, p[1]);
    }
    c.components[0].terminal.rows = 1;
    c.components[0].terminal.columns = 2;
    s.synchronize(&c);
    settle(&mut s);
    assert_eq!(s.tty(id).unwrap().lines, vec![vec![69, 70]]);
    assert_eq!((s.tty(id).unwrap().row, s.tty(id).unwrap().column), (0, 2));
    c.components[0].terminal.rows = 4;
    c.components[0].terminal.columns = 8;
    s.synchronize(&c);
    settle(&mut s);
    s.set_input(p[0], 73);
    settle(&mut s);
    pulse(&mut s, p[1]);
    assert_eq!(s.tty(id).unwrap().lines[0], vec![69, 70, 73]);
    let before = s.tty(id).unwrap().clone();
    c.components[0].io.on_color = [1, 2, 3, 128];
    assert!(!s.synchronize(&c));
    assert_eq!(s.tty(id), Some(&before));
}

#[test]
fn tty_screens_follow_independent_instance_clocks_and_reset() {
    let mut p = Project::default();
    let root = p.main;
    let child = p.add("terminal".into()).unwrap();
    let c = &mut p.circuit_mut(child).unwrap().circuit;
    let k = c.add_component(Component::new(Kind::Keyboard, Point::new(200, 200)));
    let t = c.add_component(Component::new(Kind::Tty, Point::new(340, 220)));
    let clock = c.add_component(Component::new(Kind::Button, Point::new(200, 200)));
    c.add_wire(Point::new(200, 200), Point::new(200, 220));
    c.add_wire(Point::new(200, 220), Point::new(340, 220));
    let mut ids = vec![];
    for x in [100, 400] {
        let instance = p.instance(child, Point::new(x, 100)).unwrap();
        ids.push(p.circuit_mut(root).unwrap().circuit.add_component(instance));
    }
    let mut s = ProjectSimulator::default();
    s.synchronize(&p, root);
    s.advance(128);
    s.type_character(&[ids[0]], k, 65);
    s.type_character(&[ids[1]], k, 66);
    s.advance(128);
    s.set_button(&[ids[0]], clock, true);
    s.advance(128);
    assert_eq!(
        s.view(&[ids[0]]).unwrap().tty(t).unwrap().lines[0],
        vec![65]
    );
    assert!(s.view(&[ids[1]]).unwrap().tty(t).unwrap().lines[0].is_empty());
    assert_eq!(
        s.view(&[ids[1]]).unwrap().keyboard_buffer(k).unwrap(),
        vec![66]
    );
    s.set_button(&[ids[1]], clock, true);
    s.advance(128);
    assert_eq!(
        s.view(&[ids[1]]).unwrap().tty(t).unwrap().lines[0],
        vec![66]
    );
    s.reset();
    s.advance(128);
    for id in ids {
        assert!(
            s.view(&[id])
                .unwrap()
                .tty(t)
                .unwrap()
                .lines
                .iter()
                .all(Vec::is_empty)
        );
    }
}
