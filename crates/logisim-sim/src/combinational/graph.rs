//! Iterative SCC detection followed by acyclic region partitioning.
use std::collections::BTreeSet;

/// Returns feedback members and weakly connected regions in topological order.
/// Vertices are ordered by stable component IDs, not insertion/geometry order.
pub(super) fn partition(edges: &[Vec<usize>]) -> (Vec<usize>, Vec<Vec<usize>>) {
    let mut reverse = vec![vec![]; edges.len()];
    for (from, targets) in edges.iter().enumerate() {
        for &to in targets {
            reverse[to].push(from);
        }
    }
    let mut seen = vec![false; edges.len()];
    let mut finished = vec![];
    for seed in 0..edges.len() {
        if std::mem::replace(&mut seen[seed], true) {
            continue;
        }
        let mut stack = vec![(seed, 0)];
        while let Some((node, next)) = stack.last_mut() {
            if let Some(&target) = edges[*node].get(*next) {
                *next += 1;
                if !std::mem::replace(&mut seen[target], true) {
                    stack.push((target, 0));
                }
            } else {
                finished.push(*node);
                stack.pop();
            }
        }
    }
    seen.fill(false);
    let mut cyclic = vec![false; edges.len()];
    for seed in finished.into_iter().rev() {
        if std::mem::replace(&mut seen[seed], true) {
            continue;
        }
        let mut stack = vec![seed];
        let mut members = vec![];
        while let Some(node) = stack.pop() {
            members.push(node);
            for &source in &reverse[node] {
                if !std::mem::replace(&mut seen[source], true) {
                    stack.push(source);
                }
            }
        }
        if members.len() > 1 || edges[seed].contains(&seed) {
            for node in members {
                cyclic[node] = true;
            }
        }
    }
    let mut membership = vec![usize::MAX; edges.len()];
    let mut regions: Vec<Vec<usize>> = vec![];
    for seed in 0..edges.len() {
        if cyclic[seed] || membership[seed] != usize::MAX {
            continue;
        }
        let group = regions.len();
        regions.push(vec![]);
        membership[seed] = group;
        let mut stack = vec![seed];
        while let Some(node) = stack.pop() {
            for &next in edges[node].iter().chain(&reverse[node]) {
                if !cyclic[next] && membership[next] == usize::MAX {
                    membership[next] = group;
                    stack.push(next);
                }
            }
        }
    }
    let mut incoming: Vec<_> = reverse
        .iter()
        .map(|sources| sources.iter().filter(|i| !cyclic[**i]).count())
        .collect();
    let mut ready: BTreeSet<_> = (0..edges.len())
        .filter(|i| !cyclic[*i] && incoming[*i] == 0)
        .collect();
    while let Some(node) = ready.pop_first() {
        regions[membership[node]].push(node);
        for &next in &edges[node] {
            if !cyclic[next] {
                incoming[next] -= 1;
                if incoming[next] == 0 {
                    ready.insert(next);
                }
            }
        }
    }
    let feedback = (0..edges.len()).filter(|i| cyclic[*i]).collect();
    (feedback, regions)
}
