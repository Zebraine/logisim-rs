//! Canonical straight segments, retaining explicit electrical junctions.
use crate::{Circuit, Component, Id, Point, Wire, WireIndex};
use std::collections::{BTreeMap, BTreeSet};

type Line = (bool, i32);
fn line(w: Wire) -> Line {
    (w.horizontal(), if w.horizontal() { w.a.y } else { w.a.x })
}
fn coordinate(p: Point, horizontal: bool) -> i32 {
    if horizontal { p.x } else { p.y }
}
fn point(line: Line, at: i32) -> Point {
    if line.0 {
        Point::new(at, line.1)
    } else {
        Point::new(line.1, at)
    }
}
fn interval(w: Wire) -> (i32, i32) {
    let a = coordinate(w.a, w.horizontal());
    let b = coordinate(w.b, w.horizontal());
    (a.min(b), a.max(b))
}
impl Circuit {
    /// Run after a complete edit/import, when every wire and terminal is available.
    /// Returns old wire IDs mapped to their surviving segments for selection/history.
    pub fn normalize_wires(&mut self) -> BTreeMap<Id, BTreeSet<Id>> {
        let mut groups: BTreeMap<Line, Vec<Wire>> = BTreeMap::new();
        for w in &self.wires {
            if w.a != w.b {
                groups.entry(line(*w)).or_default().push(*w);
            }
        }
        let mut nodes: BTreeSet<_> = self.components.iter().flat_map(Component::ports).collect();
        for w in &self.wires {
            for p in [w.a, w.b] {
                let perpendicular = (!w.horizontal(), coordinate(p, w.horizontal()));
                if groups
                    .get(&perpendicular)
                    .is_some_and(|wires| wires.iter().any(|w| w.contains(p)))
                {
                    nodes.insert(p);
                }
            }
        }
        let mut stops: BTreeMap<Line, BTreeSet<i32>> = BTreeMap::new();
        for p in nodes {
            stops.entry((true, p.y)).or_default().insert(p.x);
            stops.entry((false, p.x)).or_default().insert(p.y);
        }
        let mut mapped: BTreeMap<Id, BTreeSet<Id>> = BTreeMap::new();
        let mut used = BTreeSet::new();
        let mut result = Vec::new();
        for (line, mut wires) in groups {
            wires.sort_by_key(|w| (interval(*w), w.id));
            let mut unions: Vec<(i32, i32)> = Vec::new();
            for w in &wires {
                let (a, b) = interval(*w);
                if let Some(last) = unions.last_mut()
                    && a <= last.1
                {
                    last.1 = last.1.max(b);
                } else {
                    unions.push((a, b));
                }
            }
            for (a, b) in unions {
                let mut cuts = vec![a, b];
                if let Some(nodes) = stops.get(&line) {
                    cuts.extend(nodes.range(a..=b));
                }
                cuts.sort_unstable();
                cuts.dedup();
                for ends in cuts.windows(2) {
                    let (a, b) = (ends[0], ends[1]);
                    let members: Vec<_> = wires
                        .iter()
                        .copied()
                        .filter(|w| {
                            let (start, end) = interval(*w);
                            start < b && end > a
                        })
                        .collect();
                    // Exact segments retain identity. A merge reuses one ID; a split allocates
                    // only the extra IDs. Repeating normalization is therefore a no-op.
                    let id = members
                        .iter()
                        .find(|w| interval(**w) == (a, b) && !used.contains(&w.id))
                        .or_else(|| members.iter().find(|w| !used.contains(&w.id)))
                        .map(|w| w.id)
                        .unwrap_or_else(|| self.id());
                    used.insert(id);
                    for old in members {
                        mapped.entry(old.id).or_default().insert(id);
                    }
                    result.push(Wire {
                        id,
                        a: point(line, a),
                        b: point(line, b),
                    });
                }
            }
        }
        result.sort_by_key(|w| w.id);
        self.wires = result;
        mapped
    }

    /// Open wire ends, excluding component terminals, elbows and attached branches.
    pub fn dangling_ends(&self) -> Vec<Point> {
        let index = WireIndex::new(&self.wires);
        let ports: BTreeSet<_> = self.components.iter().flat_map(Component::ports).collect();
        let ends: BTreeSet<_> = self.wires.iter().flat_map(|w| [w.a, w.b]).collect();
        ends.into_iter()
            .filter(|p| {
                if ports.contains(p) {
                    return false;
                }
                let mut count = 0;
                index.visit(*p, |_, _| count += 1);
                count == 1
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Kind;
    fn geometry(c: &Circuit) -> BTreeSet<(Point, Point)> {
        c.wires.iter().map(|w| (w.a, w.b)).collect()
    }
    #[test]
    fn touching_overlapping_reversed_and_contained_segments_merge_without_closing_gaps() {
        let mut c = Circuit::default();
        let ids: Vec<_> = [(0, 40), (100, 40), (20, 60), (25, 30)]
            .into_iter()
            .map(|(a, b)| c.add_wire(Point::new(a, 0), Point::new(b, 0)).unwrap())
            .collect();
        let gap = c.add_wire(Point::new(110, 0), Point::new(150, 0)).unwrap();
        c.add_wire(Point::new(200, 0), Point::new(200, 60));
        c.add_wire(Point::new(200, 100), Point::new(200, 40));
        let mapped = c.normalize_wires();
        assert_eq!(
            geometry(&c),
            BTreeSet::from([
                (Point::ZERO, Point::new(100, 0)),
                (Point::new(110, 0), Point::new(150, 0)),
                (Point::new(200, 0), Point::new(200, 100)),
            ])
        );
        for id in ids {
            assert_eq!(mapped[&id], BTreeSet::from([c.wires[0].id]));
        }
        assert!(!c.connected_wires(c.wires[0].id).contains(&gap));
        let canonical = c.clone();
        c.normalize_wires();
        assert_eq!(c, canonical);
    }
    #[test]
    fn junctions_and_terminals_survive_merging_but_plain_crossings_stay_insulated() {
        let mut c = Circuit::default();
        c.add_wire(Point::new(0, 50), Point::new(50, 50));
        c.add_wire(Point::new(50, 50), Point::new(100, 50));
        c.add_wire(Point::new(50, 0), Point::new(50, 100));
        c.add_wire(Point::new(200, 50), Point::new(300, 50));
        c.add_wire(Point::new(250, 0), Point::new(250, 100));
        c.add_wire(Point::new(400, 50), Point::new(500, 50));
        let pin = c.add_component(Component::new(Kind::Pin, Point::new(450, 50)));
        c.normalize_wires();
        assert_eq!(c.wires.len(), 8); // four-way node, insulated pair, terminal split
        assert_eq!(c.junctions(), vec![Point::new(50, 50), Point::new(450, 50)]);
        let wire_at = |p| c.wires.iter().find(|w| w.contains(p)).unwrap().id;
        assert_eq!(c.connected_wires(wire_at(Point::new(0, 50))).len(), 4);
        assert_eq!(c.connected_wires(wire_at(Point::new(200, 50))).len(), 1);
        let original = c.clone();
        c.normalize_wires();
        assert_eq!(c, original);
        c.remove(&BTreeSet::from([pin]));
        c.normalize_wires();
        assert_eq!(c.wires.len(), 7);
        assert!(geometry(&c).contains(&(Point::new(400, 50), Point::new(500, 50))));
    }
    #[test]
    fn dangling_markers_exclude_ports_elbows_and_branches_and_track_disconnection() {
        let mut c = Circuit::default();
        let pin = c.add_component(Component::new(Kind::Pin, Point::ZERO));
        c.add_wire(Point::ZERO, Point::new(100, 0));
        let branch = c.add_wire(Point::new(50, 0), Point::new(50, 50)).unwrap();
        c.add_wire(Point::new(100, 0), Point::new(100, 50));
        c.add_wire(Point::new(200, 0), Point::new(250, 0));
        c.normalize_wires();
        assert_eq!(
            c.dangling_ends(),
            vec![
                Point::new(50, 50),
                Point::new(100, 50),
                Point::new(200, 0),
                Point::new(250, 0)
            ]
        );
        assert_eq!(c.junctions(), vec![Point::new(50, 0)]);
        c.remove(&BTreeSet::from([pin, branch]));
        c.normalize_wires();
        assert!(c.dangling_ends().contains(&Point::ZERO));
        assert!(c.junctions().is_empty());
        assert!(geometry(&c).contains(&(Point::ZERO, Point::new(100, 0))));
    }
}
