//! Bounded fan-in synthesis. Groups retain operand order and occupy disjoint bands.
use super::{Gate, GateBasis, Layout, Node, Op, Point, Result};

impl Layout {
    pub(super) fn multi_terms(&mut self, op: Op, terms: &[&Node], inverted: bool) -> Result<Point> {
        self.check()?;
        if terms.len() == 1 {
            return if self.options.basis == GateBasis::Nand {
                self.nand_node(terms[0], inverted)
            } else {
                self.node(terms[0])
            };
        }
        // Use the minimum number of levels for this fan-in limit, dividing each
        // level evenly rather than leaving a long chain for excess operands.
        let limit = usize::from(Gate::MAX_INPUTS);
        let mut capacity = 1;
        while capacity * limit < terms.len() {
            capacity *= limit;
        }
        let groups = terms.len().div_ceil(capacity);
        let mut sources = Vec::with_capacity(groups);
        let child_inverted = self.options.basis == GateBasis::Nand && op == Op::Or;
        let mut start = 0;
        for group in 0..groups {
            let size = terms.len() / groups + usize::from(group < terms.len() % groups);
            sources.push(self.multi_terms(op, &terms[start..start + size], child_inverted)?);
            start += size;
        }
        let result = if self.options.basis == GateBasis::Nand {
            // NAND is not associative: combine positive products or negative sums,
            // then restore the polarity requested by the parent expression.
            debug_assert!(matches!(op, Op::And | Op::Or));
            let result = self.gate(Gate::Nand, &sources);
            if inverted == (op == Op::Or) {
                self.invert(result)
            } else {
                result
            }
        } else {
            let gate = match op {
                Op::And => Gate::And,
                Op::Or => Gate::Or,
                Op::Xor => Gate::Xor,
            };
            self.gate(gate, &sources)
        };
        self.check()?;
        Ok(result)
    }
}
