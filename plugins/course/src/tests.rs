use super::*;
fn update(request: Evaluation) -> Update {
    let tool = graphics::manifest()
        .tools
        .iter()
        .find(|t| t.id == request.tool)
        .unwrap();
    let result = evaluate(request).unwrap();
    tool.validate_update(&result).unwrap();
    result
}

#[test]
fn image_import_replaces_all_words_atomically_and_preserves_clock() {
    let manifest = graphics::manifest();
    let metadata = ui_metadata();
    metadata.validate(manifest).unwrap();
    assert_eq!(metadata.imports[0].tool, "register-file");
    assert_eq!(metadata.text("zh-CN", "Trigger"), "触发方式");
    for tool in &manifest.tools {
        assert_ne!(metadata.text("zh-CN", &tool.name), tool.name);
        for property in &tool.properties {
            assert_ne!(metadata.text("zh-CN", &property.label), property.label);
            if let PropertyKind::Choice { values } = &property.kind {
                for value in values {
                    assert_ne!(metadata.text("zh-CN", value), value);
                }
            }
        }
    }
    let c = manifest.components().remove(0);
    let mut request = Evaluation {
        tool: c.tool.id,
        properties: c.properties,
        inputs: [
            (32, 0),
            (32, 0),
            (32, 42),
            (1, 0),
            (1, 1),
            (5, 1),
            (5, 1),
            (5, 0),
        ]
        .into_iter()
        .map(|(w, v)| Value::known(w, v).into())
        .collect(),
        state: vec![],
        event: Event::Reset,
    };
    let initial = update(request.clone()).state;
    request.state = initial.clone();
    let words = (0..32)
        .map(|i| format!("{:x}", 0xffff0000u32 + i))
        .collect::<Vec<_>>()
        .join(" ");
    request.event = Event::Action {
        id: "write".into(),
        argument: format!("image {words}"),
    };
    let imported = update(request.clone());
    let (stored, clock) = register_state::decode(&imported.state).unwrap().snapshot();
    assert_eq!(clock, logisim_core::Bit::Zero);
    assert_eq!(stored[0].number(), Some(0));
    for (i, word) in stored.iter().enumerate().skip(1) {
        assert_eq!(word.number(), Some(0xffff0000 + i as u32));
    }
    assert_eq!(imported.outputs[0].value, 0xffff0001);
    assert_eq!(imported.outputs[1].value, 0);
    // Invalid complete images cannot produce a partial replacement state.
    for image in [
        "1".to_owned(),
        format!("{words} 0"),
        words.replacen("ffff001f", "badword", 1),
    ] {
        request.event = Event::Action {
            id: "write".into(),
            argument: format!("image {image}"),
        };
        assert!(evaluate(request.clone()).is_err());
    }
    request.event = Event::Propagate;
    assert_eq!(update(request.clone()).state, initial);
    request.state = imported.state;
    request.inputs[3].value = 1;
    assert_eq!(update(request.clone()).outputs[0].value, 42);
    request.event = Event::Reset;
    assert_eq!(update(request).outputs[0].value, 0);
}
#[test]
fn manifest_and_state_transport_keep_clock_words_and_runtime_actions() {
    let manifest = graphics::manifest();
    manifest.validate().unwrap();
    let c = &manifest.components()[0];
    let mut request = Evaluation {
        tool: c.tool.id.clone(),
        properties: c.properties.clone(),
        inputs: [
            (32, 0),
            (32, 0),
            (32, 42),
            (1, 0),
            (1, 1),
            (5, 1),
            (5, 1),
            (5, 0),
        ]
        .into_iter()
        .map(|(width, value)| Signal {
            width,
            value,
            unknown: 0,
            error: 0,
        })
        .collect(),
        state: vec![],
        event: Event::Reset,
    };
    let first = update(request.clone());
    assert_eq!(first.outputs[0].value, 0);
    request.state = first.state;
    request.event = Event::Propagate;
    request.inputs[3].value = 1;
    let next = update(request.clone());
    assert_eq!(next.outputs[0].value, 42);
    assert_eq!(next.outputs[1].value, 0);
    request.state = next.state;
    request.event = Event::Action {
        id: "write".into(),
        argument: "1 DEADBEEF".into(),
    };
    let poked = update(request.clone());
    assert_eq!(poked.outputs[0].value, 0xdeadbeef);
    request.state = poked.state;
    request.event = Event::Reset;
    assert_eq!(update(request.clone()).outputs[0].value, 0);
    request.event = Event::Action {
        id: "write".into(),
        argument: "0 1".into(),
    };
    assert!(evaluate(request).is_err());
}
#[test]
fn probe_supplies_runtime_graphics_and_unknown_inputs_are_blank() {
    let manifest = graphics::manifest();
    let c = &manifest.components()[1];
    let mut request = Evaluation {
        tool: c.tool.id.clone(),
        properties: c.properties.clone(),
        inputs: vec![Signal {
            width: 32,
            value: 0x800211f3,
            unknown: 0,
            error: 0,
        }],
        state: vec![],
        event: Event::Reset,
    };
    assert!(
        matches!(&update(request.clone()).graphics[0],Shape::Text{text,..} if text=="csrrw gp,2048,tp")
    );
    request.inputs[0] = Signal {
        width: 32,
        value: 0,
        unknown: u32::MAX,
        error: 0,
    };
    assert!(update(request).graphics.is_empty());
}

#[test]
fn stable_capabilities_are_configuration_specific_and_reject_invalid_requests() {
    for c in graphics::manifest().components() {
        let config = StableConfig {
            tool: c.tool.id.clone(),
            properties: c.properties.clone(),
        };
        let contract = stable::contract(config.clone()).unwrap().unwrap();
        contract.validate(&c.tool).unwrap();
        let mut invalid = config.clone();
        invalid.tool = "missing".into();
        assert!(stable::contract(invalid).is_err());
        invalid = config.clone();
        invalid.properties.insert("extra".into(), "true".into());
        assert!(stable::contract(invalid).is_err());
        if c.tool.id == "register-file" {
            for trigger in ["high", "low"] {
                let mut config = config.clone();
                config.properties.insert("trigger".into(), trigger.into());
                assert!(stable::contract(config.clone()).unwrap().is_none());
                let request = StableEvaluation {
                    tool: config.tool,
                    properties: config.properties,
                    inputs: c
                        .tool
                        .ports
                        .iter()
                        .map(|p| Value::known(p.width, 0).into())
                        .collect(),
                    state: vec![],
                };
                assert!(stable::outputs(request.clone()).is_err());
                assert!(stable::sample(request).is_err());
            }
        }
    }
}

#[test]
fn stable_reads_do_not_consume_edges_and_samples_do_not_depend_on_read_addresses() {
    let c = graphics::manifest().components().remove(0);
    let mut inputs: Vec<Signal> = [
        (32, 0),
        (32, 0),
        (32, 42),
        (1, 0),
        (1, 1),
        (5, 1),
        (5, 1),
        (5, 0),
    ]
    .into_iter()
    .map(|(w, v)| Value::known(w, v).into())
    .collect();
    let state = update(Evaluation {
        tool: c.tool.id.clone(),
        properties: c.properties.clone(),
        inputs: inputs.clone(),
        state: vec![],
        event: Event::Reset,
    })
    .state;
    inputs[3].value = 1;
    let mut request = StableEvaluation {
        tool: c.tool.id,
        properties: c.properties,
        inputs,
        state,
    };
    let old = stable::outputs(request.clone()).unwrap();
    let next = stable::sample(request.clone()).unwrap();
    assert_eq!(old.outputs[0].value, 0);
    assert_eq!(stable::outputs(request.clone()).unwrap(), old);
    request.inputs[6].value = 31;
    request.inputs[7].value = 31;
    assert_eq!(stable::sample(request.clone()).unwrap(), next);
    request.inputs[6].value = 1;
    request.inputs[7].value = 0;
    request.state = next;
    assert_eq!(
        stable::outputs(request.clone()).unwrap().outputs[0].value,
        42
    );
    let held = stable::outputs(request.clone()).unwrap();
    // All write inputs may vary without changing current-state reads or graphics.
    for port in [2, 3, 4, 5] {
        request.inputs[port].value = 0;
    }
    let after = stable::outputs(request).unwrap();
    assert_eq!(after.outputs[..2], held.outputs[..2]);
    assert_eq!(after.graphics, held.graphics);
}
