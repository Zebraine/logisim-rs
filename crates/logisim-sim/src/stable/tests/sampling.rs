use crate::stable::*;
use logisim_core::{Circuit, FlipFlop};

use super::support::*;

#[test]
fn same_edge_chain_reads_old_state_in_both_document_orders() {
    for reverse in [false, true] {
        let mut c = Circuit::default();
        let a = add(&mut c, Kind::FlipFlop(FlipFlop::D), 1);
        let b = add(&mut c, Kind::FlipFlop(FlipFlop::D), 1);
        let clock = driven(&mut c, (a, 2));
        link(&mut c, (clock, 0), (b, 2));
        let data = driven(&mut c, (a, 3));
        link(&mut c, (a, 0), (b, 3));
        if reverse {
            c.components.reverse();
        }
        let mut sim = Simulator::default();
        sim.synchronize(&c);
        sim.set_enabled(false);
        sim.settle_compiled().unwrap();
        sim.set_input(data, 1);
        sim.set_input(clock, 1);
        sim.settle_compiled().unwrap();
        assert_eq!((stored(&sim, a), stored(&sim, b)), (1, 0));
        sim.set_input(clock, 0);
        sim.settle_compiled().unwrap();
        sim.set_input(clock, 1);
        sim.settle_compiled().unwrap();
        assert_eq!((stored(&sim, a), stored(&sim, b)), (1, 1));
        assert!(!sim.enabled());
        assert_eq!(sim.pending_count(), 0);
        let mut renamed = c.component(a).unwrap().clone();
        renamed.label = "renamed state".into();
        c.replace_component(renamed);
        assert!(!sim.synchronize(&c));
        sim.settle_compiled().unwrap();
        assert_eq!((stored(&sim, a), stored(&sim, b)), (1, 1));
    }
}

#[test]
fn stable_sampling_ignores_combinational_depth_but_step_preserves_old_batches() {
    for depth in [1, 2, 32] {
        let mut c = Circuit::default();
        let ff = add(&mut c, Kind::FlipFlop(FlipFlop::D), 1);
        let clock = add(&mut c, Kind::Pin, 1);
        let data = add(&mut c, Kind::Pin, 1);
        let clk_out = chain(&mut c, clock, 1);
        let data_out = chain(&mut c, data, depth);
        link(&mut c, (clk_out, 0), (ff, 2));
        link(&mut c, (data_out, 0), (ff, 3));
        for compiled in [false, true] {
            let mut sim = Simulator::default();
            sim.synchronize(&c);
            sim.advance(256);
            sim.set_input(data, 1);
            sim.set_input(clock, 1);
            if compiled {
                sim.settle_compiled().unwrap();
            } else {
                sim.advance(256);
            }
            assert_eq!(stored(&sim, ff), u32::from(compiled || depth == 1));
            assert_eq!(sim.port_value(ff, 3), Some(Value::known(1, 1)));
            assert_eq!(sim.pending_count(), 0);
            // Resume exact stepping after a compiled settle, without stale pending.
            sim.set_input(clock, 0);
            sim.advance(256);
            sim.set_input(data, 0);
            sim.advance(256);
            sim.set_input(clock, 1);
            sim.advance(256);
            assert_eq!(stored(&sim, ff), 0);
        }
    }
}

#[test]
fn asynchronous_controls_and_four_state_register_data_match_settled_reference() {
    for trigger in [Trigger::Rising, Trigger::Falling] {
        for width in [1, 4, 32] {
            let mut c = Circuit::default();
            let reg = add(&mut c, Kind::Register, width);
            let mut comp = c.component(reg).unwrap().clone();
            comp.sequential.trigger = trigger;
            c.replace_component(comp);
            let pins: Vec<_> = (1..7).map(|p| driven(&mut c, (reg, p))).collect();
            let mut slow = Simulator::default();
            let mut fast = Simulator::default();
            slow.synchronize(&c);
            fast.synchronize(&c);
            let mut seed = 0x98ab_7123u32;
            for i in 0..128 {
                for (port, &pin) in pins.iter().enumerate() {
                    // Set data/async controls first, then independently apply a clock
                    // transition: both models share this explicit stimulus ordering.
                    if port == 1 {
                        continue;
                    }
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    let bits = c.component(pin).unwrap().bits;
                    let value = if port == 0 && i % 4 >= 2 {
                        Value::filled(bits, if i % 2 == 0 { Bit::Unknown } else { Bit::Error })
                    } else {
                        Value::known(bits, seed)
                    };
                    slow.external_input(pin, value);
                    fast.external_input(pin, value);
                }
                slow.advance(128);
                fast.settle_compiled().unwrap();
                for sim in [&mut slow, &mut fast] {
                    sim.set_input(pins[1], i & 1);
                }
                slow.advance(128);
                fast.settle_compiled().unwrap();
                assert_eq!(fast.stored_value(reg), slow.stored_value(reg));
                for port in 0..7 {
                    assert_eq!(fast.port_value(reg, port), slow.port_value(reg, port));
                }
            }
        }
    }
}

#[test]
fn supported_edge_state_families_keep_transitions_and_clock_ticks() {
    for kind in [
        Kind::FlipFlop(FlipFlop::D),
        Kind::FlipFlop(FlipFlop::T),
        Kind::FlipFlop(FlipFlop::Jk),
        Kind::FlipFlop(FlipFlop::Sr),
        Kind::Register,
        Kind::Counter,
        Kind::ShiftRegister,
        Kind::Random,
    ] {
        for trigger in [Trigger::Rising, Trigger::Falling] {
            let mut c = Circuit::default();
            let id = add(
                &mut c,
                kind,
                if matches!(kind, Kind::FlipFlop(_)) {
                    1
                } else {
                    4
                },
            );
            let mut component = c.component(id).unwrap().clone();
            component.sequential.trigger = trigger;
            component.sequential.counter_max = 15;
            component.sequential.random_seed = 17;
            c.replace_component(component);
            let clock = add(&mut c, Kind::Clock, 1);
            link(&mut c, (clock, 0), (id, 2));
            let component = c.component(id).unwrap().clone();
            let pins: Vec<_> = (0..component.ports().len())
                .filter(|&i| i != 2 && !component.drives_port(i))
                .map(|i| driven(&mut c, (id, i)))
                .collect();
            let mut a = Simulator::default();
            let mut b = Simulator::default();
            a.synchronize(&c);
            b.synchronize(&c);
            a.advance(128);
            b.settle_compiled().unwrap();
            let mut seed = 0x8123_1235u32;
            for _ in 0..64 {
                for &pin in &pins {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    a.set_input(pin, seed);
                    b.set_input(pin, seed);
                }
                a.advance(128);
                b.settle_compiled().unwrap();
                assert!(a.tick());
                assert!(b.tick());
                a.advance(128);
                b.settle_compiled().unwrap();
                assert_eq!(
                    a.stored_value(id),
                    b.stored_value(id),
                    "{kind:?} {trigger:?}"
                );
                for i in 0..component.ports().len() {
                    assert_eq!(
                        a.port_value(id, i),
                        b.port_value(id, i),
                        "{kind:?} {trigger:?} {i}"
                    );
                }
            }
        }
    }
}
