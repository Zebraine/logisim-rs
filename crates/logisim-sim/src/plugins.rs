//! Generic plugin scheduling. No library-specific component behavior or drawing.
pub(crate) mod stable;
use crate::{Bit, Simulator, Value};
use logisim_core::{
    Circuit, Id, Kind,
    plugin::api::{self, Evaluation, Event},
};
use logisim_plugin::{Instance, Registry};

pub(crate) struct State {
    definition: api::Component,
    instance: Option<Instance>,
    state: Vec<u8>,
    inputs: Option<Vec<api::Signal>>,
    outputs: Vec<api::Signal>,
    graphics: Vec<api::Shape>,
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
            error,
            event_dirty: false,
        }
    }
    fn evaluate(&mut self, inputs: Vec<api::Signal>, event: Event) {
        if self.error.is_some()
            || (matches!(event, Event::Propagate)
                && !self.event_dirty
                && self.inputs.as_ref() == Some(&inputs))
        {
            return;
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
                self.state = update.state;
                self.outputs = update.outputs;
                self.graphics = update.graphics;
            }
            Err(error) => self.fail(error),
        }
    }
    fn fail(&mut self, error: String) {
        self.error = Some(error);
        self.graphics.clear();
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
    ) {
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
            state.evaluate(inputs, event);
            let drives: Vec<_> = state.drives().collect();
            for (i, v) in drives {
                self.drive_port((id, i), v);
            }
        }
    }
    pub fn plugin_action(&mut self, id: Id, action: String, argument: String) {
        let Some(state) = self.plugin_states.get(&id) else {
            return;
        };
        if argument.len() > 1024 || !state.definition.tool.actions.iter().any(|a| a.id == action) {
            return;
        }
        self.invalidate_stable_values();
        self.schedule_plugins(
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
    }
    pub fn plugin_graphics(&self, id: Id) -> Option<&[api::Shape]> {
        self.plugin_states.get(&id).map(|s| s.graphics.as_slice())
    }
    pub fn plugin_errors(&self) -> impl Iterator<Item = (Id, &str)> {
        self.plugin_states
            .iter()
            .filter_map(|(id, s)| s.error.as_deref().map(|e| (*id, e)))
    }
}
