//! Generic plugin scheduling. No library-specific component behavior or drawing.
pub(crate) mod stable;
use crate::{Bit, Simulator, Value};
use logisim_core::{
    Circuit, Id, Kind,
    plugin::api::{self, Evaluation, Event},
};
use logisim_plugin::{Instance, Registry};
use std::sync::Arc;

pub(crate) struct State {
    definition: api::Component,
    instance: Option<Instance>,
    state: Vec<u8>,
    inputs: Option<Vec<api::Signal>>,
    outputs: Vec<api::Signal>,
    graphics: Vec<api::Shape>,
    raster: Option<Arc<api::raster::Image>>,
    error: Option<String>,
    event_dirty: bool,
}
impl State {
    fn new(definition: api::Component, registry: &Registry) -> Self {
        let instance = registry.instantiate(&definition);
        let error = instance.as_ref().err().cloned();
        let outputs = definition
            .tool
            .ports
            .iter()
            .map(|p| api::Signal::from(Value::filled(p.width, Bit::Error)))
            .collect();
        Self {
            definition,
            instance: instance.ok(),
            state: vec![],
            inputs: None,
            outputs,
            graphics: vec![],
            raster: None,
            error,
            event_dirty: false,
        }
    }
    fn evaluate(&mut self, inputs: Vec<api::Signal>, event: Event) -> bool {
        if self.error.is_some()
            || (matches!(event, Event::Propagate)
                && !self.event_dirty
                && self.inputs.as_ref() == Some(&inputs))
        {
            return false;
        }
        let action = matches!(event, Event::Action { .. });
        if matches!(event, Event::Reset) {
            self.raster = None;
        }
        let request = Evaluation {
            tool: self.definition.tool.id.clone(),
            properties: self.definition.properties.clone(),
            inputs: inputs.clone(),
            state: self.state.clone(),
            event,
        };
        self.inputs = Some(inputs);
        self.event_dirty = false;
        let result = self
            .instance
            .as_mut()
            .expect("valid plugin instance")
            .evaluate(&self.definition, request);
        match result {
            Ok(update) => {
                // Only explicit actions need a change result. Include opaque state even
                // when the addressed register or other storage is not driving a port.
                let changed = action
                    && (self.state != update.state
                        || self.outputs != update.outputs
                        || self.graphics != update.graphics);
                self.state = update.state;
                self.outputs = update.outputs;
                self.graphics = update.graphics;
                match self
                    .instance
                    .as_mut()
                    .expect("valid plugin instance")
                    .raster(&self.definition, &self.state, self.raster.as_ref())
                {
                    Ok(image) => {
                        let raster_changed = action
                            && self.raster.as_ref().map(|i| i.revision)
                                != image.as_ref().map(|i| i.revision);
                        self.raster = image;
                        changed || raster_changed
                    }
                    Err(error) => {
                        self.fail(error);
                        true
                    }
                }
            }
            Err(error) => {
                self.fail(error);
                true
            }
        }
    }
    fn fail(&mut self, error: String) {
        self.error = Some(error);
        self.graphics.clear();
        self.raster = None;
        for (v, p) in self.outputs.iter_mut().zip(&self.definition.tool.ports) {
            *v = Value::filled(p.width, Bit::Error).into();
        }
    }
    fn drives(&self) -> impl Iterator<Item = (usize, Value)> + '_ {
        self.outputs
            .iter()
            .zip(&self.definition.tool.ports)
            .enumerate()
            .filter(|(_, (_, p))| p.output)
            .map(|(i, (v, _))| (i, Value::from(*v)))
    }
}
impl Simulator {
    pub fn set_plugins(&mut self, plugins: Registry) {
        if self.plugins.same(&plugins) {
            return;
        }
        self.plugin_states
            .retain(|_, state| self.plugins.same_plugin(&plugins, &state.definition));
        self.plugins = plugins;
        self.stable_plan = None;
        self.synchronize_plugins(&self.circuit.clone());
        self.schedule_plugins(None, None);
        self.progress.restart();
        self.limited = false;
    }
    pub(crate) fn synchronize_plugins(&mut self, circuit: &Circuit) {
        self.plugin_states = circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::Plugin)
            .filter_map(|c| c.plugin.as_ref().map(|p| (c.id, p)))
            .map(|(id, p)| {
                let state = match self.plugin_states.remove(&id) {
                    Some(s) if s.definition == *p && s.instance.is_some() => s,
                    Some(mut s)
                        if s.error.is_none()
                            && s.instance.is_some()
                            && self.plugins.preserves_state(&s.definition, p) =>
                    {
                        match self.plugins.instantiate(p) {
                            Ok(instance) => {
                                s.definition = p.clone();
                                s.instance = Some(instance);
                                s.event_dirty = true;
                                s.raster = None;
                                s
                            }
                            Err(_) => State::new(p.clone(), &self.plugins),
                        }
                    }
                    _ => State::new(p.clone(), &self.plugins),
                };
                (id, state)
            })
            .collect();
    }
    pub(crate) fn schedule_plugins(
        &mut self,
        action: Option<(Id, Event)>,
        changed: Option<&crate::work::WorkSet>,
    ) -> bool {
        let samples: Vec<_> = self
            .plan
            .plugins
            .iter()
            .filter(|entry| entry.affected(changed))
            .map(|entry| {
                let c = &self.circuit.components[entry.component];
                let values = entry
                    .values(&self.values)
                    .map(Into::into)
                    .collect::<Vec<_>>();
                (c.id, values)
            })
            .collect();
        let mut action_changed = false;
        for (id, inputs) in samples {
            let Some(state) = self.plugin_states.get_mut(&id) else {
                continue;
            };
            let event = if state.inputs.is_none() {
                Event::Reset
            } else {
                action
                    .as_ref()
                    .filter(|(target, _)| *target == id)
                    .map_or(Event::Propagate, |(_, event)| event.clone())
            };
            action_changed |= state.evaluate(inputs, event);
            let drives: Vec<_> = state.drives().collect();
            for (i, v) in drives {
                self.drive_port((id, i), v);
            }
        }
        action_changed
    }
    pub fn plugin_action(&mut self, id: Id, action: String, argument: String) -> bool {
        let Some(state) = self.plugin_states.get(&id) else {
            return false;
        };
        if argument.len() > 1024 || !state.definition.tool.actions.iter().any(|a| a.id == action) {
            return false;
        }
        self.invalidate_stable_values();
        let changed = self.schedule_plugins(
            Some((
                id,
                Event::Action {
                    id: action,
                    argument,
                },
            )),
            None,
        );
        self.progress.restart();
        self.limited = false;
        changed
    }
    pub fn plugin_raster(&self, id: Id) -> Option<&Arc<api::raster::Image>> {
        self.plugin_states.get(&id).and_then(|s| s.raster.as_ref())
    }
    pub fn plugin_graphics(&self, id: Id) -> Option<&[api::Shape]> {
        self.plugin_states.get(&id).map(|s| s.graphics.as_slice())
    }
    /// An instantiated module failed during execution. Unavailable modules remain diagnostic-only.
    pub(crate) fn runtime_plugin_failure(&self) -> Option<&str> {
        self.plugin_states
            .values()
            .filter(|s| s.instance.is_some())
            .find_map(|s| s.error.as_deref())
    }
    pub fn plugin_errors(&self) -> impl Iterator<Item = (Id, &str)> {
        self.plugin_states
            .iter()
            .filter_map(|(id, s)| s.error.as_deref().map(|e| (*id, e)))
    }
}
