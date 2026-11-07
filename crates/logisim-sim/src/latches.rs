use crate::{Bit, Value};
use logisim_core::{Component, Kind, Latch, LatchEnable, LatchRole};

pub(super) fn evaluate(stored: &mut Value, c: &Component, ports: &[Value]) -> Value {
    let terminals = c.latch_terminals();
    let value = |role| {
        terminals
            .iter()
            .position(|(r, _)| *r == role)
            .map_or(Bit::Unknown, |i| ports[i].bit(0))
    };
    let active = if c.sequential.negate_controls {
        Bit::Zero
    } else {
        Bit::One
    };
    let enabled = match c.latch_enable {
        LatchEnable::High => value(LatchRole::Enable) == Bit::One,
        LatchEnable::Low => value(LatchRole::Enable) == Bit::Zero,
        LatchEnable::None => c.kind == Kind::Latch(Latch::Sr),
    };
    if value(LatchRole::Clear) == active {
        *stored = Value::known(1, 0);
    } else if value(LatchRole::Preset) == active {
        *stored = Value::known(1, 1);
    } else if enabled {
        let next = if c.kind == Kind::Latch(Latch::D) {
            match value(LatchRole::D) {
                Bit::Zero => Some(0),
                Bit::One => Some(1),
                _ => None,
            }
        } else {
            match (value(LatchRole::S), value(LatchRole::R)) {
                (Bit::One, Bit::Zero) => Some(1),
                (Bit::Zero, Bit::One) => Some(0),
                _ => None,
            }
        };
        if let Some(next) = next {
            *stored = Value::known(1, next);
        }
    }
    *stored
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProjectSimulator, Simulator};
    use Bit::{Error as E, One as H, Unknown as X, Zero as L};
    use LatchRole::*;
    use logisim_core::{Circuit, Id, Point, Project};

    fn network(
        c: &Component,
        inputs: &[(LatchRole, Option<Bit>)],
        initial: u32,
    ) -> (Circuit, Id, Simulator) {
        let mut circuit = Circuit::default();
        let id = circuit.add_component(c.clone());
        let mut pins = vec![];
        for (role, value) in inputs {
            if let Some(value) = value {
                let offset = c
                    .latch_terminals()
                    .into_iter()
                    .find(|(r, _)| r == role)
                    .unwrap()
                    .1;
                let pin = circuit.add_component(Component::new(Kind::Pin, c.position + offset));
                pins.push((pin, *value));
            }
        }
        let mut sim = Simulator::default();
        sim.synchronize(&circuit);
        // Seed held storage before delivering a complete input batch, as in the runtime matrices.
        if initial == 1 {
            sim.toggle_stored(id);
        }
        for (pin, value) in pins {
            sim.external_input(pin, Value::filled(1, value));
        }
        sim.advance(64);
        assert!(!sim.limited());
        assert_eq!(sim.pending_count(), 0);
        (circuit, id, sim)
    }
    fn outputs(sim: &Simulator, id: Id, expected: u32) {
        assert_eq!(sim.port_value(id, 0), Some(Value::known(1, expected)));
        assert_eq!(sim.port_value(id, 1), Some(Value::known(1, expected ^ 1)));
    }

    #[test]
    fn latch_networks_match_observed_data_and_enable_matrices_from_both_states() {
        let states = [Some(L), Some(H), Some(X), Some(E), None];
        for initial in [0, 1] {
            for mode in [LatchEnable::High, LatchEnable::Low] {
                let mut c = Component::new(Kind::Latch(Latch::D), Point::new(300, 300));
                c.latch_enable = mode;
                for (ei, en) in states.into_iter().enumerate() {
                    for (di, data) in states.into_iter().enumerate() {
                        let (_, id, sim) = network(&c, &[(Enable, en), (D, data)], initial);
                        let active_column = if mode == LatchEnable::High { 1 } else { 0 };
                        outputs(
                            &sim,
                            id,
                            if ei == active_column && di < 2 {
                                di as u32
                            } else {
                                initial
                            },
                        );
                    }
                }
            }
            for mode in LatchEnable::ALL {
                let mut c = Component::new(Kind::Latch(Latch::Sr), Point::new(300, 300));
                c.latch_enable = mode;
                for (si, s) in states.into_iter().enumerate() {
                    for (ri, r) in states.into_iter().enumerate() {
                        for (ei, en) in states.into_iter().enumerate() {
                            let mut inputs = vec![(S, s), (R, r)];
                            if mode != LatchEnable::None {
                                inputs.push((Enable, en));
                            }
                            let (_, id, sim) = network(&c, &inputs, initial);
                            let active = mode == LatchEnable::None
                                || ei == usize::from(mode == LatchEnable::High);
                            let expected = match (active, si, ri) {
                                (true, 1, 0) => 1,
                                (true, 0, 1) => 0,
                                _ => initial,
                            };
                            outputs(&sim, id, expected);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn asynchronous_clear_precedes_preset_and_data_for_all_layouts_and_polarities() {
        for kind in Latch::ALL {
            for new in [true, false] {
                for controls in logisim_core::ControlPosition::ALL {
                    for negated in [false, true] {
                        let active = if negated { L } else { H };
                        let mut c = Component::new(Kind::Latch(kind), Point::new(300, 300));
                        c.sequential.new_layout = new;
                        c.sequential.controls = controls;
                        c.sequential.negate_controls = negated;
                        for data in [L, H] {
                            for pre in [Some(L), Some(H), Some(X), Some(E), None] {
                                for clr in [Some(L), Some(H), Some(X), Some(E), None] {
                                    let mut inputs =
                                        vec![(Enable, Some(H)), (Preset, pre), (Clear, clr)];
                                    if kind == Latch::D {
                                        inputs.push((D, Some(data)));
                                    } else {
                                        inputs.extend([
                                            (S, Some(data)),
                                            (R, Some(if data == H { L } else { H })),
                                        ]);
                                    }
                                    let (_, id, sim) = network(&c, &inputs, 0);
                                    outputs(
                                        &sim,
                                        id,
                                        if clr == Some(active) {
                                            0
                                        } else if pre == Some(active) {
                                            1
                                        } else {
                                            u32::from(data == H)
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn transparent_updates_poke_reset_and_mode_edits_resolve_without_clock_edges() {
        let c = Component::new(Kind::Latch(Latch::D), Point::new(300, 300));
        let (mut circuit, id, mut sim) = network(
            &c,
            &[
                (Enable, Some(H)),
                (D, Some(L)),
                (Preset, Some(L)),
                (Clear, Some(L)),
            ],
            0,
        );
        let pin = |role| {
            let p = c.position
                + c.latch_terminals()
                    .into_iter()
                    .find(|(r, _)| *r == role)
                    .unwrap()
                    .1;
            circuit
                .components
                .iter()
                .find(|c| c.kind == Kind::Pin && c.position == p)
                .unwrap()
                .id
        };
        let (en, data, pre, clr) = (pin(Enable), pin(D), pin(Preset), pin(Clear));
        for (pin, value, expected) in [
            (data, 1, 1),
            (data, 0, 0),
            (en, 0, 0),
            (data, 1, 0),
            (pre, 1, 1),
            (clr, 1, 0),
            (clr, 0, 1),
            (pre, 0, 1),
        ] {
            sim.set_input(pin, value);
            sim.advance(64);
            outputs(&sim, id, expected);
        }
        sim.toggle_stored(id);
        sim.advance(64);
        outputs(&sim, id, 0);
        sim.toggle_stored(id);
        sim.advance(64);
        outputs(&sim, id, 1);
        sim.set_input(en, 1);
        sim.set_input(data, 0);
        sim.advance(64);
        sim.toggle_stored(id);
        sim.advance(64);
        outputs(&sim, id, 0);
        sim.set_input(pre, 1);
        sim.advance(64);
        sim.toggle_stored(id);
        sim.advance(64);
        outputs(&sim, id, 1);
        sim.set_input(pre, 0);
        sim.set_input(en, 0);
        sim.advance(64);
        let mut edit = circuit.component(id).unwrap().clone();
        edit.latch_enable = LatchEnable::Low;
        circuit.replace_component(edit);
        assert!(sim.synchronize(&circuit));
        sim.advance(64);
        outputs(&sim, id, 0);
        sim.reset();
        sim.advance(64);
        outputs(&sim, id, 0);
    }

    #[test]
    fn nested_latches_keep_independent_instance_state_and_reset_together() {
        let mut project = Project::default();
        let root = project.main;
        let cell = project.add("cell".into()).unwrap();
        let latch = project
            .circuit_mut(cell)
            .unwrap()
            .circuit
            .add_component(Component::new(Kind::Latch(Latch::Sr), Point::new(100, 100)));
        let wrapper = project.add("wrapper".into()).unwrap();
        let inner = project.instance(cell, Point::new(100, 100)).unwrap();
        let inner = project
            .circuit_mut(wrapper)
            .unwrap()
            .circuit
            .add_component(inner);
        let mut ids = vec![];
        for y in [100, 300] {
            let instance = project.instance(wrapper, Point::new(300, y)).unwrap();
            ids.push(
                project
                    .circuit_mut(root)
                    .unwrap()
                    .circuit
                    .add_component(instance),
            );
        }
        let mut sim = ProjectSimulator::default();
        sim.synchronize(&project, root);
        sim.advance(128);
        sim.toggle_stored(&[ids[0], inner], latch);
        sim.advance(128);
        outputs(sim.view(&[ids[0], inner]).unwrap(), latch, 1);
        outputs(sim.view(&[ids[1], inner]).unwrap(), latch, 0);
        assert!(!sim.synchronize(&project, root));
        sim.reset();
        sim.advance(128);
        for id in ids {
            outputs(sim.view(&[id, inner]).unwrap(), latch, 0);
        }
    }
}
