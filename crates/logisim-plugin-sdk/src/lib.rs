//! Rust SDK for the versioned Wasm interface. Only the export macro needs an unsafe
//! attribute (symbol naming); memory buffers and parsing use safe Rust throughout.
pub mod frame;
pub mod stateless;
use api::{MAX_MESSAGE, Request, Response};
pub use logisim_plugin_api as api;
use std::sync::Mutex;
static INPUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static OUTPUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());

pub fn buffer(length: u32) -> u32 {
    if length as usize > MAX_MESSAGE {
        return 0;
    }
    let mut input = INPUT.lock().expect("request buffer");
    input.resize(length as usize, 0);
    input.as_mut_ptr() as u32
}
pub fn call(length: u32, handler: fn(Request) -> Response) -> u64 {
    let request = {
        let input = INPUT.lock().expect("request buffer");
        input
            .get(..length as usize)
            .ok_or_else(|| "Invalid request length".into())
            .and_then(|bytes| serde_json::from_slice(bytes).map_err(|e| e.to_string()))
    };
    let response = match request {
        Ok(request) => handler(request),
        Err(e) => Response::Error(e),
    };
    let mut output = OUTPUT.lock().expect("response buffer");
    *output = serde_json::to_vec(&response).expect("serializable response");
    if output.len() > MAX_MESSAGE {
        *output = serde_json::to_vec(&Response::Error(
            "Plugin response exceeds message limit.".into(),
        ))
        .unwrap();
    }
    ((output.len() as u64) << 32) | u64::from(output.as_ptr() as u32)
}
pub fn presentation(value: api::presentation::Presentation) -> u64 {
    metadata(&value)
}
pub fn reconfiguration(value: api::reconfiguration::Policy) -> u64 {
    metadata(&value)
}
fn metadata(value: &impl serde::Serialize) -> u64 {
    let mut output = OUTPUT.lock().expect("response buffer");
    *output = serde_json::to_vec(value).expect("serializable metadata");
    if output.len() > MAX_MESSAGE {
        output.clear();
    }
    ((output.len() as u64) << 32) | u64::from(output.as_ptr() as u32)
}
/// Export current ABI entry points from a cdylib. The handler must not assume the Describe
/// instance is reused for simulation. Each component has independent module memory.
#[macro_export]
macro_rules! export_plugin {
    ($handler:path $(, stable = $stable:path)? $(, presentation = $ui:path)?, reconfiguration = $policy:path) => {
        $crate::export_plugin!($handler $(, stable = $stable)? $(, presentation = $ui)?);
        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn logisim_reconfiguration_version() -> u32 {
            $crate::api::reconfiguration::VERSION
        }
        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn logisim_reconfiguration() -> u64 {
            $crate::reconfiguration($policy())
        }
    };
    ($handler:path, stable = $stable:path, presentation = $ui:path) => {
        $crate::export_plugin!($handler, stable = $stable);
        $crate::export_plugin!(@presentation $ui);
    };
    ($handler:path, presentation = $ui:path) => {
        $crate::export_plugin!($handler);
        $crate::export_plugin!(@presentation $ui);
    };
    (@presentation $ui:path) => {
        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn logisim_ui_version() -> u32 {
            $crate::api::presentation::VERSION
        }
        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn logisim_ui() -> u64 {
            $crate::presentation($ui())
        }
    };
    ($handler:path, stable = $stable:path) => {
        $crate::export_plugin!($handler);
        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn logisim_frame_version() -> u32 {
            $crate::api::frame::VERSION
        }
        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn logisim_frame_config(length: u32) -> u32 {
            $crate::frame::configure(length)
        }
        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn logisim_frame_call(length: u32) -> u64 {
            $crate::frame::call(length, $stable)
        }
    };
    ($handler:path) => {
        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn logisim_abi() -> u32 {
            $crate::api::ABI_VERSION
        }
        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn logisim_buffer(length: u32) -> u32 {
            $crate::buffer(length)
        }
        #[cfg(target_arch = "wasm32")]
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn logisim_call(length: u32) -> u64 {
            $crate::call(length, $handler)
        }
    };
}
