//! Generic lookup of validated interaction metadata.
use super::*;
#[cfg(test)]
mod tests;
impl Registry {
    pub fn keyboard(
        &self,
        component: &Component,
    ) -> Option<&logisim_plugin_api::interaction::Keyboard> {
        let plugin = self.plugin(component).ok()?;
        plugin.check_component(component).ok()?;
        plugin
            .interaction
            .keyboards
            .iter()
            .find(|k| k.tool == component.tool.id)
    }
}
