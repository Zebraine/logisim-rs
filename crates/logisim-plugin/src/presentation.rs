use super::*;
use logisim_plugin_api::presentation::{self, Presentation};
pub(super) fn load(
    instance: &wasmi::Instance,
    store: &mut Store<StoreLimits>,
) -> Result<Option<TypedFunc<(), u64>>> {
    metadata::load(
        instance,
        store,
        "logisim_ui_version",
        "logisim_ui",
        presentation::VERSION,
    )
}
impl Instance {
    pub(super) fn presentation(&mut self) -> Result<Presentation> {
        self.metadata(self.ui)
    }
}
#[cfg(test)]
mod tests;
