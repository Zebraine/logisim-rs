//! Lower each expression once, reusing emitted signals instead of duplicating trees.
//! Every expansion stays inside its operands' vertical band. The same ordered
//! routing as ordinary gates handles fanout and tied-input NAND inverters.
use super::{Gate, Layout, Node, Op, Point, Result};

impl Layout {
    fn nand(&mut self, a: Point, b: Point) -> Point {
        self.binary_gate(Gate::Nand, a, b)
    }
    pub(super) fn invert(&mut self, source: Point) -> Point {
        self.nand(source, source)
    }
    pub(super) fn nand_node(&mut self, node: &Node, inverted: bool) -> Result<Point> {
        self.check()?;
        let point = match node {
            Node::Constant(value) => self.constant(*value ^ inverted),
            Node::Input(index) => {
                let source = self.input(*index);
                if inverted {
                    self.invert(source)
                } else {
                    source
                }
            }
            // Carry the requested polarity into the child rather than adding and
            // then cancelling inversions, especially for NAND-of-products covers.
            Node::Not(child) => self.nand_node(child, !inverted)?,
            Node::Binary(op @ (Op::And | Op::Or), _, _) if !self.options.two_input_only => {
                self.multi_terms(*op, &node.operands(*op), inverted)?
            }
            Node::Binary(Op::And, a, b) => {
                let a = self.nand_node(a, false)?;
                let b = self.nand_node(b, false)?;
                let result = self.nand(a, b);
                if inverted {
                    result
                } else {
                    self.invert(result)
                }
            }
            Node::Binary(Op::Or, a, b) => {
                let a = self.nand_node(a, true)?;
                let b = self.nand_node(b, true)?;
                let result = self.nand(a, b);
                if inverted {
                    self.invert(result)
                } else {
                    result
                }
            }
            Node::Binary(Op::Xor, a, b) => {
                let a = self.nand_node(a, false)?;
                let b = self.nand_node(b, false)?;
                let shared = self.nand(a, b);
                let upper = self.nand(a, shared);
                let lower = self.nand(shared, b);
                let result = self.nand(upper, lower);
                if inverted {
                    self.invert(result)
                } else {
                    result
                }
            }
        };
        self.check()?;
        Ok(point)
    }
}
