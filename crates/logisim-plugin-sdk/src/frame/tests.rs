use super::*;
#[test]
fn configuration_invalid_requests_and_oversized_replies_are_bounded() {
    fn handler(
        c: &StableConfig,
        op: frame::Operation,
        inputs: &[api::Signal],
        state: &[u8],
    ) -> Result<frame::Output, String> {
        assert_eq!(c.tool, "test");
        assert_eq!(op, frame::Operation::Transition);
        assert!(inputs.is_empty());
        if state == [255] {
            return Ok(frame::Output::Transition(
                api::StateTransition::conservative(vec![0; api::MAX_STATE + 1]),
            ));
        }
        Ok(frame::Output::Transition(api::StateTransition {
            state: state.to_vec(),
            outputs_changed: false,
            fixed_point: true,
        }))
    }
    let config = StableConfig {
        tool: "test".into(),
        properties: Default::default(),
    };
    let bytes = serde_json::to_vec(&config).unwrap();
    *INPUT.lock().unwrap() = bytes.clone();
    assert_eq!(configure(bytes.len() as u32), 0);
    for state in [&[42, 0, 255][..], &[255][..]] {
        let n = frame::request_len(&[], state).unwrap();
        {
            let mut input = INPUT.lock().unwrap();
            input.resize(n, 0);
            frame::write_request(&mut input, frame::Operation::Transition, &[], state).unwrap();
        }
        call(n as u32, handler);
        let out = OUTPUT.lock().unwrap();
        let result = frame::read_output(&out, frame::Operation::Transition);
        if state == [255] {
            assert!(result.is_err());
        } else {
            assert_eq!(
                result.unwrap(),
                frame::Output::Transition(api::StateTransition {
                    state: state.to_vec(),
                    outputs_changed: false,
                    fixed_point: true
                })
            );
        }
    }
    call(api::MAX_MESSAGE as u32 + 1, handler);
    assert!(frame::read_output(&OUTPUT.lock().unwrap(), frame::Operation::Transition).is_err());
    assert_eq!(configure(api::MAX_MESSAGE as u32 + 1), 1);
    let n = frame::request_len(&[], &[]).unwrap();
    {
        let mut input = INPUT.lock().unwrap();
        input.resize(n, 0);
        frame::write_request(&mut input, frame::Operation::Transition, &[], &[]).unwrap();
    }
    call(n as u32, handler);
    assert!(
        frame::read_output(&OUTPUT.lock().unwrap(), frame::Operation::Transition)
            .unwrap_err()
            .contains("not configured")
    );
}
