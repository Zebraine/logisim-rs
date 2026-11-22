//! Immutable point queries for orthogonal wire geometry, independent of connectivity rules.
use crate::{Point, Wire};
use std::collections::BTreeMap;

/// An index borrowed from one wire slice. Rebuild after editing or replacing that slice.
/// Each segment occupies one interval regardless of length; queries include both ends.
/// A point on two crossing interiors visits both wires, but does not itself connect them.
pub struct WireIndex<'a> {
    wires: &'a [Wire],
    rows: BTreeMap<i32, Vec<Interval>>,
    columns: BTreeMap<i32, Vec<Interval>>,
}
struct Interval {
    start: i32,
    end: i32,
    wire: usize,
    /// Maximum end in the balanced subtree whose root is this slice's midpoint.
    max_end: i32,
}
impl<'a> WireIndex<'a> {
    pub fn new(wires: &'a [Wire]) -> Self {
        let mut index = Self {
            wires,
            rows: BTreeMap::new(),
            columns: BTreeMap::new(),
        };
        for (i, wire) in wires.iter().enumerate() {
            let (lines, line, a, b) = if wire.horizontal() {
                (&mut index.rows, wire.a.y, wire.a.x, wire.b.x)
            } else {
                (&mut index.columns, wire.a.x, wire.a.y, wire.b.y)
            };
            lines.entry(line).or_default().push(Interval {
                start: a.min(b),
                end: a.max(b),
                wire: i,
                max_end: a.max(b),
            });
        }
        for intervals in index.rows.values_mut().chain(index.columns.values_mut()) {
            intervals.sort_unstable_by_key(|i| (i.start, i.end, i.wire));
            augment(intervals);
        }
        index
    }

    /// Visit each containing segment once, with its original slice index. Order is unspecified.
    /// No result allocation, distance arithmetic, or grid-size assumption is needed.
    pub fn visit(&self, point: Point, mut visitor: impl FnMut(usize, &'a Wire)) {
        let mut emit = |i| visitor(i, &self.wires[i]);
        if let Some(line) = self.rows.get(&point.y) {
            query(line, point.x, &mut emit);
        }
        if let Some(line) = self.columns.get(&point.x) {
            query(line, point.y, &mut emit);
        }
    }
}

fn augment(intervals: &mut [Interval]) -> i32 {
    if intervals.is_empty() {
        return i32::MIN;
    }
    let mid = intervals.len() / 2;
    let (left, rest) = intervals.split_at_mut(mid);
    let (root, right) = rest.split_first_mut().unwrap();
    root.max_end = root.end.max(augment(left)).max(augment(right));
    root.max_end
}
fn query(intervals: &[Interval], at: i32, visit: &mut impl FnMut(usize)) {
    if intervals.is_empty() {
        return;
    }
    let mid = intervals.len() / 2;
    let root = &intervals[mid];
    if at < intervals[0].start || at > root.max_end {
        return;
    }
    query(&intervals[..mid], at, visit);
    if at < root.start {
        return;
    }
    if at <= root.end {
        visit(root.wire);
    }
    query(&intervals[mid + 1..], at, visit);
}

/// Disjoint connection groups. Physical nodes are indexed by wire order followed by port order.
pub struct ConnectionGroups(Vec<usize>);
impl ConnectionGroups {
    pub fn new(count: usize) -> Self {
        Self((0..count).collect())
    }

    /// Wire ends and terminals connect all segments at that point. Interior crossings alone
    /// do not connect. Ports with the same nonempty tunnel label share a group.
    pub fn from_geometry<'a>(
        wires: &[Wire],
        ports: impl ExactSizeIterator<Item = (Point, Option<&'a str>)>,
    ) -> Self {
        let offset = wires.len();
        let mut groups = Self::new(offset + ports.len());
        let index = WireIndex::new(wires);
        let mut positions = BTreeMap::new();
        // Query each connection coordinate once. Repeated endpoints and co-located
        // ports reuse its representative, without materializing pairwise wire edges.
        let mut attach = |point, node, groups: &mut Self| {
            if let Some(previous) = positions.insert(point, node) {
                groups.join(node, previous);
            } else {
                index.visit(point, |wire, _| groups.join(node, wire));
            }
        };
        for (i, wire) in wires.iter().enumerate() {
            for point in [wire.a, wire.b] {
                attach(point, i, &mut groups);
            }
        }
        let mut tunnels = BTreeMap::new();
        for (i, (point, tunnel)) in ports.enumerate() {
            let node = offset + i;
            attach(point, node, &mut groups);
            if let Some(label) = tunnel
                && let Some(previous) = tunnels.insert(label, node)
            {
                groups.join(node, previous);
            }
        }
        groups
    }

    pub fn root(&mut self, mut i: usize) -> usize {
        while self.0[i] != i {
            self.0[i] = self.0[self.0[i]];
            i = self.0[i];
        }
        i
    }
    pub fn join(&mut self, a: usize, b: usize) {
        let a = self.root(a);
        let b = self.root(b);
        self.0[a] = b;
    }
}

#[cfg(test)]
mod tests;
