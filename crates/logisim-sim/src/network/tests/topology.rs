use super::super::*;
use logisim_core::{Component, Gate, Point, Wire};

// Independent reference: exhaustively build the old geometric relation, then flood-fill
// it. This intentionally has no spatial index or union-find shared with the compiler.
fn compare(c: &Circuit) {
    let ports: Vec<_> = c
        .components
        .iter()
        .flat_map(|c| {
            c.ports()
                .into_iter()
                .enumerate()
                .map(move |(i, p)| (c, i, p))
        })
        .collect();
    let offset = c.wires.len();
    let mut edges = vec![vec![]; offset + ports.len()];
    let mut edge = |a: usize, b: usize| {
        edges[a].push(b);
        edges[b].push(a);
    };
    for (i, a) in c.wires.iter().enumerate() {
        for (j, b) in c.wires[..i].iter().enumerate() {
            if a.touches(*b) {
                edge(i, j);
            }
        }
    }
    for (i, (a, _, point)) in ports.iter().enumerate() {
        for (j, w) in c.wires.iter().enumerate() {
            if w.contains(*point) {
                edge(offset + i, j);
            }
        }
        for (j, (b, _, other)) in ports[..i].iter().enumerate() {
            if point == other
                || (a.kind == Kind::Tunnel
                    && b.kind == Kind::Tunnel
                    && !a.label.is_empty()
                    && a.label == b.label)
            {
                edge(offset + i, offset + j);
            }
        }
    }
    let mut seen = vec![false; edges.len()];
    let mut expected = vec![];
    for seed in 0..edges.len() {
        if seen[seed] {
            continue;
        }
        let mut stack = vec![seed];
        let mut members = vec![];
        while let Some(i) = stack.pop() {
            if std::mem::replace(&mut seen[i], true) {
                continue;
            }
            members.push(i);
            stack.extend(&edges[i]);
        }
        members.sort_unstable();
        expected.push(members);
    }
    let actual = Network::compile(c);
    assert_eq!(actual.nets.len(), expected.len());
    for (n, members) in expected.iter().enumerate() {
        let net = &actual.nets[n];
        let mut wires = vec![];
        let mut pins = vec![];
        let mut widths = BTreeSet::new();
        for &i in members {
            if i < offset {
                let id = c.wires[i].id;
                wires.push(id);
                assert_eq!(actual.wires[&id], n);
            } else {
                let (c, index, _) = ports[i - offset];
                pins.push((c.id, index));
                assert_eq!(actual.ports[&(c.id, index)], n);
                if let Some(width) = c.port_width(index).filter(|w| *w > 0) {
                    widths.insert(width);
                }
                assert_eq!(actual.connected((c.id, index)), members.len() > 1);
            }
        }
        assert_eq!(net.wires, wires, "wire/document order");
        assert_eq!(net.ports, pins, "port/document order");
        assert_eq!(net.width, widths.last().copied().unwrap_or(0));
        assert_eq!(net.mismatch, widths.len() > 1);
    }
    let terminals: BTreeSet<_> = ports.iter().map(|(_, _, p)| *p).collect();
    let ends: BTreeSet<_> = c.wires.iter().flat_map(|w| [w.a, w.b]).collect();
    let junctions: Vec<_> = ends
        .union(&terminals)
        .copied()
        .filter(|p| {
            c.wires
                .iter()
                .filter(|w| w.contains(*p))
                .map(|w| if w.a == *p || w.b == *p { 1 } else { 2 })
                .sum::<usize>()
                + usize::from(terminals.contains(p))
                >= 3
        })
        .collect();
    let dangling: Vec<_> = ends
        .into_iter()
        .filter(|p| {
            !terminals.contains(p) && c.wires.iter().filter(|w| w.contains(*p)).take(2).count() == 1
        })
        .collect();
    assert_eq!(c.junctions(), junctions);
    assert_eq!(c.dangling_ends(), dangling);
}

#[test]
fn topology_matches_linear_rules_for_crossings_terminals_tunnels_and_edits() {
    let mut c = Circuit::default();
    let horizontal = c.add_wire(Point::new(-100, 0), Point::new(100, 0)).unwrap();
    let vertical = c.add_wire(Point::new(0, -100), Point::new(0, 100)).unwrap();
    compare(&c);
    let net = Network::compile(&c);
    assert_ne!(net.wires[&horizontal], net.wires[&vertical]);
    let pin = c.add_component(Component::new(Kind::Pin, Point::ZERO));
    compare(&c);
    let net = Network::compile(&c);
    assert_eq!(net.wires[&horizontal], net.wires[&vertical]);
    c.remove(&BTreeSet::from([pin]));
    let branch = c.add_wire(Point::ZERO, Point::new(50, 0)).unwrap();
    compare(&c);
    let net = Network::compile(&c);
    assert_eq!(net.wires[&horizontal], net.wires[&vertical]);
    c.remove(&BTreeSet::from([branch]));
    for (i, label) in ["bus", "bus", "Bus", "", ""].into_iter().enumerate() {
        let mut t = Component::new(Kind::Tunnel, Point::new(i as i32 * 100, 200));
        t.label = label.into();
        t.bits = if i == 1 { 4 } else { 1 };
        c.add_component(t);
    }
    compare(&c);
    c.components[1].label = "Bus".into();
    compare(&c);
    c.components.reverse();
    c.wires.reverse();
    compare(&c);
    c.normalize_wires();
    compare(&c);
    compare(&Circuit::default());
}

#[test]
fn randomized_topology_matches_linear_oracle_including_splitters_and_duplicate_wires() {
    let mut state = 42_u64;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (state >> 32) as u32
    };
    for _ in 0..128 {
        let mut c = Circuit::default();
        for _ in 0..60 {
            let a = Point::new(
                (next() % 21) as i32 * 10 - 100,
                (next() % 21) as i32 * 10 - 100,
            );
            let b = if next() % 2 == 0 {
                Point::new(a.x, (next() % 21) as i32 * 10 - 100)
            } else {
                Point::new((next() % 21) as i32 * 10 - 100, a.y)
            };
            c.add_wire(a, b);
        }
        // Public documents can retain duplicate/zero-length segments before normalization.
        c.wires.push(Wire {
            id: 1000,
            a: Point::ZERO,
            b: Point::ZERO,
        });
        if let Some(w) = c.wires.first().copied() {
            c.wires.push(Wire { id: 1001, ..w });
        }
        for i in 0..40 {
            let kind = [
                Kind::Pin,
                Kind::Tunnel,
                Kind::Gate(Gate::And),
                Kind::Splitter,
                Kind::Text,
            ][(next() % 5) as usize];
            let mut comp = Component::new(
                kind,
                Point::new(
                    (next() % 21) as i32 * 10 - 100,
                    (next() % 21) as i32 * 10 - 100,
                ),
            );
            comp.facing = logisim_core::Facing::ALL[(next() % 4) as usize];
            if kind == Kind::Tunnel {
                comp.label = ["", "bus", "Bus", "other", "总线"][(next() % 5) as usize].into();
                comp.bits = if i % 2 == 0 { 1 } else { 8 };
            }
            c.add_component(comp);
        }
        compare(&c);
        c.wires.reverse();
        c.components.reverse();
        compare(&c);
        c.normalize_wires();
        compare(&c);
    }
}
