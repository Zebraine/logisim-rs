//! A pointer is a checked Wasm32 offset, never a native address. Memory borrows end
//! before every guest call (which may grow or relocate linear memory).
use super::*;
use logisim_plugin_api::{ABI_VERSION, Signal, StableConfig, frame};
pub(super) struct FrameApi {
    configure: TypedFunc<u32, u32>,
    call: TypedFunc<u32, u64>,
}
impl FrameApi {
    pub(super) fn load(
        instance: &backend::Instance,
        store: &mut Store<StoreLimits>,
        abi: u32,
    ) -> Result<Option<Self>> {
        if abi < ABI_VERSION
            || instance
                .get_export(store.as_context_mut(), "logisim_frame_version")
                .is_none()
        {
            return Ok(None);
        }
        let version = instance
            .get_typed_func::<(), u32>(store.as_context_mut(), "logisim_frame_version")
            .map_err(error)?;
        if version.call(&mut *store, ()).map_err(error)? != frame::VERSION {
            return Ok(None); // Unsupported optional transport: retain the JSON interface.
        }
        Ok(Some(Self {
            configure: instance
                .get_typed_func(store.as_context_mut(), "logisim_frame_config")
                .map_err(error)?,
            call: instance
                .get_typed_func(store.as_context_mut(), "logisim_frame_call")
                .map_err(error)?,
        }))
    }
}
impl Instance {
    pub(super) fn bind_frame(&mut self, component: &Component) -> Result<()> {
        let Some(api) = &self.frame else {
            return Ok(());
        };
        let bytes = serde_json::to_vec(&StableConfig {
            tool: component.tool.id.clone(),
            properties: component.properties.clone(),
        })
        .map_err(error)?;
        if bytes.len() > MAX_MESSAGE {
            return Err("Plugin configuration exceeds message limit.".into());
        }
        self.store.set_fuel(fuel_budget()).map_err(error)?;
        let offset = self
            .buffer
            .call(&mut self.store, bytes.len() as u32)
            .map_err(error)?;
        self.memory
            .write(&mut self.store, offset as usize, &bytes)
            .map_err(error)?;
        if api
            .configure
            .call(&mut self.store, bytes.len() as u32)
            .map_err(error)?
            != 0
        {
            return Err("Plugin rejected linear-memory configuration.".into());
        }
        Ok(())
    }
    pub(super) fn call_frame(
        &mut self,
        operation: frame::Operation,
        signals: &[Signal],
        state: &[u8],
    ) -> Result<frame::Output> {
        let api = self
            .frame
            .as_ref()
            .ok_or("Plugin has no linear-memory interface.")?;
        #[cfg(feature = "plugin-metrics")]
        let before_copy = frame::metrics::copied_ns();
        let length = timed!("encode", frame::request_len(signals, state))?;
        self.store.set_fuel(fuel_budget()).map_err(error)?;
        let offset = timed!(
            "guest_buffer",
            self.buffer.call(&mut self.store, length as u32)
        )
        .map_err(error)? as usize;
        let end = offset
            .checked_add(length)
            .ok_or("Invalid plugin input range.")?;
        let input = self
            .memory
            .data_mut(&mut self.store)
            .get_mut(offset..end)
            .ok_or("Invalid plugin input range.")?;
        timed!(
            "encode",
            frame::write_request(input, operation, signals, state)
        )?;
        #[cfg(feature = "plugin-metrics")]
        let copy_after_write = frame::metrics::copied_ns();
        let packed = timed!(
            "guest_dispatch",
            api.call.call(&mut self.store, length as u32)
        );
        #[cfg(feature = "engine-study")]
        study::record_remaining(&self.store)?;
        let packed = packed.map_err(error)?;
        #[cfg(feature = "plugin-metrics")]
        metrics::on_call(|c| {
            c.frame_calls += 1;
            c.state_write_ns += copy_after_write - before_copy;
            c.request_bytes += length;
            c.response_bytes += (packed >> 32) as usize;
        });
        #[cfg(feature = "plugin-metrics")]
        let _decode = metrics::timer("decode");
        let length = (packed >> 32) as usize;
        if length > MAX_MESSAGE {
            return Err("Plugin response exceeds message limit.".into());
        }
        let offset = packed as u32 as usize;
        let end = offset
            .checked_add(length)
            .ok_or("Invalid plugin output range.")?;
        let output = self
            .memory
            .data(&self.store)
            .get(offset..end)
            .ok_or("Invalid plugin output range.")?;
        let result = frame::read_output(output, operation);
        #[cfg(feature = "plugin-metrics")]
        metrics::on_call(|c| c.state_read_ns += frame::metrics::copied_ns() - copy_after_write);
        result
    }
}
