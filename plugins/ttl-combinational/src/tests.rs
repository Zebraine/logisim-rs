use super::*;
use model::{Chip, Kind};
use validation::*;

#[test]
fn reference_binary_and_four_state_vectors_match_with_both_supply_interfaces() {
    for kind in Kind::ALL {
        for power in [false, true] {
            let chip = Chip { kind, power };
            let c = graphics::manifest()
                .components()
                .into_iter()
                .find(|c| c.tool.id == chip.id())
                .unwrap();
            for (cases, expected) in [
                (binary(kind), expected(kind).0),
                (four(kind), expected(kind).1),
            ] {
                let mut hash = HASH_START;
                for values in cases {
                    let inputs = signals(chip, &values);
                    let Response::Update(result) = handle(Request::Evaluate(Evaluation {
                        tool: c.tool.id.clone(),
                        properties: c.properties.clone(),
                        inputs,
                        state: vec![],
                        event: Event::Propagate,
                    })) else {
                        panic!("evaluation failed")
                    };
                    c.tool.validate_update(&result).unwrap();
                    fingerprint(&mut hash, chip, &result.outputs);
                }
                assert_eq!(hash, expected, "{}", chip.id());
            }
        }
    }
}
#[test]
fn interfaces_graphics_and_stable_dependencies_are_valid_and_read_only() {
    let manifest = graphics::manifest();
    manifest.validate().unwrap();
    ui_metadata().validate(manifest).unwrap();
    assert_eq!(manifest.tools.len(), 8);
    for c in manifest.components() {
        let chip = Chip::parse(&c.tool.id).unwrap();
        let mut internal = c.tool.clone();
        internal.graphics = graphics::drawing(chip, true);
        internal.validate().unwrap();
        assert_eq!(c.tool.bounds[1][0], i32::from(chip.kind.count()) * 10);
        for (i, pin) in chip.pins().iter().enumerate() {
            assert_eq!(c.tool.ports[i].at, chip.kind.position(*pin));
            assert_eq!(c.tool.ports[i].output, chip.kind.outputs().contains(pin));
            assert_eq!(chip.index(*pin), i);
        }
        let config = StableConfig {
            tool: c.tool.id.clone(),
            properties: c.properties.clone(),
        };
        let contract = LIBRARY.contract(config.clone()).unwrap().unwrap();
        contract.validate(&c.tool).unwrap();
        assert_eq!(contract.state_inputs, None);
        if chip.kind == Kind::Buffer74125 {
            assert!(
                contract
                    .outputs
                    .iter()
                    .all(|p| p.inputs.len() == if chip.power { 4 } else { 2 })
            );
        }
        let inputs = signals(chip, &"0".repeat(chip.kind.inputs().len()));
        let first = LIBRARY
            .frame(&config, frame::Operation::Outputs, &inputs, &[])
            .unwrap();
        assert_eq!(
            LIBRARY
                .frame(&config, frame::Operation::Outputs, &inputs, &[])
                .unwrap(),
            first
        );
        assert!(
            LIBRARY
                .frame(&config, frame::Operation::Transition, &inputs, &[1])
                .is_err()
        );
        let frame::Output::Transition(t) = LIBRARY
            .frame(&config, frame::Operation::Transition, &inputs, &[])
            .unwrap()
        else {
            panic!()
        };
        assert!(t.state.is_empty() && t.fixed_point && !t.outputs_changed);
        for bad in [
            StableEvaluation {
                tool: "missing".into(),
                properties: c.properties.clone(),
                inputs: inputs.clone(),
                state: vec![],
            },
            StableEvaluation {
                tool: c.tool.id.clone(),
                properties: Properties::new(),
                inputs: inputs.clone(),
                state: vec![],
            },
            StableEvaluation {
                tool: c.tool.id.clone(),
                properties: c.properties.clone(),
                inputs: vec![],
                state: vec![],
            },
            StableEvaluation {
                tool: c.tool.id.clone(),
                properties: c.properties.clone(),
                inputs: inputs.clone(),
                state: vec![1],
            },
        ] {
            for request in [
                Request::StableOutputs(bad.clone()),
                Request::StableSample(bad.clone()),
                Request::StableTransition(bad),
            ] {
                assert!(matches!(handle(request), Response::Error(_)));
            }
        }
    }
}
