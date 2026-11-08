//! The hot path borrows state from the input buffer until the handler returns.
use crate::{INPUT, OUTPUT, api};
use api::{StableConfig, frame};
use std::sync::Mutex;
static CONFIG: Mutex<Option<StableConfig>> = Mutex::new(None);
pub type Handler =
    fn(&StableConfig, frame::Operation, &[api::Signal], &[u8]) -> Result<frame::Output, String>;

/// A new execution VM is configured before any frame call. Nonzero rejects binding.
pub fn configure(length: u32) -> u32 {
    let input = INPUT.lock().expect("request buffer");
    let mut config = CONFIG.lock().expect("frame configuration");
    *config = input
        .get(..length as usize)
        .filter(|_| length as usize <= api::MAX_MESSAGE)
        .and_then(|bytes| serde_json::from_slice(bytes).ok());
    u32::from(config.is_none())
}
pub fn call(length: u32, handler: Handler) -> u64 {
    let result = (|| {
        let input = INPUT.lock().expect("request buffer");
        let input = frame::Input::read(
            input
                .get(..length as usize)
                .ok_or("Invalid frame length.")?,
        )?;
        let config = CONFIG.lock().expect("frame configuration");
        handler(
            config.as_ref().ok_or("Frame instance is not configured.")?,
            input.operation,
            input.signals(),
            input.state,
        )
    })();
    let mut output = OUTPUT.lock().expect("response buffer");
    let result = result.and_then(|response| frame::write_output(&mut output, &response));
    if let Err(error) = result {
        frame::write_error(&mut output, &error);
    }
    ((output.len() as u64) << 32) | u64::from(output.as_ptr() as u32)
}

#[cfg(test)]
mod tests {
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
}
