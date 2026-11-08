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
        instance: &wasmi::Instance,
        store: &mut Store<StoreLimits>,
        abi: u32,
    ) -> Result<Option<Self>> {
        if abi < ABI_VERSION
            || instance
                .get_export(&*store, "logisim_frame_version")
                .is_none()
        {
            return Ok(None);
        }
        let version = instance
            .get_typed_func::<(), u32>(&*store, "logisim_frame_version")
            .map_err(error)?;
        if version.call(&mut *store, ()).map_err(error)? != frame::VERSION {
            return Ok(None); // Unsupported optional transport: retain the JSON interface.
        }
        Ok(Some(Self {
            configure: instance
                .get_typed_func(&*store, "logisim_frame_config")
                .map_err(error)?,
            call: instance
                .get_typed_func(&*store, "logisim_frame_call")
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
        self.store.set_fuel(FUEL).map_err(error)?;
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
        let length = frame::request_len(signals, state)?;
        self.store.set_fuel(FUEL).map_err(error)?;
        let offset = self
            .buffer
            .call(&mut self.store, length as u32)
            .map_err(error)? as usize;
        let end = offset
            .checked_add(length)
            .ok_or("Invalid plugin input range.")?;
        let input = self
            .memory
            .data_mut(&mut self.store)
            .get_mut(offset..end)
            .ok_or("Invalid plugin input range.")?;
        frame::write_request(input, operation, signals, state)?;
        let packed = api
            .call
            .call(&mut self.store, length as u32)
            .map_err(error)?;
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
        frame::read_output(output, operation)
    }
}
