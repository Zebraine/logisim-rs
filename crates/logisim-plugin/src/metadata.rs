//! Common bounded transport for independently versioned optional metadata.
use super::*;
pub(super) fn load(
    instance: &wasmi::Instance,
    store: &mut Store<StoreLimits>,
    version_export: &str,
    data_export: &str,
    supported: u32,
) -> Result<Option<TypedFunc<(), u64>>> {
    if instance.get_export(&*store, version_export).is_none() {
        return Ok(None);
    }
    let version = instance
        .get_typed_func::<(), u32>(&*store, version_export)
        .map_err(error)?;
    if version.call(&mut *store, ()).map_err(error)? != supported {
        return Ok(None);
    }
    instance
        .get_typed_func(&*store, data_export)
        .map(Some)
        .map_err(error)
}
impl Instance {
    pub(super) fn metadata<T: serde::de::DeserializeOwned + Default>(
        &mut self,
        call: Option<TypedFunc<(), u64>>,
    ) -> Result<T> {
        let Some(call) = call else {
            return Ok(T::default());
        };
        self.store.set_fuel(FUEL).map_err(error)?;
        let packed = call.call(&mut self.store, ()).map_err(error)?;
        let length = (packed >> 32) as usize;
        if length > MAX_MESSAGE {
            return Err("Plugin metadata exceeds message limit.".into());
        }
        let mut bytes = vec![0; length];
        self.memory
            .read(&self.store, packed as u32 as usize, &mut bytes)
            .map_err(error)?;
        serde_json::from_slice(&bytes).map_err(error)
    }
}
