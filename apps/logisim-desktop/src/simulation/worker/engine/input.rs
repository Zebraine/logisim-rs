//! Capture survives ordinary snapshots, but cannot cross instance or module changes.
use super::*;
use logisim_core::plugin::api::interaction::InputEvent;
use logisim_ui::PluginInputTarget;
#[cfg(test)]
mod tests;
impl Engine {
    pub(super) fn input_matches(target: &PluginInputTarget, context: &Context) -> bool {
        target.matches(
            &context.project,
            context.root,
            context.circuit,
            &context.path,
        ) && context.plugins.keyboard(&target.component) == Some(&target.binding)
    }
    pub(super) fn input_event(&mut self, target: Box<PluginInputTarget>, event: InputEvent) {
        if self.suspended
            || self
                .context
                .as_ref()
                .is_none_or(|c| !Self::input_matches(&target, c))
            || matches!(event, InputEvent::Key { key, .. } if !target.binding.keys.contains(&key))
        {
            return;
        }
        if self.input.as_ref().is_some_and(|t| t != &target) {
            self.release_input();
        }
        self.sim.plugin_action(
            &target.path,
            target.id,
            target.binding.action.clone(),
            event.argument(),
        );
        self.input = if matches!(event, InputEvent::ReleaseAll {}) {
            None
        } else {
            Some(target)
        };
    }
    pub(super) fn release_input(&mut self) {
        if let Some(target) = self.input.take() {
            self.sim.plugin_action(
                &target.path,
                target.id,
                target.binding.action,
                InputEvent::ReleaseAll {}.argument(),
            );
        }
    }
}
