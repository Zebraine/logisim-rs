//! Independently negotiated raster transport. Guest memory is copied before another call.
use super::*;
use logisim_plugin_api::{StableConfig, raster};
#[cfg(test)]
mod tests;
pub(super) struct RasterApi {
    configure: TypedFunc<u32, u32>,
    call: TypedFunc<u32, u64>,
}
impl RasterApi {
    pub(super) fn load(
        instance: &wasmi::Instance,
        store: &mut Store<StoreLimits>,
    ) -> Result<Option<Self>> {
        if instance
            .get_export(&*store, "logisim_raster_version")
            .is_none()
        {
            return Ok(None);
        }
        let version = instance
            .get_typed_func::<(), u32>(&*store, "logisim_raster_version")
            .map_err(error)?;
        if version.call(&mut *store, ()).map_err(error)? != raster::VERSION {
            return Ok(None);
        }
        Ok(Some(Self {
            configure: instance
                .get_typed_func(&*store, "logisim_raster_config")
                .map_err(error)?,
            call: instance
                .get_typed_func(&*store, "logisim_raster_call")
                .map_err(error)?,
        }))
    }
}
impl Instance {
    pub(super) fn bind_raster(&mut self, component: &Component) -> Result<()> {
        let Some(api) = &self.raster else {
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
            return Err("Plugin rejected raster configuration.".into());
        }
        Ok(())
    }
    /// Pure state rendering. The caller invalidates previous on reset/config changes.
    pub fn raster(
        &mut self,
        component: &Component,
        state: &[u8],
        previous: Option<&Arc<raster::Image>>,
    ) -> Result<Option<Arc<raster::Image>>> {
        let Some(api) = &self.raster else {
            return Ok(None);
        };
        let revision = previous.map(|i| i.revision);
        let mut request = Vec::new();
        raster::write_request(&mut request, revision, state)?;
        self.store.set_fuel(FUEL).map_err(error)?;
        let offset = self
            .buffer
            .call(&mut self.store, request.len() as u32)
            .map_err(error)?;
        self.memory
            .write(&mut self.store, offset as usize, &request)
            .map_err(error)?;
        let packed = api
            .call
            .call(&mut self.store, request.len() as u32)
            .map_err(error)?;
        let length = (packed >> 32) as usize;
        if length > raster::MAX_RESPONSE {
            return Err("Plugin raster exceeds message limit.".into());
        }
        let start = packed as u32 as usize;
        let end = start
            .checked_add(length)
            .ok_or("Invalid plugin raster range.")?;
        let bytes = self
            .memory
            .data(&self.store)
            .get(start..end)
            .ok_or("Invalid plugin raster range.")?;
        match raster::read_output(bytes, revision)? {
            raster::Output::None => Ok(None),
            raster::Output::Unchanged(_) => Ok(previous.cloned()),
            raster::Output::Image(image) => {
                image.validate_tool(&component.tool)?;
                if let Some(old) = previous {
                    if **old == image {
                        return Ok(Some(Arc::clone(old)));
                    }
                    if old.revision == image.revision {
                        return Err("Plugin raster reused a revision for different pixels.".into());
                    }
                }
                Ok(Some(Arc::new(image)))
            }
        }
    }
}
