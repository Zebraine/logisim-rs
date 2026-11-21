//! Run with --release -- --nocapture to collect the engine's raw fuel counters.
//! Phase results are caller-owned; the simulation tests cover committing a complete batch.
use super::*;
use crate::{metrics, study};
use serde_json::json;

fn fixture(iterations: Option<u32>) -> Plugin {
    let mut out = Vec::new();
    let mut next = Vec::new();
    frame::write_output(&mut out, &frame::Output::Outputs(outputs())).unwrap();
    frame::write_output(
        &mut next,
        &frame::Output::Transition(StateTransition::conservative(vec![42])),
    )
    .unwrap();
    let burn = match iterations {
        Some(n) => format!(
            "(local $n i32) i32.const {n} local.set $n
            (loop $work local.get $n i32.const 1 i32.sub local.tee $n br_if $work)"
        ),
        None => "(loop $work br $work)".into(),
    };
    let exports = frame_exports(&out, &next, "call $burn");
    frame_module(&format!("{exports} (func $burn {burn})"))
}

#[test]
fn phase_fuel_contract_matrix() {
    for count in [
        Some(1000),
        Some(10_000),
        Some(100_000),
        Some(1_000_000),
        Some(2_000_000),
        None,
    ] {
        let plugin = fixture(count);
        let component = &plugin.manifest().components()[0];
        for multiple in [1, 2, 4, 8] {
            study::with_multiplier(multiple, || {
                for phase in ["outputs", "transition"] {
                    let mut vm = plugin.instantiate_stable(component).unwrap().unwrap();
                    let baseline = vm.reset(&[signal()]).unwrap().state;
                    let invoke = |vm: &mut StableInstance| {
                        if phase == "outputs" {
                            vm.outputs(&[signal()], &baseline)
                                .map(|out| json!(out.outputs))
                        } else {
                            vm.transition(&[signal()], &baseline)
                                .map(|next| json!(next.state))
                        }
                    };
                    let previous = metrics::replace(Some(metrics::Recording::default()));
                    let result = invoke(&mut vm);
                    let retry = invoke(&mut vm);
                    let record = metrics::replace(previous).unwrap();
                    let fuel = if phase == "outputs" {
                        record.outputs
                    } else {
                        record.transition
                    };
                    assert_eq!(
                        fuel.remaining_fuel.len(),
                        if result.is_ok() { 2 } else { 1 }
                    );
                    assert_eq!(retry, result);
                    assert_eq!(vm.reset(&[signal()]).unwrap().state, baseline);
                    assert_eq!(invoke(&mut vm), result);
                    if let Err(error) = &result {
                        assert!(error.contains("fuel"), "{error}");
                    }
                    if count.is_none() {
                        assert!(result.is_err());
                    }
                    if count.is_some_and(|n| n <= 100_000) {
                        assert!(result.is_ok());
                    }
                    let budget = multiple * 10_000_000;
                    println!(
                        "FUEL {}",
                        json!({"iterations":count,"phase":phase,"budget":budget,
                        "remaining":fuel.remaining_fuel[0],"consumed":budget-fuel.remaining_fuel[0],
                        "published_phase_result":result.as_ref().ok(),"error":result.as_ref().err(),
                        "dispatches_including_retry":fuel.remaining_fuel.len(),
                        "retry_matches":retry==result,"reset_replay_matches":true})
                    );
                }
            })
            .unwrap();
        }
    }
}
