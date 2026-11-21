use super::{super::*, support::*};
use crate::jit_experiment::{self, Mode};
use logisim_core::{Arithmetic, Gate, Kind};

#[test]
fn experiment_matches_event_and_interpreter_for_mixed_bus_bits_and_carry() {
    for kind in [
        Kind::Gate(Gate::Buffer),
        Kind::Gate(Gate::Not),
        Kind::Arithmetic(Arithmetic::Adder),
        Kind::Gate(Gate::Xor),
    ] {
        for width in [1, 4, 16, 32] {
            let (c, _, inputs) = fixture(component(kind, width));
            let mut event = Simulator::default();
            event.synchronize(&c);
            let (_, stats) = jit_experiment::run(Mode::Verify, || {
                let mut ev = event.compile_combinational().experiment_evaluator();
                for seed in 0..256_usize {
                    for (i, &(id, w)) in inputs.iter().enumerate() {
                        let bits = (0..w).map(|bit| {
                            // All 64 triples at bit zero, plus mixed columns and carry.
                            FOUR[((seed >> (2 * i)) + usize::from(bit) * (1 + i)) % 4]
                        });
                        event.external_input(id, Value::from_bits(bits));
                    }
                    event.advance(1024);
                    assert_eq!(event.pending_count(), 0);
                    ev.evaluate_cached(|(id, p)| event.drive_value(id, p))
                        .unwrap();
                    for ((id, p), v) in ev.values() {
                        assert_eq!(Some(v), event.drive_value(id, p), "{kind:?}/{width}/{seed}");
                    }
                }
            });
            assert_eq!(
                stats.native_nodes,
                usize::from(kind != Kind::Gate(Gate::Xor))
            );
        }
    }
}

#[test]
fn feedback_and_conflicting_widths_reject_native_preparation() {
    for feedback in [false, true] {
        let (mut circuit, gate, inputs) = fixture(component(Kind::Gate(Gate::Not), 4));
        if feedback {
            link(&mut circuit, (gate, 0), (gate, 1), "feedback");
        } else {
            circuit
                .components
                .iter_mut()
                .find(|c| c.id == inputs[0].0)
                .unwrap()
                .bits = 1;
        }
        let mut sim = Simulator::default();
        sim.synchronize(&circuit);
        let (_, stats) = jit_experiment::run(Mode::Verify, || {
            let _ = sim.compile_combinational().experiment_evaluator();
            assert!(sim.settle_compiled().is_err());
        });
        assert_eq!((stats.plans, stats.compiled_kernels), (0, 0));
    }
}
