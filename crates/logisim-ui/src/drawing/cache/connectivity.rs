use logisim_core::{Circuit, Component, Id, Kind, WireIndex};
use std::collections::{BTreeMap, BTreeSet};

/// Highlight all wires reachable from the selection. Wire endpoints and component ports
/// are junctions; two crossing interiors alone are not. Equal named tunnels link nets.
pub(super) fn selected_wires(circuit: &Circuit, selection: &BTreeSet<Id>) -> BTreeSet<Id> {
    if selection.is_empty() {
        return BTreeSet::new();
    }
    let wires = &circuit.wires;
    let mut pending: Vec<_> = wires
        .iter()
        .enumerate()
        .filter(|(_, w)| selection.contains(&w.id))
        .map(|(i, _)| i)
        .collect();
    if pending.is_empty() {
        return BTreeSet::new();
    }

    let index = WireIndex::new(wires);
    let mut neighbors = vec![Vec::new(); wires.len()];
    let mut link = |a: usize, b: usize| {
        if a != b {
            neighbors[a].push(b);
            neighbors[b].push(a);
        }
    };
    let endpoints = wires.iter().flat_map(|w| [w.a, w.b]);
    let ports = circuit.components.iter().flat_map(Component::ports);
    let mut contacts = BTreeMap::new();
    for point in endpoints.chain(ports) {
        contacts.entry(point).or_insert_with(|| {
            // A star connects all wires here, including a terminal on a crossing.
            let mut first = None;
            index.visit(point, |i, _| {
                if let Some(other) = first {
                    link(i, other);
                } else {
                    first = Some(i);
                }
            });
            first
        });
    }
    let mut tunnels = BTreeMap::new();
    for c in &circuit.components {
        if c.kind == Kind::Tunnel
            && !c.label.is_empty()
            && let Some(&Some(wire)) = contacts.get(&c.position)
            && let Some(previous) = tunnels.insert(&c.label, wire)
        {
            link(wire, previous);
        }
    }
    let mut result = BTreeSet::new();
    while let Some(i) = pending.pop() {
        if result.insert(wires[i].id) {
            pending.extend_from_slice(&neighbors[i]);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use logisim_core::Point;

    #[test]
    fn multi_wire_highlight_joins_a_closed_net_and_terminal_crossings_once() {
        let mut c = Circuit::default();
        let top = c.add_wire(Point::ZERO, Point::new(100, 0)).unwrap();
        let right = c.add_wire(Point::new(100, 0), Point::new(100, 80)).unwrap();
        let bottom = c.add_wire(Point::new(100, 80), Point::new(0, 80)).unwrap();
        let left = c.add_wire(Point::new(0, 80), Point::ZERO).unwrap();
        let crossing = c.add_wire(Point::new(50, -10), Point::new(50, 20)).unwrap();
        let other = c.add_wire(Point::new(200, 0), Point::new(300, 0)).unwrap();
        let selected = BTreeSet::from([top, left, other]);
        assert_eq!(
            selected_wires(&c, &selected),
            BTreeSet::from([top, left, right, bottom, other])
        );
        let pin = c.add_component(Component::new(Kind::Pin, Point::new(50, 0)));
        assert_eq!(
            selected_wires(&c, &BTreeSet::from([top, pin])),
            BTreeSet::from([top, left, right, bottom, crossing])
        );
        assert!(selected_wires(&c, &BTreeSet::from([pin])).is_empty());
        for (x, name) in [(100, "link"), (280, "link")] {
            let mut tunnel = Component::new(Kind::Tunnel, Point::new(x, 0));
            tunnel.label = name.to_owned();
            c.add_component(tunnel);
        }
        let highlighted = selected_wires(&c, &BTreeSet::from([top]));
        assert!(highlighted.contains(&other));
        assert_eq!(highlighted, c.connected_wires(top));
    }
}
