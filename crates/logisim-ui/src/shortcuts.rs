//! egui keyboard events and localized captions for portable bindings.
use egui::{Key, Modifiers};
pub use logisim_preferences::shortcuts::{Chord, Command, Conflict, Group, Scope, Shortcuts};
/// Canvas input runs before its context menu is painted in the current pass.
pub(crate) fn popup_open(ctx: &egui::Context) -> bool {
    ctx.any_popup_open() || egui::Popup::is_any_open(ctx)
}

pub(crate) trait ChordUi: Sized {
    fn is_modifier(key: Key) -> bool;
    fn consume(ui: &egui::Ui, mods: Modifiers, key: Key) -> bool;
    fn from_event(key: Key, mods: Modifiers, mac: bool) -> Self;
}
impl ChordUi for Chord {
    fn is_modifier(key: Key) -> bool {
        matches!(
            key,
            Key::ShiftLeft
                | Key::ShiftRight
                | Key::ControlLeft
                | Key::ControlRight
                | Key::AltLeft
                | Key::AltRight
                | Key::SuperLeft
                | Key::SuperRight
                | Key::Fn
        )
    }

    fn consume(ui: &egui::Ui, mods: Modifiers, key: Key) -> bool {
        let mac = ui.ctx().os().is_mac();
        let wanted = Self::from_event(key, mods, mac);
        ui.input_mut(|i| {
            let count = i.events.len();
            i.events.retain(|event| match event {
                egui::Event::Key {
                    key,
                    modifiers,
                    pressed: true,
                    ..
                } => !wanted.matches(Self::from_event(*key, *modifiers, mac), mac),
                _ => true,
            });
            i.events.len() != count
        })
    }
    /// egui sets both ctrl and command for the same key on Windows and Linux.
    fn from_event(key: Key, modifiers: Modifiers, mac: bool) -> Self {
        Self::from_key(
            key.name(),
            modifiers.command || modifiers.mac_cmd || (!mac && modifiers.ctrl),
            modifiers.shift,
            modifiers.alt,
            mac && modifiers.ctrl,
        )
    }
}
pub(crate) trait ShortcutCaptions {
    fn caption(&self, c: Command, ui: &egui::Ui) -> String;
    fn caption_with(&self, c: Command, label: &str, ui: &egui::Ui) -> String;
}
impl ShortcutCaptions for Shortcuts {
    fn caption(&self, c: Command, ui: &egui::Ui) -> String {
        self.caption_with(c, c.label(), ui)
    }
    fn caption_with(&self, c: Command, label: &str, ui: &egui::Ui) -> String {
        format!(
            "{} · {}",
            crate::i18n::tr(label),
            self.get(c).label(ui.ctx().os().is_mac())
        )
    }
}
#[cfg(test)]
mod tests;
