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
