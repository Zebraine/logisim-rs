use super::*;
use logisim_plugin_api::presentation::{self, Presentation};
pub(super) fn load(
    instance: &backend::Instance,
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
        metadata::read(&mut self.store, &self.memory, self.ui.as_ref())
    }
}
#[cfg(test)]
mod tests;
