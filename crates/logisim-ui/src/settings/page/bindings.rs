use crate::{
    i18n::tr,
    icons::{self, Icon},
    shortcuts::{Chord, Command, Conflict, Group, Shortcuts},
    tf, theme,
};
use egui::{Align, Context, Event, Key, Layout, Ui};

#[derive(Default)]
pub(super) struct BindingEditor {
    capture: Option<Command>,
    error: Option<(Command, Conflict)>,
    reveal_feedback: bool,
    pub(super) confirm_reset: bool,
}
impl BindingEditor {
    pub(super) fn input(&mut self, ctx: &Context, bindings: &mut Shortcuts) {
        let Some(command) = self.capture else {
            return;
        };
        let mac = ctx.os().is_mac();
        let mut pressed = None;
        ctx.input_mut(|i| {
            let mods = i.modifiers;
            i.events.retain(|e| {
                let candidate = match e {
                    Event::Key {
                        key,
                        modifiers,
                        pressed: true,
                        repeat: false,
                        ..
                    } if !Chord::is_modifier(*key) => Some((*key, *modifiers)),
                    // With an empty clipboard the desktop backend can deliver
                    // only the release of primary+V (no Paste or key press).
                    Event::Key {
                        key: Key::V,
                        pressed: false,
                        modifiers,
                        ..
                    } if modifiers.command_only() => Some((Key::V, *modifiers)),
                    Event::Copy if mods.command => Some((Key::C, mods)),
                    Event::Cut if mods.command => Some((Key::X, mods)),
                    Event::Paste(_) | Event::PasteImage(_) if mods.command => Some((Key::V, mods)),
                    Event::Text(_)
                    | Event::Key { .. }
                    | Event::Ime(_)
                    | Event::Paste(_)
                    | Event::PasteImage(_)
                    | Event::Copy
                    | Event::Cut => None,
                    _ => return true,
                };
                if pressed.is_none() {
                    pressed = candidate;
                }
                false
            });
        });
        if let Some((key, mods)) = pressed {
            if key == Key::Escape {
                *self = Self::default();
            } else {
                self.assign(bindings, command, Chord::from_event(key, mods, mac));
            }
        }
    }
    fn assign(&mut self, bindings: &mut Shortcuts, c: Command, chord: Chord) {
        match bindings.bind(c, chord) {
            Ok(()) => *self = Self::default(),
            Err(conflict) => {
                self.error = Some((c, conflict));
                self.reveal_feedback = true;
            }
        }
    }
    pub(super) fn show(&mut self, ui: &mut Ui, bindings: &mut Shortcuts, query: &str) -> bool {
        let category = tr("Shortcuts").to_lowercase();
        let matches = |c: Command| {
            query.is_empty()
                || category.contains(query)
                || [c.id(), tr(c.label()), tr(c.group().label())]
                    .iter()
                    .any(|s| s.to_lowercase().contains(query))
        };
        if query.is_empty() {
            if self.confirm_reset {
                ui.strong(tr("Reset all shortcuts to their defaults?"));
                ui.horizontal_wrapped(|ui| {
                    if ui.button(tr("Confirm reset")).clicked() {
                        *bindings = Shortcuts::default();
                        *self = Self::default();
                    }
                    if ui.button(tr("Cancel")).clicked() {
                        self.confirm_reset = false;
                    }
                });
            } else if ui.button(tr("Reset all shortcuts")).clicked() {
                *self = Self {
                    confirm_reset: true,
                    ..Default::default()
                };
            }
            ui.add_space(4.);
        }
        let mut found = false;
        for group in Group::ALL {
            let mut commands = Command::ALL
                .into_iter()
                .filter(|c| c.group() == group && matches(*c))
                .peekable();
            if commands.peek().is_none() {
                continue;
            }
            found = true;
            theme::card(ui, tr(group.label()), |ui| {
                for (i, c) in commands.enumerate() {
                    if i > 0 {
                        ui.separator();
                    }
                    ui.push_id(c.id(), |ui| self.row(ui, bindings, c));
                }
            });
            ui.add_space(f32::from(theme::metrics(ui.ctx()).gap));
        }
        found
    }
    fn row(&mut self, ui: &mut Ui, bindings: &mut Shortcuts, c: Command) {
        let chord = bindings.get(c);
        ui.horizontal(|ui| {
            let width = ui.available_width();
            let label_width = (width * 0.48).min(width - 115.).max(50.);
            ui.allocate_ui_with_layout(
                egui::vec2(label_width, theme::metrics(ui.ctx()).row),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.add(egui::Label::new(tr(c.label())).wrap())
                        .on_hover_text(c.id());
                },
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icons::button(
                    ui,
                    Icon::Restore,
                    false,
                    chord != c.default_chord(),
                    "Restore default",
                )
                .clicked()
                {
                    self.capture = None;
                    self.assign(bindings, c, c.default_chord());
                }
                let label = chord.label(ui.ctx().os().is_mac());
                let r = ui.add(
                    egui::Button::new(label)
                        .selected(self.capture == Some(c))
                        .wrap(),
                );
                theme::focus_ring(ui, &r);
                if r.clicked() {
                    *self = Self {
                        capture: Some(c),
                        reveal_feedback: true,
                        ..Default::default()
                    };
                    r.request_focus();
                }
            });
        });
        if self.capture == Some(c) {
            let hint = ui.label(tr("Press a shortcut… Esc to cancel"));
            if self.error.is_none() && std::mem::take(&mut self.reveal_feedback) {
                hint.scroll_to_me(None);
            }
        }
        if let Some((target, conflict)) = self.error
            && target == c
        {
            let message = match conflict {
                Conflict::Command(other) => tf!("Already assigned to “{}”.", tr(other.label())),
                Conflict::Fixed => tr("Reserved by a fixed shortcut.").to_owned(),
            };
            let error = ui.colored_label(ui.visuals().error_fg_color, message);
            if std::mem::take(&mut self.reveal_feedback) {
                error.scroll_to_me(None);
            }
        }
    }
}
