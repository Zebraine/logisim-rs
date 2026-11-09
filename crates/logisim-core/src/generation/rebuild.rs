//! Bind analysis variables to document Pins independently of generated item IDs.
use super::{Analysis, Error, Result};
use crate::{Circuit, CircuitDefinition, Component, Id, Kind, Point};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[cfg(test)]
mod tests;

/// The same deterministic order is used by analysis and source binding, including
/// unnamed Pins whose analyzer names exist only in the session.
pub fn analysis_pins(circuit: &Circuit) -> Vec<&Component> {
    let mut pins: Vec<_> = circuit
        .components
        .iter()
        .filter(|c| c.kind == Kind::Pin)
        .collect();
    pins.sort_by_key(|p| (p.output, p.position.y, p.position.x, p.id));
    pins
}

#[derive(Clone)]
pub struct RebuildTarget {
    original: Arc<CircuitDefinition>,
    pins: Arc<BTreeMap<(bool, String), Id>>,
}
impl RebuildTarget {
    pub fn bind(definition: &CircuitDefinition, analysis: &Analysis) -> Result<Self> {
        let pins = analysis_pins(&definition.circuit);
        let names: Vec<_> = analysis
            .inputs()
            .iter()
            .map(|s| (false, s.as_str()))
            .chain(analysis.outputs().iter().map(|o| (true, o.name())))
            .collect();
        if pins.len() != names.len()
            || pins.iter().zip(&names).any(|(p, (output, name))| {
                p.bits != 1 || p.output != *output || (!p.label.is_empty() && p.label != *name)
            })
        {
            return Err(Error::new(
                "The analyzed interface no longer matches the circuit. Please analyze it again.",
            ));
        }
        Ok(Self {
            original: Arc::new(definition.clone()),
            pins: pins
                .into_iter()
                .zip(names)
                .map(|(p, (output, name))| ((output, name.to_owned()), p.id))
                .collect::<BTreeMap<_, _>>()
                .into(),
        })
    }
    pub fn id(&self) -> Id {
        self.original.id
    }
    pub fn name(&self) -> &str {
        &self.original.name
    }

    /// Prepare a replacement without mutating the target or its parent circuits.
    pub fn rebuild(&self, current: &CircuitDefinition, generated: Circuit) -> Result<Circuit> {
        if current != self.original.as_ref() {
            return Err(Error::new(
                "The circuit changed during analysis. Please analyze it again.",
            ));
        }
        let keys: BTreeSet<_> = analysis_pins(&generated)
            .into_iter()
            .map(|p| (p.output, p.label.clone()))
            .collect();
        let unchanged =
            keys.len() == self.pins.len() && keys.iter().all(|key| self.pins.contains_key(key));
        // Retain the allocator watermark, never alias old internal state/selection
        // with a newly generated gate or wire. Surviving Pins keep their identity.
        let mut result = current.circuit.clone();
        result.components.clear();
        result.wires.clear();
        result.appearance = None;
        for mut c in generated.components {
            let pin = (c.kind == Kind::Pin)
                .then(|| self.pins.get(&(c.output, c.label.clone())))
                .flatten()
                .copied();
            if unchanged && let Some(id) = pin {
                let position = c.position;
                c = current.circuit.component(id).unwrap().clone();
                c.position = position;
            }
            result.add_component(c);
            if let Some(id) = pin {
                result.components.last_mut().unwrap().id = id;
            }
        }
        for w in generated.wires {
            result.add_wire(w.a, w.b);
        }
        if unchanged {
            result.appearance = current.circuit.appearance.clone();
            if result.appearance.is_none() {
                let rebuilt = CircuitDefinition {
                    circuit: result.clone(),
                    ..current.clone()
                };
                if rebuilt.interface() != current.interface() {
                    // Internal Pin placement can change the default side ordering.
                    // Freeze its existing geometry only when that is necessary.
                    result.appearance = Some(current.editable_appearance());
                }
            }
        } else if let Some(mut appearance) = current.circuit.appearance.clone() {
            let (min, max) = appearance.bounds();
            appearance.ports.clear();
            let mut rows = [0, 0];
            for pin in analysis_pins(&result) {
                let row = &mut rows[usize::from(pin.output)];
                let x = if pin.output { max.x } else { min.x };
                let at = Point::snapped(x as f32, min.y as f32) + Point::new(0, 10 * (*row + 1));
                appearance.ports.insert(pin.id, at);
                *row += 1;
            }
            result.appearance = Some(appearance);
        }
        Ok(result)
    }
}
