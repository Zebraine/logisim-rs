use super::*;
use logisim_plugin_api::presentation::{self, Presentation};
pub(super) fn load(
    instance: &wasmi::Instance,
    store: &mut Store<StoreLimits>,
) -> Result<Option<TypedFunc<(), u64>>> {
    if instance.get_export(&*store, "logisim_ui_version").is_none() {
        return Ok(None);
    }
    let version = instance
        .get_typed_func::<(), u32>(&*store, "logisim_ui_version")
        .map_err(error)?;
    if version.call(&mut *store, ()).map_err(error)? != presentation::VERSION {
        return Ok(None);
    }
    instance
        .get_typed_func(&*store, "logisim_ui")
        .map(Some)
        .map_err(error)
}
impl Instance {
    pub(super) fn presentation(&mut self) -> Result<Presentation> {
        let Some(call) = &self.ui else {
            return Ok(Presentation::default());
        };
        self.store.set_fuel(FUEL).map_err(error)?;
        let packed = call.call(&mut self.store, ()).map_err(error)?;
        let length = (packed >> 32) as usize;
        if length > MAX_MESSAGE {
            return Err("Plugin UI response exceeds message limit.".into());
        }
        let mut bytes = vec![0; length];
        self.memory
            .read(&self.store, packed as u32 as usize, &mut bytes)
            .map_err(error)?;
        serde_json::from_slice(&bytes).map_err(error)
    }
}

#[cfg(test)]
mod tests;
