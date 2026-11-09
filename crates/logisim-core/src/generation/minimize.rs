//! Bounded prime implicants, essential terms and deterministic greedy covering.
//! The cover is valid and irredundant; it need not be the globally smallest cover.
use super::{
    Cell, Expression, Form,
    expression::{Node, Op},
};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Cube {
    mask: usize,
    value: usize,
}
impl Cube {
    fn covers(self, row: usize) -> bool {
        row & self.mask == self.value
    }
}

pub(super) fn minimize(values: &[Cell], inputs: usize, form: Form) -> Expression {
    if values.iter().all(|v| *v == Cell::DontCare) {
        return Expression(Node::Constant(false));
    }
    let target = if form == Form::SumOfProducts {
        Cell::One
    } else {
        Cell::Zero
    };
    let required: BTreeSet<_> = values
        .iter()
        .enumerate()
        .filter_map(|(i, v)| (*v == target).then_some(i))
        .collect();
    if required.is_empty() {
        return Expression(Node::Constant(target == Cell::Zero));
    }
    let mut cubes: BTreeSet<_> = values
        .iter()
        .enumerate()
        .filter(|(_, v)| **v == target || **v == Cell::DontCare)
        .map(|(value, _)| Cube {
            mask: (1 << inputs) - 1,
            value,
        })
        .collect();
    let mut primes = BTreeSet::new();
    while !cubes.is_empty() {
        let mut merged = BTreeSet::new();
        for cube in &cubes {
            let mut used = false;
            for bit in 0..inputs {
                let bit = 1 << bit;
                if cube.mask & bit != 0
                    && cubes.contains(&Cube {
                        mask: cube.mask,
                        value: cube.value ^ bit,
                    })
                {
                    used = true;
                    merged.insert(Cube {
                        mask: cube.mask & !bit,
                        value: cube.value & !bit,
                    });
                }
            }
            if !used && required.iter().any(|row| cube.covers(*row)) {
                primes.insert(*cube);
            }
        }
        cubes = merged;
    }
    let mut selected = BTreeSet::new();
    for row in &required {
        let mut covering = primes.iter().filter(|cube| cube.covers(*row));
        if let Some(first) = covering.next()
            && covering.next().is_none()
        {
            selected.insert(*first);
        }
    }
    let mut uncovered: BTreeSet<_> = required
        .iter()
        .copied()
        .filter(|row| !selected.iter().any(|cube| cube.covers(*row)))
        .collect();
    while !uncovered.is_empty() {
        // Fixed tie ordering makes the generated document reproducible.
        let best = primes
            .iter()
            .max_by_key(|cube| {
                (
                    uncovered.iter().filter(|row| cube.covers(**row)).count(),
                    std::cmp::Reverse(cube.mask.count_ones()),
                    std::cmp::Reverse(**cube),
                )
            })
            .copied()
            .expect("each required row has a prime implicant");
        uncovered.retain(|row| !best.covers(*row));
        selected.insert(best);
    }
    for cube in selected.clone().iter().rev() {
        if required.iter().all(|row| {
            selected
                .iter()
                .any(|other| other != cube && other.covers(*row))
        }) {
            selected.remove(cube);
        }
    }
    let sop = form == Form::SumOfProducts;
    let terms = selected
        .into_iter()
        .map(|cube| {
            let literals = (0..inputs)
                .filter_map(|input| {
                    let bit = 1 << (inputs - input - 1);
                    if cube.mask & bit == 0 {
                        return None;
                    }
                    let node = Node::Input(input);
                    Some(if (cube.value & bit != 0) == sop {
                        node
                    } else {
                        node.not()
                    })
                })
                .collect();
            combine(literals, if sop { Op::And } else { Op::Or }, sop)
        })
        .collect();
    Expression(combine(terms, if sop { Op::Or } else { Op::And }, !sop))
}

fn combine(mut nodes: Vec<Node>, op: Op, empty: bool) -> Node {
    if nodes.is_empty() {
        return Node::Constant(empty);
    }
    // Balanced trees bound both layout depth and gate propagation depth.
    while nodes.len() > 1 {
        let mut next = Vec::with_capacity(nodes.len().div_ceil(2));
        let mut iter = nodes.into_iter();
        while let Some(a) = iter.next() {
            next.push(if let Some(b) = iter.next() {
                Node::binary(op, a, b)
            } else {
                a
            });
        }
        nodes = next;
    }
    nodes.pop().unwrap()
}
