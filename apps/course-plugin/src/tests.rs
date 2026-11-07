use super::*;
fn update(request: Evaluation) -> Update {
    let tool = graphics::manifest()
        .tools
        .into_iter()
        .find(|t| t.id == request.tool)
        .unwrap();
    let result = evaluate(request).unwrap();
    tool.validate_update(&result).unwrap();
    result
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
