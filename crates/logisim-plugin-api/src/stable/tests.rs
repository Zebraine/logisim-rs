use super::*;
fn tool() -> Tool {
    Tool {
        id: "test".into(),
        name: "Test".into(),
        bounds: [[0, 0], [30, 30]],
        ports: (0..3)
            .map(|i| Port {
                id: format!("p{i}"),
                label: format!("Port {i}"),
                at: [i * 10, 0],
                width: 1,
                output: i == 2,
            })
            .collect(),
        graphics: vec![],
        properties: vec![],
        actions: vec![],
    }
}
fn contract() -> StableContract {
    StableContract {
        outputs: vec![OutputDependency {
            port: 2,
            inputs: vec![0],
        }],
        graphics_inputs: vec![0, 1],
        state_inputs: Some(vec![1]),
    }
}
#[test]
fn dependencies_cover_outputs_once_and_only_reference_unique_input_ports() {
    let t = tool();
    let good = contract();
    good.validate(&t).unwrap();
    for ports in [vec![0, 0], vec![2], vec![3], vec![u8::MAX]] {
        let mut c = good.clone();
        c.outputs[0].inputs = ports.clone();
        assert!(c.validate(&t).is_err());
        c = good.clone();
        c.graphics_inputs = ports.clone();
        assert!(c.validate(&t).is_err());
        c = good.clone();
        c.state_inputs = Some(ports);
        assert!(c.validate(&t).is_err());
    }
    let mut c = good.clone();
    c.outputs.clear();
    assert!(c.validate(&t).is_err());
    c = good.clone();
    c.outputs.push(c.outputs[0].clone());
    assert!(c.validate(&t).is_err());
    for port in [0, 3, u8::MAX] {
        c = good.clone();
        c.outputs[0].port = port;
        assert!(c.validate(&t).is_err());
    }
    c = good;
    c.outputs[0].inputs.clear();
    c.graphics_inputs.clear();
    c.state_inputs = None;
    c.validate(&t).unwrap();
    c.validate_state(&[]).unwrap();
    assert!(c.validate_state(&[0]).is_err());
    c.state_inputs = Some(vec![]);
    c.validate_state(&vec![0; MAX_STATE]).unwrap();
    assert!(c.validate_state(&vec![0; MAX_STATE + 1]).is_err());
}
#[test]
fn stable_wire_schema_rejects_state_in_outputs_and_extra_fields() {
    assert!(
        serde_json::from_str::<StableOutputs>(r#"{"outputs":[],"graphics":[],"state":[]}"#)
            .is_err()
    );
    assert!(
        serde_json::from_str::<StableContract>(
            r#"{"outputs":[],"graphics_inputs":[],"state_inputs":null,"delay":1}"#
        )
        .is_err()
    );
    assert!(serde_json::from_str::<Request>(r#"{"type":"stable_sample","data":{"tool":"test","properties":{},"inputs":[],"state":[],"event":{"type":"reset"}}}"#).is_err());
}
