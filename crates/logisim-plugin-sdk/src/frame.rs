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
mod tests;
