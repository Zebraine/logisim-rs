use crate::{SimulationAction, Workspace};
use logisim_core::{Component, Id, MemoryContents, plugin::api::presentation::MemoryImport};
/// Captured before opening a picker; never resolve an import by a reused component ID alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginImportTarget {
    circuit: Id,
    root: Id,
    path: Vec<Id>,
    component: Component,
    index: usize,
    import: MemoryImport,
}
impl PluginImportTarget {
    pub fn import(&self) -> &MemoryImport {
        &self.import
    }
}
impl Workspace {
    pub fn plugin_import_target(&self, component: Id, index: usize) -> Option<PluginImportTarget> {
        let c = self.circuit().component(component)?;
        let import = super::metadata(&self.plugins, c)?.imports.get(index)?;
        if c.plugin.as_ref()?.tool.id != import.tool
            || self
                .editor
                .simulation
                .view
                .plugin_errors
                .contains_key(&component)
        {
            return None;
        }
        Some(PluginImportTarget {
            circuit: self.active_circuit(),
            root: self.simulation_root(),
            path: self.simulation_path().to_vec(),
            component: c.clone(),
            index,
            import: import.clone(),
        })
    }
    pub fn complete_plugin_import(
        &mut self,
        target: &PluginImportTarget,
        contents: &MemoryContents,
    ) -> bool {
        if self
            .plugin_import_target(target.component.id, target.index)
            .as_ref()
            != Some(target)
        {
            return false;
        }
        let words = (0..1u32 << target.import.address_bits)
            .map(|i| format!("{:x}", contents.get(i)))
            .collect::<Vec<_>>()
            .join(" ");
        let argument = format!("{}{words}", target.import.argument_prefix);
        if argument.len() > 1024 {
            return false;
        }
        self.editor
            .simulation
            .actions
            .push(SimulationAction::PluginAction {
                id: target.component.id,
                action: target.import.action.clone(),
                argument,
            });
        true
    }
}
