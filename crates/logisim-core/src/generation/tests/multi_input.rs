use super::super::*;
use crate::{Gate, Kind};

fn chain(count: usize, operator: &str) -> Analysis {
    let mut a = Analysis::expressions();
    for i in 0..count {
        a.add_input(format!("I{i}")).unwrap();
    }
    a.add_output("Y".into()).unwrap();
    a.apply_expression(0, &a.inputs().join(operator)).unwrap();
    a
}

#[test]
fn multi_input_gates_compact_runs_and_keep_fanin_bounded() {
    for count in 2..=32 {
        for (operator, gate) in [(" & ", Gate::And), (" | ", Gate::Or), (" ^ ", Gate::Xor)] {
            let a = chain(count, operator);
            let original = a.clone();
            for basis in [GateBasis::Standard, GateBasis::Nand] {
                let c = a
                    .generate_with_options(GenerationOptions {
                        basis,
                        two_input_only: false,
                    })
                    .unwrap();
                let gates: Vec<_> = c
                    .components
                    .iter()
                    .filter(|c| matches!(c.kind, Kind::Gate(_)))
                    .collect();
                assert!(
                    gates
                        .iter()
                        .all(|c| (2..=Gate::MAX_INPUTS).contains(&c.input_count()))
                );
                assert!(c.dangling_ends().is_empty());
                if basis == GateBasis::Standard {
                    assert!(gates.iter().all(|c| c.kind == Kind::Gate(gate)));
                    assert!(gates.iter().all(|c| gate != Gate::Xor
                        || c.input_count() == 2
                        || c.gate_options.xor_odd));
                    let levels = if count <= 8 { 1 } else { 2 };
                    let output = c.components.iter().find(|c| c.label == "Y").unwrap();
                    assert_eq!(
                        output.position.x,
                        300 + count as i32 * 30 + levels * 140 + 100
                    );
                    if count <= 8 {
                        assert_eq!(gates.len(), 1);
                        assert_eq!(gates[0].input_count(), count as u8);
                    } else if count == 32 {
                        assert_eq!(gates.len(), 5);
                    }
                } else {
                    assert!(gates.iter().all(|c| c.kind == Kind::Gate(Gate::Nand)));
                    if count <= 8 {
                        assert_eq!(
                            gates.len(),
                            match gate {
                                Gate::And => 2,
                                Gate::Or => count + 1,
                                _ => 4 * (count - 1),
                            }
                        );
                    }
                }
                assert_eq!(a, original);
                assert_eq!(
                    a.generate_with_options(GenerationOptions {
                        basis,
                        two_input_only: true
                    })
                    .unwrap(),
                    a.generate_with_basis(basis).unwrap()
                );
            }
        }
    }
}

#[test]
fn multi_input_cancellation_and_size_failure_leave_the_analysis_intact() {
    let mut a = chain(1, " | ");
    let mut text = "I0".to_owned();
    for _ in 0..9 {
        text = format!("({text}) | ({text})");
    }
    a.apply_expression(0, &text).unwrap();
    let original = a.clone();
    let options = GenerationOptions::default();
    assert!(a.generate_cancellable(options, || true).is_err());
    assert!(
        a.generate_with_options(GenerationOptions {
            basis: GateBasis::Nand,
            ..options
        })
        .is_err()
    );
    assert!(a.generate_with_options(options).is_ok());
    assert_eq!(a, original);
}
