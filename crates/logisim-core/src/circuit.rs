//! Circuit ownership, stable item identity and atomic geometry edits.
#[cfg(test)]
mod tests;
mod wires;

use crate::{Appearance, CircuitLabel, Component, Id, Kind, Point, Wire, WireIndex};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Circuit {
    pub agent_ids: crate::AgentIds,
    pub label: CircuitLabel,
    pub appearance: Option<Appearance>,
    pub components: Vec<Component>,
    pub wires: Vec<Wire>,
    next_id: Id,
}
impl Circuit {
    fn id(&mut self) -> Id {
        self.next_id += 1;
        self.next_id
    }
    pub fn add_component(&mut self, mut component: Component) -> Id {
        component.id = self.id();
        let id = component.id;
        self.components.push(component);
        id
    }
    pub fn component(&self, id: Id) -> Option<&Component> {
        self.components.iter().find(|c| c.id == id)
    }
    pub fn ids(&self) -> BTreeSet<Id> {
        self.components
            .iter()
            .map(|c| c.id)
            .chain(self.wires.iter().map(|w| w.id))
            .collect()
    }
    pub(crate) fn retain_agent_ids(&mut self) {
        if !self.agent_ids.items.is_empty() {
            let live = self.ids();
            self.agent_ids.items.retain(|id, _| live.contains(id));
        }
    }
    pub fn remove(&mut self, ids: &BTreeSet<Id>) {
        self.components.retain(|c| !ids.contains(&c.id));
        self.wires.retain(|w| !ids.contains(&w.id));
        for id in ids {
            self.agent_ids.items.remove(id);
        }
    }
    pub fn add_wire(&mut self, a: Point, b: Point) -> Option<Id> {
        if a == b || (a.x != b.x && a.y != b.y) {
            return None;
        }
        if let Some(w) = self
            .wires
            .iter()
            .find(|w| (w.a == a && w.b == b) || (w.a == b && w.b == a))
        {
            return Some(w.id);
        }
        let id = self.id();
        self.wires.push(Wire { id, a, b });
        Some(id)
    }
    pub fn route(a: Point, b: Point, horizontal_first: bool) -> [Point; 3] {
        [
            a,
            if horizontal_first {
                Point::new(b.x, a.y)
            } else {
                Point::new(a.x, b.y)
            },
            b,
        ]
    }
    pub fn add_route(&mut self, a: Point, b: Point, horizontal_first: bool) -> Vec<Id> {
        Self::route(a, b, horizontal_first)
            .windows(2)
            .filter_map(|p| self.add_wire(p[0], p[1]))
            .collect()
    }
    /// Split at terminals before rerouting, so a terminal on a wire's interior stays connected.
    fn split_at(&mut self, points: &[Point], skip: &BTreeSet<Id>) {
        let old = std::mem::take(&mut self.wires);
        for wire in old {
            if skip.contains(&wire.id) {
                self.wires.push(wire);
                continue;
            }
            let mut stops = vec![wire.a, wire.b];
            stops.extend(points.iter().copied().filter(|p| wire.contains(*p)));
            stops.sort();
            stops.dedup();
            for (index, pair) in stops.windows(2).enumerate() {
                let id = if index == 0 { wire.id } else { self.id() };
                self.wires.push(Wire {
                    id,
                    a: pair[0],
                    b: pair[1],
                });
            }
        }
    }
    pub fn move_selection(&mut self, ids: &BTreeSet<Id>, delta: Point) {
        if delta == Point::ZERO {
            return;
        }
        let mut old_ports: Vec<_> = self
            .components
            .iter()
            .filter(|c| ids.contains(&c.id))
            .flat_map(Component::ports)
            .collect();
        let moving_wires: Vec<_> = self
            .wires
            .iter()
            .filter(|w| ids.contains(&w.id))
            .copied()
            .collect();
        old_ports.extend(
            self.wires
                .iter()
                .flat_map(|w| [w.a, w.b])
                .chain(self.components.iter().flat_map(Component::ports))
                .filter(|point| moving_wires.iter().any(|w| w.contains(*point))),
        );
        old_ports.sort();
        old_ports.dedup();
        self.split_at(&old_ports, ids);
        let fixed: BTreeSet<_> = self
            .components
            .iter()
            .filter(|c| !ids.contains(&c.id))
            .flat_map(Component::ports)
            .collect();
        for c in &mut self.components {
            if ids.contains(&c.id) {
                c.position = c.position + delta;
            }
        }
        let map: BTreeMap<_, _> = old_ports
            .iter()
            .filter(|p| !fixed.contains(p))
            .map(|p| (*p, *p + delta))
            .collect();
        self.reroute(&map, ids, delta);
        for point in old_ports.iter().filter(|p| fixed.contains(p)) {
            self.add_route(*point, *point + delta, true);
        }
    }
    /// Apply an attribute edit using the component family's surviving connection mapping.
    pub fn replace_component(&mut self, replacement: Component) {
        let Some(old) = self.component(replacement.id).cloned() else {
            return;
        };
        if old == replacement {
            return;
        }
        let map = old.connection_map(&replacement);
        if !map.is_empty() {
            self.split_at(&old.ports(), &BTreeSet::new());
        }
        *self.components.iter_mut().find(|c| c.id == old.id).unwrap() = replacement;
        self.reroute(&map, &BTreeSet::new(), Point::ZERO);
    }
    fn reroute(&mut self, ports: &BTreeMap<Point, Point>, selected: &BTreeSet<Id>, delta: Point) {
        let old = std::mem::take(&mut self.wires);
        for wire in old {
            let (a, b) = if selected.contains(&wire.id) {
                (wire.a + delta, wire.b + delta)
            } else {
                (
                    *ports.get(&wire.a).unwrap_or(&wire.a),
                    *ports.get(&wire.b).unwrap_or(&wire.b),
                )
            };
            let route = Self::route(a, b, wire.horizontal());
            let mut first = true;
            for pair in route.windows(2) {
                if pair[0] != pair[1] {
                    let id = if first { wire.id } else { self.id() };
                    self.wires.push(Wire {
                        id,
                        a: pair[0],
                        b: pair[1],
                    });
                    first = false;
                }
            }
        }
    }
    pub fn junctions(&self) -> Vec<Point> {
        let index = WireIndex::new(&self.wires);
        let ports: BTreeSet<_> = self.components.iter().flat_map(Component::ports).collect();
        let points: BTreeSet<_> = self
            .wires
            .iter()
            .flat_map(|w| [w.a, w.b])
            .chain(ports.iter().copied())
            .collect();
        points
            .into_iter()
            .filter(|p| {
                let mut branches = 0;
                index.visit(*p, |_, w| {
                    branches += if w.a == *p || w.b == *p { 1 } else { 2 };
                });
                branches + usize::from(ports.contains(p)) >= 3
            })
            .collect()
    }
    /// Topological highlighting only; no values, propagation or simulation semantics.
    pub fn connected_wires(&self, seed: Id) -> BTreeSet<Id> {
        let mut net = self.connected_wire_selection(&BTreeSet::from([seed]));
        net.insert(seed);
        net
    }

    /// Union of the physical nets of all selected wires (ignoring selected components).
    pub fn connected_wire_selection(&self, selection: &BTreeSet<Id>) -> BTreeSet<Id> {
        if selection.is_empty() || !self.wires.iter().any(|w| selection.contains(&w.id)) {
            return BTreeSet::new();
        }
        let ports: Vec<_> = self
            .components
            .iter()
            .flat_map(|c| {
                let tunnel =
                    (c.kind == Kind::Tunnel && !c.label.is_empty()).then_some(c.label.as_str());
                c.ports().into_iter().map(move |point| (point, tunnel))
            })
            .collect();
        let mut groups = crate::ConnectionGroups::from_geometry(&self.wires, ports.into_iter());
        let roots: BTreeSet<_> = self
            .wires
            .iter()
            .enumerate()
            .filter(|(_, w)| selection.contains(&w.id))
            .map(|(index, _)| groups.root(index))
            .collect();
        self.wires
            .iter()
            .enumerate()
            .filter(|(index, _)| roots.contains(&groups.root(*index)))
            .map(|(_, wire)| wire.id)
            .collect()
    }
}
