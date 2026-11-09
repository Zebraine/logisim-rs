use super::*;
impl Registry {
    /// The current module explicitly guarantees that these property edits preserve state.
    pub fn preserves_state(&self, old: &Component, new: &Component) -> bool {
        self.plugin(new).is_ok_and(|p| {
            p.check_component(old).is_ok()
                && p.check_component(new).is_ok()
                && p.reconfiguration.allows(old, new)
        })
    }
}
#[cfg(test)]
mod tests;
