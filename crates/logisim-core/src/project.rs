//! Project identity and independently derived default subcircuit interfaces.
use crate::{Circuit, Component, Facing, Id, Kind, Point};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_CIRCUITS: usize = 128;

/// Text displayed inside every instance, independent of the instance label.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CircuitLabel {
    pub text: String,
    pub font: crate::ArtworkFont,
    pub up: Facing,
    pub color: [u8; 3],
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InterfacePort {
    pub pin: Id,
    pub offset: Point,
    pub bits: u8,
    pub output: bool,
    pub label: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Interface {
    pub label: CircuitLabel,
    pub name: String,
    pub appearance: Option<crate::Appearance>,
    pub ports: Vec<InterfacePort>,
    pub min: Point,
    pub max: Point,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CircuitDefinition {
    pub id: Id,
    pub name: String,
    pub circuit: Circuit,
}
impl CircuitDefinition {
    pub fn interface(&self) -> Interface {
        let Some(appearance) = &self.circuit.appearance else {
            return self.default_interface();
        };
        let mut ports = vec![];
        for pin in self
            .circuit
            .components
            .iter()
            .filter(|p| p.kind == Kind::Pin)
        {
            if let Some(at) = appearance.ports.get(&pin.id) {
                ports.push(InterfacePort {
                    pin: pin.id,
                    offset: appearance.local(*at),
                    bits: pin.bits,
                    output: pin.output,
                    label: pin.label.clone(),
                });
            }
        }
        ports.sort_by_key(|p| p.pin);
        let (a, b) = appearance.bounds();
        let corners =
            [a, b, Point::new(a.x, b.y), Point::new(b.x, a.y)].map(|p| appearance.local(p));
        Interface {
            label: self.circuit.label.clone(),
            name: self.name.clone(),
            appearance: Some(appearance.clone()),
            ports,
            min: Point::new(
                corners.iter().map(|p| p.x).min().unwrap(),
                corners.iter().map(|p| p.y).min().unwrap(),
            ),
            max: Point::new(
                corners.iter().map(|p| p.x).max().unwrap(),
                corners.iter().map(|p| p.y).max().unwrap(),
            ),
        }
    }
    pub fn default_interface(&self) -> Interface {
        // Side order is also the observed anchor preference: east, north, west, south.
        let mut sides: [Vec<&Component>; 4] = Default::default();
        for pin in self
            .circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::Pin)
        {
            let side = match pin.facing {
                Facing::West => 0,
                Facing::South => 1,
                Facing::East => 2,
                Facing::North => 3,
            };
            sides[side].push(pin);
        }
        for (i, pins) in sides.iter_mut().enumerate() {
            pins.sort_by_key(|p| {
                if i % 2 == 0 {
                    (p.position.y, p.position.x, p.id)
                } else {
                    (p.position.x, p.position.y, p.id)
                }
            });
        }
        let vertical = sides[0].len().max(sides[2].len()) as i32;
        let horizontal = sides[1].len().max(sides[3].len()) as i32;
        let width = (10 * (horizontal + i32::from(vertical > 0))).max(30);
        let height = (10 * (vertical + i32::from(horizontal > 0))).max(30);
        let mut ports = vec![];
        for (side, pins) in sides.iter().enumerate() {
            let (span, count) = if side % 2 == 0 {
                (height, vertical)
            } else {
                (width, horizontal)
            };
            let orthogonal = if side % 2 == 0 { horizontal } else { vertical };
            let start = if orthogonal > 0 {
                10
            } else {
                (span - (count - 1) * 10) / 2
            } + (count - pins.len() as i32) / 2 * 10;
            for (i, pin) in pins.iter().enumerate() {
                let along = start + i as i32 * 10;
                let offset = match side {
                    0 => Point::new(width, along),
                    1 => Point::new(along, 0),
                    2 => Point::new(0, along),
                    _ => Point::new(along, height),
                };
                ports.push(InterfacePort {
                    pin: pin.id,
                    offset,
                    bits: pin.bits.clamp(1, 32),
                    output: pin.output,
                    label: pin.label.clone(),
                });
            }
        }
        let anchor = ports.first().map(|p| p.offset).unwrap_or(Point::ZERO);
        for p in &mut ports {
            p.offset = p.offset - anchor;
        }
        // Pin identity, not side order, is the persistent association across edits.
        ports.sort_by_key(|p| p.pin);
        Interface {
            label: self.circuit.label.clone(),
            appearance: None,
            name: self.name.clone(),
            ports,
            min: Point::ZERO - anchor,
            max: Point::new(width, height) - anchor,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Project {
    pub plugin_sources: Vec<crate::plugin::PluginSource>,
    pub circuits: Vec<CircuitDefinition>,
    pub main: Id,
    next_id: Id,
}
impl PartialEq for Project {
    fn eq(&self, other: &Self) -> bool {
        self.main == other.main
            && self.circuits == other.circuits
            && self.plugin_sources == other.plugin_sources
    }
}
impl Eq for Project {}
impl Default for Project {
    fn default() -> Self {
        Self::single("main", Circuit::default())
    }
}
impl Project {
    /// Allocation history is not document content. Undo must not let clipboard references alias a new definition.
    pub fn preserve_allocations(&mut self, other: &Self) {
        self.next_id = self.next_id.max(other.next_id);
    }

    pub fn single(name: impl Into<String>, circuit: Circuit) -> Self {
        Self {
            plugin_sources: vec![],
            circuits: vec![CircuitDefinition {
                id: 1,
                name: name.into(),
                circuit,
            }],
            main: 1,
            next_id: 2,
        }
    }
    pub fn circuit(&self, id: Id) -> Option<&CircuitDefinition> {
        self.circuits.iter().find(|c| c.id == id)
    }
    pub fn circuit_mut(&mut self, id: Id) -> Option<&mut CircuitDefinition> {
        self.circuits.iter_mut().find(|c| c.id == id)
    }
    pub fn main_circuit(&self) -> &CircuitDefinition {
        self.circuit(self.main).expect("valid project main")
    }
    pub fn main_circuit_mut(&mut self) -> &mut CircuitDefinition {
        self.circuit_mut(self.main).expect("valid project main")
    }
    pub fn named(&self, name: &str) -> Option<&CircuitDefinition> {
        self.circuits.iter().find(|c| c.name == name)
    }
    pub fn valid_name(&self, name: &str, except: Option<Id>) -> bool {
        !name.trim().is_empty()
            && name.len() <= 1024
            && !name.chars().any(char::is_control)
            && !self
                .circuits
                .iter()
                .any(|c| c.name == name && Some(c.id) != except)
    }
    pub fn add(&mut self, name: String) -> Result<Id, String> {
        if self.circuits.len() >= MAX_CIRCUITS {
            return Err(format!("At most {MAX_CIRCUITS} circuits are supported."));
        }
        if !self.valid_name(&name, None) {
            return Err("Choose a nonempty, unique circuit name (at most 1024 bytes).".into());
        }
        let id = self.next_id;
        self.next_id += 1;
        self.circuits.push(CircuitDefinition {
            id,
            name,
            circuit: Circuit::default(),
        });
        Ok(id)
    }
    pub fn rename(&mut self, id: Id, name: String) -> Result<(), String> {
        if !self.valid_name(&name, Some(id)) {
            return Err("Choose a nonempty, unique circuit name.".into());
        }
        self.circuit_mut(id).ok_or("Missing circuit")?.name = name;
        self.refresh_interfaces(false);
        Ok(())
    }
    pub fn can_remove(&self, id: Id) -> bool {
        self.circuits.len() > 1
            && self.circuit(id).is_some()
            && !self.circuits.iter().any(|c| {
                c.circuit
                    .components
                    .iter()
                    .any(|c| c.kind == Kind::Subcircuit(id))
            })
    }
    pub fn remove(&mut self, id: Id) -> Result<(), String> {
        if !self.can_remove(id) {
            return Err(
                "Cannot remove the last circuit or a circuit that is still used by an instance."
                    .into(),
            );
        }
        self.circuits.retain(|c| c.id != id);
        if self.main == id {
            self.main = self.circuits[0].id;
        }
        Ok(())
    }
    pub fn can_instantiate(&self, parent: Id, target: Id) -> bool {
        if self.circuit(parent).is_none() || self.circuit(target).is_none() {
            return false;
        }
        let mut stack = vec![target];
        let mut visited = BTreeSet::new();
        while let Some(id) = stack.pop() {
            if id == parent {
                return false;
            }
            if visited.insert(id)
                && let Some(c) = self.circuit(id)
            {
                stack.extend(c.circuit.components.iter().filter_map(|c| {
                    if let Kind::Subcircuit(id) = c.kind {
                        Some(id)
                    } else {
                        None
                    }
                }));
            }
        }
        true
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.plugin_sources.len() > 64 {
            return Err("Too many plugin sources.".into());
        }
        let mut plugins = BTreeSet::new();
        for source in &self.plugin_sources {
            source.validate()?;
            if !plugins.insert((&source.plugin, &source.version)) {
                return Err("Duplicate plugin source.".into());
            }
        }
        if self.circuits.is_empty()
            || self.circuits.len() > MAX_CIRCUITS
            || self.circuit(self.main).is_none()
        {
            return Err("Invalid project circuit list or main circuit.".into());
        }
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        for c in &self.circuits {
            if !ids.insert(c.id) || !names.insert(&c.name) || !self.valid_name(&c.name, Some(c.id))
            {
                return Err("Invalid or duplicate circuit identity/name.".into());
            }
            if let Some(a) = &c.circuit.appearance {
                a.validate(
                    &c.circuit
                        .components
                        .iter()
                        .filter(|p| p.kind == Kind::Pin)
                        .map(|p| p.id)
                        .collect(),
                )?;
            }
            for component in &c.circuit.components {
                if component.kind == Kind::Plugin {
                    let definition = component
                        .plugin
                        .as_ref()
                        .ok_or("Missing plugin definition.")?;
                    definition.validate()?;
                    plugins.insert((&definition.plugin, &definition.version));
                    if plugins.len() > 64 {
                        return Err("Too many plugin sources.".into());
                    }
                    if component.facing != Facing::East {
                        return Err("Plugin ABI v1 components must face East.".into());
                    }
                } else if component.plugin.is_some() {
                    return Err("Plugin definition on a non-plugin component.".into());
                }
                if let Kind::Subcircuit(target) = component.kind
                    && !self.can_instantiate(c.id, target)
                {
                    return Err(format!("Missing or recursive subcircuit in '{}'.", c.name));
                }
            }
        }
        Ok(())
    }
    /// Import uses false: file coordinates already refer to the final interface.
    /// Default-interface Pin edits reroute surviving connections. Custom appearance edits
    /// keep file/layout wire coordinates, matching the observed appearance editor.
    pub fn refresh_interfaces(&mut self, reroute: bool) {
        if reroute {
            for c in &mut self.circuits {
                c.reconcile_appearance();
            }
        }
        let interfaces: BTreeMap<_, _> = self
            .circuits
            .iter()
            .map(|c| (c.id, c.interface()))
            .collect();
        for definition in &mut self.circuits {
            let replacements: Vec<_> = definition
                .circuit
                .components
                .iter()
                .filter_map(|c| {
                    if let Kind::Subcircuit(id) = c.kind
                        && let Some(interface) = interfaces.get(&id)
                        && &c.interface != interface
                    {
                        let mut c = c.clone();
                        c.interface = interface.clone();
                        Some(c)
                    } else {
                        None
                    }
                })
                .collect();
            for c in replacements {
                let default_interface_change = c.interface.appearance.is_none()
                    && definition
                        .circuit
                        .component(c.id)
                        .is_some_and(|old| old.interface.appearance.is_none());
                if reroute && default_interface_change {
                    definition.circuit.replace_component(c);
                } else {
                    let id = c.id;
                    *definition
                        .circuit
                        .components
                        .iter_mut()
                        .find(|old| old.id == id)
                        .unwrap() = c;
                }
            }
        }
    }
    pub fn instance(&self, target: Id, position: Point) -> Option<Component> {
        let definition = self.circuit(target)?;
        let mut c = Component::new(Kind::Subcircuit(target), position);
        c.interface = definition.interface();
        if let Some(a) = &c.interface.appearance {
            c.facing = a.facing;
        }
        Some(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pins(groups: &[(Facing, usize)]) -> CircuitDefinition {
        let mut d = CircuitDefinition {
            id: 1,
            name: "part".into(),
            circuit: Circuit::default(),
        };
        for (side, (facing, count)) in groups.iter().enumerate() {
            for i in 0..*count {
                let mut c = Component::new(
                    Kind::Pin,
                    Point::new(side as i32 * 100 + i as i32 * 20, i as i32 * 30),
                );
                c.facing = *facing;
                c.output = *facing == Facing::West;
                d.circuit.add_component(c);
            }
        }
        d
    }
    #[test]
    fn observed_default_interfaces_and_rotations() {
        let mixed = pins(&[(Facing::East, 3), (Facing::West, 2)]).interface();
        assert_eq!(
            mixed.ports.iter().map(|p| p.offset).collect::<Vec<_>>(),
            vec![
                Point::new(-30, 0),
                Point::new(-30, 10),
                Point::new(-30, 20),
                Point::ZERO,
                Point::new(0, 10)
            ]
        );
        assert_eq!(
            (mixed.min, mixed.max),
            (Point::new(-30, -5), Point::new(0, 25))
        );
        let northwest = pins(&[(Facing::East, 2), (Facing::South, 1)]).interface();
        assert_eq!(
            northwest.ports.iter().map(|p| p.offset).collect::<Vec<_>>(),
            vec![Point::new(-10, 10), Point::new(-10, 20), Point::ZERO]
        );
        let uneven = pins(&[(Facing::East, 5), (Facing::West, 1)]).interface();
        assert_eq!(uneven.ports[0].offset, Point::new(-30, -20));
        assert_eq!(uneven.ports[4].offset, Point::new(-30, 20));
        let mut c = Component::new(Kind::Subcircuit(1), Point::new(200, 200));
        c.interface = mixed;
        for facing in Facing::ALL {
            c.facing = facing;
            assert_eq!(
                c.ports()[1],
                c.position + facing.transform(Point::new(-30, 10))
            );
        }
        let empty = pins(&[]).interface();
        assert!(empty.ports.is_empty());
        assert_eq!(empty.min, Point::ZERO);
        assert_eq!(empty.max, Point::new(30, 30));
    }
    #[test]
    fn references_survive_rename_and_disallow_recursive_or_used_deletion() {
        let mut p = Project::default();
        let a = p.add("A".into()).unwrap();
        let b = p.add("B".into()).unwrap();
        let c = p.instance(a, Point::ZERO).unwrap();
        p.main_circuit_mut().circuit.add_component(c);
        let c = p.instance(b, Point::ZERO).unwrap();
        p.circuit_mut(a).unwrap().circuit.add_component(c);
        assert!(!p.can_instantiate(b, 1));
        assert!(!p.can_instantiate(a, a));
        assert!(p.can_instantiate(1, b));
        assert!(p.remove(a).is_err());
        assert!(p.rename(b, "A".into()).is_err());
        p.rename(a, "Renamed".into()).unwrap();
        assert_eq!(
            p.main_circuit().circuit.components[0].interface.name,
            "Renamed"
        );
        p.validate().unwrap();
        p.main = b;
        p.remove(1).unwrap();
        assert_eq!(p.main, b);
    }
    #[test]
    fn changing_pin_order_preserves_connections_by_pin_identity() {
        let mut p = Project::default();
        let sub = p.add("pair".into()).unwrap();
        p.circuit_mut(sub).unwrap().circuit = pins(&[(Facing::East, 2), (Facing::West, 1)]).circuit;
        let c = p.instance(sub, Point::new(200, 200)).unwrap();
        let pin = c.interface.ports[0].pin;
        let at = c.ports()[0];
        let id = p.main_circuit_mut().circuit.add_component(c);
        p.main_circuit_mut()
            .circuit
            .add_wire(Point::new(100, at.y), at);
        p.circuit_mut(sub).unwrap().circuit.components[0].position.y = 500;
        p.refresh_interfaces(true);
        let c = p.main_circuit().circuit.component(id).unwrap();
        let index = c.interface.ports.iter().position(|p| p.pin == pin).unwrap();
        let after = c.ports()[index];
        assert_ne!(at, after);
        assert!(
            p.main_circuit()
                .circuit
                .wires
                .iter()
                .any(|w| w.a == after || w.b == after)
        );
        assert_eq!(c.interface.ports[index].pin, pin);
    }
}
