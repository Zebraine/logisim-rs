//! Bounded prime implicants, essential terms and deterministic greedy covering.
//! The cover is valid and irredundant; it need not be the globally smallest cover.
use super::{
    Cell, Expression, Form, Result,
    expression::{Node, Op},
    work::Work,
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

pub(super) fn minimize(
    values: &[Cell],
    inputs: usize,
    form: Form,
    work: &mut Work<'_>,
) -> Result<Expression> {
    work.step()?;
    if values.iter().all(|v| *v == Cell::DontCare) {
        return Ok(Expression(Node::Constant(false)));
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
        return Ok(Expression(Node::Constant(target == Cell::Zero)));
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
                work.step()?;
                let bit = 1 << bit;
                if cube.mask & bit != 0
                    && cubes.contains(&Cube {
                        mask: cube.mask,
                        value: cube.value ^ bit,
                    })
                {
                    used = true;
                    if merged.len() >= 65_536 {
                        return Err(Work::exhausted());
                    }
                    merged.insert(Cube {
                        mask: cube.mask & !bit,
                        value: cube.value & !bit,
                    });
                }
            }
            if !used {
                for row in &required {
                    work.step()?;
                    if cube.covers(*row) {
                        primes.insert(*cube);
                        break;
                    }
                }
            }
        }
        cubes = merged;
    }
    let mut selected = BTreeSet::new();
    for row in &required {
        let mut first = None;
        let mut multiple = false;
        for cube in &primes {
            work.step()?;
            if cube.covers(*row) {
                if first.is_some() {
                    multiple = true;
                    break;
                }
                first = Some(*cube);
            }
        }
        if !multiple && let Some(cube) = first {
            selected.insert(cube);
        }
    }
    let mut uncovered = BTreeSet::new();
    for row in &required {
        let mut covered = false;
        for cube in &selected {
            work.step()?;
            if cube.covers(*row) {
                covered = true;
                break;
            }
        }
        if !covered {
            uncovered.insert(*row);
        }
    }
    while !uncovered.is_empty() {
        let mut best = None;
        for cube in &primes {
            let mut count = 0;
            for row in &uncovered {
                work.step()?;
                count += usize::from(cube.covers(*row));
            }
            let key = (
                count,
                std::cmp::Reverse(cube.mask.count_ones()),
                std::cmp::Reverse(*cube),
            );
            if best.is_none_or(|(old, _)| key > old) {
                best = Some((key, *cube));
            }
        }
        let best = best.expect("each required row has a prime implicant").1;
        uncovered.retain(|row| !best.covers(*row));
        selected.insert(best);
    }
    for cube in selected.clone().iter().rev() {
        let mut redundant = true;
        for row in &required {
            let mut covered = false;
            for other in &selected {
                work.step()?;
                if other != cube && other.covers(*row) {
                    covered = true;
                    break;
                }
            }
            if !covered {
                redundant = false;
                break;
            }
        }
        if redundant {
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
    Ok(Expression(combine(
        terms,
        if sop { Op::Or } else { Op::And },
        !sop,
    )))
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
