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

#[cfg(test)]
mod tests;
