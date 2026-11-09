use super::*;
use logisim_core::Bit;
use validation::*;
fn definition(power: bool) -> Component {
    graphics::manifest()
        .components()
        .into_iter()
        .find(|c| c.tool.id == (Chip { power }).id())
        .unwrap()
}
#[test]
fn observed_sequences_match_event_and_read_only_stable_execution() {
    for power in [false, true] {
        for internal in [false, true] {
            let chip = Chip { power };
            let mut c = definition(power);
            c.properties.insert("internal".into(), internal.to_string());
            let config = StableConfig {
                tool: c.tool.id.clone(),
                properties: c.properties.clone(),
            };
            contract(config.clone())
                .unwrap()
                .unwrap()
                .validate(&c.tool)
                .unwrap();
            for scenario in scenarios()
                .into_iter()
                .filter(|s| power || s.name != "power")
            {
                let mut bytes = vec![];
                let mut hash = HASH_START;
                for row in &scenario.rows {
                    let inputs = inputs(chip, row);
                    let frame::Output::Outputs(before) =
                        handle_stable_frame(&config, frame::Operation::Outputs, &inputs, &bytes)
                            .unwrap()
                    else {
                        panic!()
                    };
                    assert_eq!(
                        frame::Output::Outputs(before),
                        handle_stable_frame(&config, frame::Operation::Outputs, &inputs, &bytes)
                            .unwrap()
                    );
                    let frame::Output::Transition(next) =
                        handle_stable_frame(&config, frame::Operation::Transition, &inputs, &bytes)
                            .unwrap()
                    else {
                        panic!()
                    };
                    let Response::Update(event) = handle(Request::Evaluate(Evaluation {
                        tool: c.tool.id.clone(),
                        properties: c.properties.clone(),
                        inputs: inputs.clone(),
                        state: bytes.clone(),
                        event: Event::Propagate,
                    })) else {
                        panic!()
                    };
                    assert_eq!(event.state, next.state);
                    let frame::Output::Outputs(after) = handle_stable_frame(
                        &config,
                        frame::Operation::Outputs,
                        &inputs,
                        &next.state,
                    )
                    .unwrap() else {
                        panic!()
                    };
                    assert_eq!(after.outputs, event.outputs);
                    assert_eq!(after.graphics, event.graphics);
                    c.tool.validate_update(&event).unwrap();
                    let frame::Output::Transition(again) = handle_stable_frame(
                        &config,
                        frame::Operation::Transition,
                        &inputs,
                        &next.state,
                    )
                    .unwrap() else {
                        panic!()
                    };
                    assert!(next.fixed_point);
                    assert_eq!(again.state, next.state);
                    assert!(!again.outputs_changed);
                    if !next.outputs_changed {
                        assert_eq!(
                            State::decode(&bytes).unwrap().bits,
                            State::decode(&next.state).unwrap().bits
                        );
                    }
                    bytes = next.state;
                    fingerprint(&mut hash, chip, &after.outputs);
                }
                assert_eq!(hash, scenario.expected, "{}", scenario.name);
            }
        }
    }
}
#[test]
fn displayed_register_order_matches_manual_loading_and_two_shifts() {
    let chip = Chip { power: false };
    let mut s = State::default();
    for (row, shown) in [
        ("01101001000010", "00000000"),
        ("01101001001010", "01101001"),
        ("01101001010010", "01101001"),
        ("01101001011010", "11010010"),
        ("01101001010010", "11010010"),
        ("01101001011010", "10100100"),
    ] {
        s.sample(chip, &inputs(chip, row));
        assert!(
            graphics::drawing(Some(&s))
                .iter()
                .any(|g| matches!(g,Shape::Text{text,..} if text==shown))
        );
        assert_eq!(State::decode(&s.encode()).unwrap(), s);
    }
}
#[test]
fn supply_loss_preserves_register_and_clock_history() {
    let chip = Chip { power: true };
    let mut s = State::default();
    for row in ["11111111000010", "11111111001010", "11111111000010"] {
        s.sample(chip, &inputs(chip, row));
    }
    let saved = s.encode();
    for row in ["00000000001000", "00000000000000"] {
        s.sample(chip, &inputs(chip, row));
        assert_eq!(s.encode(), saved);
        assert_eq!(s.outputs(chip, &inputs(chip, row)), [Bit::Unknown; 2]);
    }
    s.sample(chip, &inputs(chip, "00000000000010"));
    assert_eq!(s.bits, [Bit::One; 8]);
    s.sample(chip, &inputs(chip, "00000000001010"));
    assert_eq!(s.bits, [Bit::Zero; 8]);
}
#[test]
fn startup_reset_and_invalid_protocol_inputs_are_checked() {
    graphics::manifest().validate().unwrap();
    ui_metadata().validate(graphics::manifest()).unwrap();
    for power in [false, true] {
        let chip = Chip { power };
        let c = definition(power);
        for clock in "01XE".chars() {
            let mut row = String::from("11111111000010");
            row.replace_range(10..11, &clock.to_string());
            let values = inputs(chip, &row);
            let mut s = State::default();
            s.sample(chip, &values);
            assert_eq!(s.bits, [Bit::Zero; 8]);
            let Response::Update(reset) = handle(Request::Evaluate(Evaluation {
                tool: c.tool.id.clone(),
                properties: c.properties.clone(),
                inputs: values,
                state: vec![99],
                event: Event::Reset,
            })) else {
                panic!()
            };
            assert_eq!(State::decode(&reset.state).unwrap(), s);
        }
        for bad in [
            vec![0],
            b"L65\x02\0\0\0".to_vec(),
            b"L65\x01\x04\0\0".to_vec(),
        ] {
            assert!(State::decode(&bad).is_err());
            assert!(matches!(
                handle(Request::StableOutputs(StableEvaluation {
                    tool: c.tool.id.clone(),
                    properties: c.properties.clone(),
                    inputs: inputs(chip, "00000000000010"),
                    state: bad
                })),
                Response::Error(_)
            ));
        }
        let config = StableConfig {
            tool: c.tool.id.clone(),
            properties: Properties::new(),
        };
        assert!(contract(config).is_err());
        let config = StableConfig {
            tool: c.tool.id.clone(),
            properties: c.properties.clone(),
        };
        assert!(handle_stable_frame(&config, frame::Operation::Outputs, &[], &[]).is_err());
        for pin in chip.pins() {
            assert_eq!(c.tool.ports[chip.index(pin)].at, model::position(pin));
        }
    }
    assert!(
        contract(StableConfig {
            tool: "missing".into(),
            properties: Properties::new()
        })
        .is_err()
    );
}
