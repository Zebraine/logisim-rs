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
    // Exact equality (including don't-cares) preserves the complete specification.
    let mut active = Vec::new();
    for input in 0..inputs {
        let bit = 1 << (inputs - input - 1);
        let mut dependent = false;
        for row in 0..values.len() {
            work.step()?;
            if row & bit == 0 && values[row] != values[row | bit] {
                dependent = true;
                break;
            }
        }
        if dependent {
            active.push(input);
        }
    }
    if active.len() == inputs {
        return minimize_reduced(values, inputs, form, work);
    }
    let reduced: Vec<_> = (0..1usize << active.len())
        .map(|row| {
            let original = active.iter().enumerate().fold(0, |value, (i, input)| {
                value | ((row >> (active.len() - i - 1) & 1) << (inputs - input - 1))
            });
            values[original]
        })
        .collect();
    let mut result = minimize_reduced(&reduced, active.len(), form, work)?;
    result.0.remap_inputs(&active);
    Ok(result)
}

fn minimize_reduced(
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
    if values.iter().all(|v| *v == target || *v == Cell::DontCare) {
        return Ok(Expression(Node::Constant(target == Cell::One)));
    }
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
    // Count coverage once. Only rows covered by this cube can lose their cover.
    let mut counts = vec![0usize; values.len()];
    for cube in &selected {
        for row in &required {
            work.step()?;
            counts[*row] += usize::from(cube.covers(*row));
        }
    }
    for cube in selected.clone().iter().rev() {
        let mut redundant = true;
        for row in &required {
            work.step()?;
            if cube.covers(*row) && counts[*row] == 1 {
                redundant = false;
                break;
            }
        }
        if redundant {
            selected.remove(cube);
            for row in &required {
                work.step()?;
                if cube.covers(*row) {
                    counts[*row] -= 1;
                }
            }
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
            Node::combine(literals, if sop { Op::And } else { Op::Or }, sop)
        })
        .collect();
    Ok(Expression(Node::combine(
        terms,
        if sop { Op::Or } else { Op::And },
        !sop,
    )))
}
