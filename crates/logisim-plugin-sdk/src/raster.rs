//! Pure, bounded raster callback using explicit state and a previous revision token.
use crate::{INPUT, OUTPUT, api};
use api::{StableConfig, raster};
use std::sync::Mutex;
static CONFIG: Mutex<Option<StableConfig>> = Mutex::new(None);
pub type Handler = fn(&StableConfig, &[u8], Option<u64>) -> Result<raster::Output, String>;
pub fn configure(length: u32) -> u32 {
    let input = INPUT.lock().expect("request buffer");
    let mut config = CONFIG.lock().expect("raster configuration");
    *config = input
        .get(..length as usize)
        .filter(|_| length as usize <= api::MAX_MESSAGE)
        .and_then(|bytes| serde_json::from_slice(bytes).ok());
    u32::from(config.is_none())
}
pub fn call(length: u32, handler: Handler) -> u64 {
    let result = (|| {
        let input = INPUT.lock().expect("request buffer");
        let (previous, state) = raster::read_request(
            input
                .get(..length as usize)
                .ok_or("Invalid raster request length.")?,
        )?;
        let config = CONFIG.lock().expect("raster configuration");
        handler(
            config
                .as_ref()
                .ok_or("Raster instance is not configured.")?,
            state,
            previous,
        )
    })();
    let mut output = OUTPUT.lock().expect("response buffer");
    if let Err(error) = result.and_then(|r| raster::write_output(&mut output, &r)) {
        raster::write_error(&mut output, &error);
    }
    ((output.len() as u64) << 32) | u64::from(output.as_ptr() as u32)
}
