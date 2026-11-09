use super::super::*;
use super::support::{add, driven, instance, link};
use crate::ProjectSimulator;
use logisim_core::{Circuit, Gate, MemoryContents, MemoryRole, Point, Project};

fn ram(c: &mut Circuit, bits: u8, interface: MemoryInterface) -> Id {
    let id = add(c, Kind::Ram, bits);
    let mut component = c.component(id).unwrap().clone();
    component.memory.address_bits = 2;
    component.memory.interface = interface;
    component.memory.simple = false;
    c.replace_component(component);
    id
}
fn port(c: &Circuit, id: Id, role: MemoryRole) -> Port {
    (id, c.component(id).unwrap().memory_port(role).unwrap())
}
fn drive(c: &mut Circuit, id: Id, role: MemoryRole) -> Id {
    driven(c, port(c, id, role))
}
fn constant(c: &mut Circuit, target: Port, value: u32) {
    let id = add(
        c,
        Kind::Constant,
        c.component(target.0).unwrap().port_width(target.1).unwrap(),
    );
    c.components
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .constant = value;
    link(c, (id, 0), target);
}
fn controls(c: &mut Circuit, id: Id) {
    for (role, value) in [
        (MemoryRole::Address, 0),
        (MemoryRole::Select, 0),
        (MemoryRole::Store, 1),
        (MemoryRole::Clear, 0),
    ] {
        constant(c, port(c, id, role), value);
    }
}
fn idle(sim: &mut Simulator) {
    let report = sim.settle_compiled().unwrap();
    assert_eq!(
        (
            report.evaluated_memories,
            report.sampled_components,
            report.evaluated_instructions
        ),
        (0, 0, 0)
    );
}

#[test]
fn ram_buffer_ram_register_chain_samples_old_words_independent_of_order_and_depth() {
    for reverse in [false, true] {
        for depth in [0, 8] {
            let mut c = Circuit::default();
            let a = ram(&mut c, 4, MemoryInterface::Separate);
            let b = ram(&mut c, 4, MemoryInterface::Separate);
            controls(&mut c, a);
            controls(&mut c, b);
            let clk = drive(&mut c, a, MemoryRole::Clock);
            let data = drive(&mut c, a, MemoryRole::Input);
            let target = port(&c, b, MemoryRole::Clock);
            link(&mut c, (clk, 0), target);
            let mut previous = a;
            for _ in 0..depth {
                let gate = add(&mut c, Kind::Gate(Gate::Buffer), 4);
                link(&mut c, (previous, 0), (gate, 1));
                previous = gate;
            }
            let target = port(&c, b, MemoryRole::Input);
            link(&mut c, (previous, 0), target);
            let reg = add(&mut c, Kind::Register, 4);
            link(&mut c, (b, 0), (reg, 1));
            link(&mut c, (clk, 0), (reg, 2));
            for (i, v) in [(3, 1), (4, 0), (5, 0), (6, 0)] {
                constant(&mut c, (reg, i), v);
            }
            if reverse {
                c.components.reverse();
            }
            let mut sim = Simulator::default();
            sim.synchronize(&c);
            sim.settle_compiled().unwrap();
            sim.set_input(data, 5);
            assert_eq!(sim.settle_compiled().unwrap().evaluated_memories, 0);
            for (expected_b, expected_reg) in [(0, 0), (5, 0), (5, 5)] {
                sim.set_input(clk, 1);
                sim.settle_compiled().unwrap();
                assert_eq!(sim.memory_contents(a).map(|m| m.get(0)), Some(5));
                assert_eq!(sim.memory_contents(b).map(|m| m.get(0)), Some(expected_b));
                assert_eq!(sim.port_value(a, 0), Some(Value::known(4, 5)));
                assert_eq!(sim.port_value(b, 0), Some(Value::known(4, expected_b)));
                assert_eq!(sim.stored_value(reg), Some(Value::known(4, expected_reg)));
                idle(&mut sim);
                sim.set_input(clk, 0);
                sim.settle_compiled().unwrap();
            }
            sim.set_input(data, 7);
            sim.advance(128);
            sim.set_input(clk, 1);
            sim.advance(128);
            sim.settle_compiled().unwrap();
            assert_eq!(sim.memory_contents(a).map(|m| m.get(0)), Some(7));
            assert_eq!(sim.memory_contents(b).map(|m| m.get(0)), Some(5));
        }
    }
}

#[test]
fn synchronous_ram_four_state_matrix_matches_event_engine_when_inputs_precede_clock() {
    let mut seed = 0x92fa_42b1u32;
    for interface in [MemoryInterface::Separate, MemoryInterface::SharedSync] {
        for simple in [false, true] {
            for select_high in [false, true] {
                for bits in [1, 4, 32] {
                    let mut c = Circuit::default();
                    let id = ram(&mut c, bits, interface);
                    let comp = c.components.iter_mut().find(|c| c.id == id).unwrap();
                    comp.memory.simple = simple;
                    comp.memory.select_high = select_high;
                    let terminals = comp.memory_terminals();
                    let pins: Vec<_> = terminals
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| interface != MemoryInterface::Separate || *i != 0)
                        .map(|(i, t)| (t.role, driven(&mut c, (id, i)), t.bits))
                        .collect();
                    let clk = pins.iter().find(|p| p.0 == MemoryRole::Clock).unwrap().1;
                    let mut slow = Simulator::default();
                    let mut fast = Simulator::default();
                    slow.synchronize(&c);
                    fast.synchronize(&c);
                    slow.advance(256);
                    fast.settle_compiled().unwrap();
                    for round in 0..96 {
                        for &(role, pin, width) in &pins {
                            if role == MemoryRole::Clock {
                                continue;
                            }
                            seed ^= seed << 13;
                            seed ^= seed >> 17;
                            seed ^= seed << 5;
                            let value = match seed % 6 {
                                0 => Value::filled(width, Bit::Unknown),
                                1 => Value::filled(width, Bit::Error),
                                _ => Value::known(width, seed >> 3),
                            };
                            slow.set_input_value(pin, value);
                            fast.set_input_value(pin, value);
                        }
                        slow.advance(256);
                        fast.settle_compiled().unwrap();
                        slow.set_input(clk, round % 2);
                        fast.set_input(clk, round % 2);
                        slow.advance(256);
                        fast.settle_compiled().unwrap();
                        assert_eq!(
                            fast.memory_contents(id),
                            slow.memory_contents(id),
                            "{interface:?} {simple} {select_high} {bits} {round}"
                        );
                        assert_eq!(fast.drive_value(id, 0), slow.drive_value(id, 0));
                        assert_eq!(fast.port_value(id, 0), slow.port_value(id, 0));
                        idle(&mut fast);
                    }
                }
            }
        }
    }
}

#[test]
fn read_cache_tracks_manual_images_reset_and_electrical_edits_without_sampling_edges() {
    let mut c = Circuit::default();
    let id = ram(&mut c, 32, MemoryInterface::Separate);
    let clk = drive(&mut c, id, MemoryRole::Clock);
    let address = drive(&mut c, id, MemoryRole::Address);
    let data = drive(&mut c, id, MemoryRole::Input);
    let store = drive(&mut c, id, MemoryRole::Store);
    let clear = drive(&mut c, id, MemoryRole::Clear);
    let sel = drive(&mut c, id, MemoryRole::Select);
    let mut sim = Simulator::default();
    sim.synchronize(&c);
    sim.settle_compiled().unwrap();
    sim.set_input(data, 7);
    sim.set_input(store, 0);
    sim.set_input(clk, 1);
    assert_eq!(sim.settle_compiled().unwrap().evaluated_memories, 0);
    sim.set_input(store, 1);
    sim.settle_compiled().unwrap();
    assert_eq!(sim.memory_contents(id).map(|m| m.get(0)), Some(0)); // high level is not another edge
    sim.fill_memory(id, 0, 4, 0x8000_0001);
    sim.settle_compiled().unwrap();
    assert_eq!(sim.drive_value(id, 0), Some(Value::known(32, 0x8000_0001)));
    sim.set_memory_word(id, 1, 19);
    sim.set_input(address, 1);
    sim.settle_compiled().unwrap();
    assert_eq!(sim.drive_value(id, 0), Some(Value::known(32, 19)));
    let mut contents = MemoryContents::default();
    contents.set(1, 23);
    assert!(sim.replace_memory(id, contents));
    sim.settle_compiled().unwrap();
    assert_eq!(sim.drive_value(id, 0), Some(Value::known(32, 23)));
    sim.clear_memory(id);
    sim.settle_compiled().unwrap();
    assert_eq!(sim.drive_value(id, 0), Some(Value::known(32, 0)));
    sim.set_memory_word(id, 1, 31);
    sim.settle_compiled().unwrap();
    sim.set_input(sel, 1);
    sim.set_input(clear, 1);
    sim.settle_compiled().unwrap();
    assert_eq!(sim.memory_contents(id).map(|m| m.get(1)), Some(0));
    assert_eq!(
        sim.drive_value(id, 0),
        Some(Value::filled(32, Bit::Unknown))
    );
    sim.set_input(clear, 0);
    sim.set_input(sel, 0);
    sim.settle_compiled().unwrap();
    assert_eq!(sim.memory_contents(id).map(|m| m.get(1)), Some(0));
    sim.set_memory_word(id, 1, 31);
    sim.settle_compiled().unwrap();
    let mut changed = c.component(id).unwrap().clone();
    changed.label = "renamed".into();
    c.replace_component(changed);
    assert!(!sim.synchronize(&c));
    idle(&mut sim);
    let mut changed = c.component(id).unwrap().clone();
    changed.bits = 1;
    c.replace_component(changed);
    // Resize the isolated input pin too, keeping the circuit's bus widths valid.
    c.components.iter_mut().find(|c| c.id == data).unwrap().bits = 1;
    for t in c
        .components
        .iter_mut()
        .filter(|c| c.kind == Kind::Tunnel && c.label == format!("{data}:0"))
    {
        t.bits = 1;
    }
    assert!(sim.synchronize(&c));
    sim.settle_compiled().unwrap();
    assert_eq!(sim.memory_contents(id).map(|m| m.get(1)), Some(1));
    sim.reset();
    sim.settle_compiled().unwrap();
    assert_eq!(sim.memory_contents(id).map(|m| m.get(1)), Some(0));
    idle(&mut sim);
}

#[test]
fn ram_read_feedback_is_rejected_but_stored_data_feedback_is_allowed() {
    for role in [MemoryRole::Address, MemoryRole::Select, MemoryRole::Input] {
        let mut c = Circuit::default();
        let id = ram(
            &mut c,
            if role == MemoryRole::Address { 2 } else { 1 },
            MemoryInterface::Separate,
        );
        let target = port(&c, id, role);
        link(&mut c, (id, 0), target);
        let mut sim = Simulator::default();
        sim.synchronize(&c);
        let before = (
            sim.steps(),
            sim.pending_count(),
            sim.memory_contents(id).cloned(),
        );
        if role == MemoryRole::Input {
            sim.settle_compiled().unwrap();
        } else {
            assert!(matches!(
                sim.settle_compiled().unwrap_err().reason,
                Failure::Topology {
                    reason: BoundaryReason::Feedback,
                    ..
                }
            ));
            assert_eq!(
                before,
                (
                    sim.steps(),
                    sim.pending_count(),
                    sim.memory_contents(id).cloned()
                )
            );
        }
    }
}

#[test]
fn sibling_ram_instances_share_the_sampling_boundary_and_keep_independent_images() {
    let mut p = Project::default();
    let root = p.main;
    let child = p.add("RAM".into()).unwrap();
    let c = &mut p.circuit_mut(child).unwrap().circuit;
    let ram = ram(c, 4, MemoryInterface::Separate);
    controls(c, ram);
    let clk = drive(c, ram, MemoryRole::Clock);
    let data = drive(c, ram, MemoryRole::Input);
    let out = add(c, Kind::Pin, 4);
    c.components
        .iter_mut()
        .find(|c| c.id == out)
        .unwrap()
        .output = true;
    link(c, (ram, 0), (out, 0));
    let (a, ports) = instance(&mut p, child, root, Point::new(300, 1000));
    let (b, _) = instance(&mut p, child, root, Point::new(900, 1000));
    let index = |pin| ports.iter().find(|p| p.0 == pin).unwrap().1;
    let c = &mut p.main_circuit_mut().circuit;
    let clock = driven(c, (a, index(clk)));
    link(c, (clock, 0), (b, index(clk)));
    let d = driven(c, (a, index(data)));
    link(c, (a, index(out)), (b, index(data)));
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&p, root);
    sim.settle_compiled().unwrap();
    sim.set_input(d, 5);
    for expected in [0, 5] {
        sim.set_input(clock, 1);
        sim.settle_compiled().unwrap();
        assert_eq!(
            sim.view(&[a])
                .unwrap()
                .memory_contents(ram)
                .map(|m| m.get(0)),
            Some(5)
        );
        assert_eq!(
            sim.view(&[b])
                .unwrap()
                .memory_contents(ram)
                .map(|m| m.get(0)),
            Some(expected)
        );
        let report = sim.settle_compiled().unwrap();
        assert_eq!(
            (report.evaluated_memories, report.sampled_components),
            (0, 0)
        );
        sim.set_input(clock, 0);
        sim.settle_compiled().unwrap();
    }
    sim.set_memory_word(&[a], ram, 0, 7);
    sim.settle_compiled().unwrap();
    assert_eq!(
        sim.view(&[b])
            .unwrap()
            .memory_contents(ram)
            .map(|m| m.get(0)),
        Some(5)
    );
    sim.reset();
    sim.settle_compiled().unwrap();
    assert_eq!(
        sim.view(&[a])
            .unwrap()
            .memory_contents(ram)
            .map(|m| m.get(0)),
        Some(0)
    );
    assert_eq!(
        sim.view(&[b])
            .unwrap()
            .memory_contents(ram)
            .map(|m| m.get(0)),
        Some(0)
    );
}

#[test]
fn shared_bus_hidden_writes_at_last_address_survive_cached_reads_and_repeated_settles() {
    let mut c = Circuit::default();
    let id = ram(&mut c, 32, MemoryInterface::SharedSync);
    c.components
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .memory
        .address_bits = 24;
    let address = drive(&mut c, id, MemoryRole::Address);
    let data = drive(&mut c, id, MemoryRole::Data);
    let clk = drive(&mut c, id, MemoryRole::Clock);
    let load = drive(&mut c, id, MemoryRole::Load);
    let select = port(&c, id, MemoryRole::Select);
    constant(&mut c, select, 0);
    let clear = port(&c, id, MemoryRole::Clear);
    constant(&mut c, clear, 0);
    let mut sim = Simulator::default();
    sim.synchronize(&c);
    sim.settle_compiled().unwrap();
    sim.set_input(address, 0xff_ffff);
    sim.set_input(data, 0x8000_0001);
    sim.set_input(clk, 1);
    let report = sim.settle_compiled().unwrap();
    assert!(report.evaluated_memories > 0);
    assert_eq!(
        sim.drive_value(id, 0),
        Some(Value::filled(32, Bit::Unknown))
    );
    assert_eq!(sim.memory_contents(id).unwrap().get(0xff_ffff), 0x8000_0001);
    idle(&mut sim);
    sim.set_input(data, 17);
    sim.settle_compiled().unwrap();
    assert_eq!(sim.memory_contents(id).unwrap().get(0xff_ffff), 0x8000_0001);
    sim.set_input(load, 1);
    sim.set_input_value(data, Value::filled(32, Bit::Unknown));
    sim.settle_compiled().unwrap();
    assert_eq!(sim.port_value(id, 0), Some(Value::known(32, 0x8000_0001)));
    assert_eq!(sim.memory_contents(id).unwrap().boundaries().count(), 2);
}

#[test]
fn ram_read_feedback_crossing_a_subcircuit_boundary_is_rejected_before_advancing() {
    let mut p = Project::default();
    let root = p.main;
    let child = p.add("read feedback".into()).unwrap();
    let c = &mut p.circuit_mut(child).unwrap().circuit;
    let ram = ram(c, 2, MemoryInterface::Separate);
    let address = drive(c, ram, MemoryRole::Address);
    let out = add(c, Kind::Pin, 2);
    c.components
        .iter_mut()
        .find(|c| c.id == out)
        .unwrap()
        .output = true;
    link(c, (ram, 0), (out, 0));
    let (id, ports) = instance(&mut p, child, root, Point::new(300, 1000));
    let index = |pin| ports.iter().find(|p| p.0 == pin).unwrap().1;
    link(
        &mut p.main_circuit_mut().circuit,
        (id, index(out)),
        (id, index(address)),
    );
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&p, root);
    let before = (sim.steps(), sim.pending_count());
    assert!(matches!(
        sim.settle_compiled().unwrap_err().reason,
        Failure::Topology {
            reason: BoundaryReason::Feedback,
            ..
        }
    ));
    assert_eq!((sim.steps(), sim.pending_count()), before);
    assert_eq!(
        sim.view(&[id])
            .unwrap()
            .memory_contents(ram)
            .unwrap()
            .get(0),
        0
    );
}
