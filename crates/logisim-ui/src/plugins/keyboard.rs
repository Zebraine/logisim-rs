//! Focus and ordered, instance-bound input for plugin-declared keyboard interactions.
use crate::{SimulationAction, editor::Editor, simulation::SimulationUi};
use egui::{Event, Ui};
use logisim_core::{
    Id, Project,
    plugin::api::{
        self,
        interaction::{InputEvent, Key, Keyboard},
        presentation::Library,
    },
};
use std::{collections::BTreeSet, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginInputTarget {
    pub root: Id,
    pub circuit: Id,
    pub path: Vec<Id>,
    pub id: Id,
    pub component: Arc<api::Component>,
    pub binding: Keyboard,
}
impl PluginInputTarget {
    pub fn matches(&self, project: &Project, root: Id, circuit: Id, path: &[Id]) -> bool {
        self.root == root
            && self.circuit == circuit
            && self.path == path
            && project
                .circuit(circuit)
                .and_then(|c| c.circuit.component(self.id))
                .and_then(|c| c.plugin.as_ref())
                == Some(self.component.as_ref())
    }
    fn action(&self, event: InputEvent) -> SimulationAction {
        SimulationAction::PluginInput {
            target: Box::new(self.clone()),
            event,
        }
    }
}
pub(crate) struct Focus {
    pub target: PluginInputTarget,
    held: BTreeSet<Key>,
}
pub(crate) fn binding<'a>(
    libraries: &'a [Library],
    c: &logisim_core::Component,
) -> Option<&'a Keyboard> {
    let p = c.plugin.as_ref()?;
    libraries
        .iter()
        .find(|l| {
            l.manifest.id == p.plugin
                && l.manifest.version == p.version
                && l.manifest.tools.contains(&p.tool)
        })?
        .interaction
        .keyboards
        .iter()
        .find(|k| k.tool == p.tool.id)
}
impl Editor {
    pub(crate) fn focus_plugin_keyboard(&mut self, id: Id, libraries: &[Library]) -> bool {
        if self.simulation.view.plugin_errors.contains_key(&id) {
            return false;
        }
        let Some(c) = self.session.circuit.component(id) else {
            return false;
        };
        let Some(binding) = binding(libraries, c) else {
            return false;
        };
        self.simulation.plugin_keyboard = Some(Focus {
            target: PluginInputTarget {
                root: self.session.context.root,
                circuit: self.session.context.active,
                path: self.session.context.path.clone(),
                id,
                component: Arc::new(c.plugin.clone().unwrap()),
                binding: binding.clone(),
            },
            held: BTreeSet::new(),
        });
        true
    }
    pub(crate) fn validate_plugin_keyboard(&mut self, libraries: &[Library]) {
        if self.simulation.plugin_keyboard.as_ref().is_some_and(|f| {
            let t = &f.target;
            t.root != self.session.context.root
                || t.circuit != self.session.context.active
                || t.path != self.session.context.path
                || self.simulation.view.plugin_errors.contains_key(&t.id)
                || self.session.circuit.component(t.id).is_none_or(|c| {
                    c.plugin.as_ref() != Some(t.component.as_ref())
                        || binding(libraries, c) != Some(&t.binding)
                })
        }) {
            self.simulation.release_keyboard();
        }
    }
}
impl SimulationUi {
    pub(crate) fn typing(&self) -> bool {
        self.keyboard_focus.is_some() || self.plugin_keyboard.is_some()
    }
    pub(crate) fn release_keyboard(&mut self) {
        self.keyboard_focus = None;
        if let Some(focus) = self.plugin_keyboard.take() {
            self.actions
                .push(focus.target.action(InputEvent::ReleaseAll {}));
        }
    }
    pub(crate) fn plugin_keyboard_input(&mut self, ui: &Ui) {
        let Some(focus) = &mut self.plugin_keyboard else {
            return;
        };
        let mut release = false;
        ui.input_mut(|input| {
            input.events.retain(|event| {
                if release {
                    return true;
                }
                match event {
                    Event::Key {
                        key: egui::Key::Escape | egui::Key::Tab,
                        pressed: true,
                        ..
                    } => {
                        release = true;
                        false
                    }
                    Event::Key {
                        key,
                        pressed,
                        repeat,
                        modifiers,
                        ..
                    } => {
                        let mapped = focus
                            .target
                            .binding
                            .keys
                            .iter()
                            .copied()
                            .find(|k| egui::Key::from_name(k.name()) == Some(*key));
                        if let Some(key) = mapped {
                            if !pressed && focus.held.remove(&key) {
                                self.actions.push(focus.target.action(InputEvent::Key {
                                    key,
                                    pressed: false,
                                }));
                                return false;
                            }
                            if !modifiers.command && !modifiers.ctrl && !modifiers.alt {
                                if *pressed && !repeat && focus.held.insert(key) {
                                    self.actions.push(
                                        focus.target.action(InputEvent::Key { key, pressed: true }),
                                    );
                                }
                                return false;
                            }
                        }
                        true
                    }
                    Event::Text(_) | Event::Paste(_) => false,
                    _ => true,
                }
            })
        });
        if release {
            self.release_keyboard();
        }
    }
}
