use super::{Error, MAX_EXPRESSION_BYTES, MAX_INPUTS, Result};

const MAX_NODES: usize = 2048;
const MAX_DEPTH: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Op {
    And,
    Xor,
    Or,
}
impl Op {
    fn precedence(self) -> u8 {
        match self {
            Self::Or => 1,
            Self::Xor => 2,
            Self::And => 3,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Node {
    Constant(bool),
    Input(usize),
    Not(Box<Node>),
    Binary(Op, Box<Node>, Box<Node>),
}
impl Node {
    pub(super) fn binary(op: Op, a: Self, b: Self) -> Self {
        Self::Binary(op, Box::new(a), Box::new(b))
    }
    pub(super) fn not(self) -> Self {
        Self::Not(Box::new(self))
    }
    fn depth(&self) -> usize {
        match self {
            Self::Not(a) => 1 + a.depth(),
            Self::Binary(_, a, b) => 1 + a.depth().max(b.depth()),
            _ => 1,
        }
    }
    fn evaluate(&self, row: usize, inputs: usize) -> bool {
        match self {
            Self::Constant(value) => *value,
            Self::Input(i) => *i < inputs && row & (1 << (inputs - i - 1)) != 0,
            Self::Not(a) => !a.evaluate(row, inputs),
            Self::Binary(op, a, b) => {
                let a = a.evaluate(row, inputs);
                let b = b.evaluate(row, inputs);
                match op {
                    Op::And => a & b,
                    Op::Xor => a ^ b,
                    Op::Or => a | b,
                }
            }
        }
    }
    fn uses_input(&self, index: usize) -> bool {
        match self {
            Self::Input(i) => *i == index,
            Self::Not(n) => n.uses_input(index),
            Self::Binary(_, a, b) => a.uses_input(index) || b.uses_input(index),
            Self::Constant(_) => false,
        }
    }
    fn node_count(&self) -> usize {
        match self {
            Self::Not(n) => 1 + n.node_count(),
            Self::Binary(_, a, b) => 1 + a.node_count() + b.node_count(),
            _ => 1,
        }
    }
    pub(super) fn remap_inputs(&mut self, inputs: &[usize]) {
        match self {
            Self::Input(i) => *i = inputs[*i],
            Self::Not(n) => n.remap_inputs(inputs),
            Self::Binary(_, a, b) => {
                a.remap_inputs(inputs);
                b.remap_inputs(inputs);
            }
            Self::Constant(_) => {}
        }
    }
    fn swap_inputs(&mut self, a: usize, b: usize) {
        match self {
            Self::Input(i) if *i == a => *i = b,
            Self::Input(i) if *i == b => *i = a,
            Self::Not(n) => n.swap_inputs(a, b),
            Self::Binary(_, left, right) => {
                left.swap_inputs(a, b);
                right.swap_inputs(a, b);
            }
            _ => {}
        }
    }
    fn format(&self, inputs: &[String], parent: u8, result: &mut String) {
        let precedence = match self {
            Self::Binary(op, _, _) => op.precedence(),
            Self::Not(_) => 4,
            _ => 5,
        };
        if precedence < parent {
            result.push('(');
        }
        match self {
            Self::Constant(value) => result.push(if *value { '1' } else { '0' }),
            Self::Input(i) => result.push_str(inputs.get(*i).map_or("?", String::as_str)),
            Self::Not(a) => {
                result.push('~');
                a.format(inputs, 4, result);
            }
            Self::Binary(op, a, b) => {
                a.format(inputs, precedence, result);
                result.push_str(match op {
                    Op::And => " & ",
                    Op::Xor => " ^ ",
                    Op::Or => " | ",
                });
                b.format(inputs, precedence, result);
            }
        }
        if precedence < parent {
            result.push(')');
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expression(pub(super) Node);
impl Expression {
    pub(super) fn uses_input(&self, index: usize) -> bool {
        self.0.uses_input(index)
    }
    pub(super) fn node_count(&self) -> usize {
        self.0.node_count()
    }
    pub fn parse(text: &str, inputs: &[String]) -> Result<Self> {
        if text.len() > MAX_EXPRESSION_BYTES || inputs.len() > MAX_INPUTS {
            return Err(Error::new("Expression exceeds the size or depth limit."));
        }
        let mut parser = Parser {
            text,
            position: 0,
            nodes: 0,
            inputs,
        };
        let node = parser.binary(1, 0)?;
        parser.whitespace();
        if parser.position != text.len() {
            return Err(parser.error("Unexpected expression token at character:"));
        }
        Ok(Self(node))
    }
    /// Evaluate one binary truth-table row. Other logic states are not synthesized.
    pub fn evaluate(&self, row: usize, inputs: usize) -> bool {
        inputs <= MAX_INPUTS && self.0.evaluate(row, inputs)
    }
    pub fn text(&self, inputs: &[String]) -> String {
        let mut result = String::new();
        self.0.format(inputs, 0, &mut result);
        result
    }
    pub(super) fn swap_inputs(&mut self, a: usize, b: usize) {
        self.0.swap_inputs(a, b);
    }
}

struct Parser<'a> {
    text: &'a str,
    position: usize,
    nodes: usize,
    inputs: &'a [String],
}
impl Parser<'_> {
    fn error(&self, message: &'static str) -> Error {
        Error::detail(message, self.text[..self.position].chars().count() + 1)
    }
    fn peek(&self) -> Option<char> {
        self.text[self.position..].chars().next()
    }
    fn advance(&mut self) {
        if let Some(c) = self.peek() {
            self.position += c.len_utf8();
        }
    }
    fn whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.advance();
        }
    }
    fn node(&mut self, node: Node) -> Result<Node> {
        self.nodes += 1;
        if self.nodes > MAX_NODES || node.depth() > MAX_DEPTH {
            Err(Error::new("Expression exceeds the size or depth limit."))
        } else {
            Ok(node)
        }
    }
    fn binary(&mut self, minimum: u8, depth: usize) -> Result<Node> {
        let mut left = self.unary(depth)?;
        loop {
            self.whitespace();
            let (op, explicit) = match self.peek() {
                Some('|') | Some('+') => (Op::Or, true),
                Some('^') => (Op::Xor, true),
                Some('&') => (Op::And, true),
                Some('(' | '~' | '!' | '0' | '1') => (Op::And, false),
                Some(c) if c.is_alphabetic() => (Op::And, false),
                _ => break,
            };
            if op.precedence() < minimum {
                break;
            }
            if explicit {
                self.advance();
            }
            let right = self.binary(op.precedence() + 1, depth + 1)?;
            left = self.node(Node::binary(op, left, right))?;
        }
        Ok(left)
    }
    fn unary(&mut self, depth: usize) -> Result<Node> {
        if depth >= MAX_DEPTH {
            return Err(Error::new("Expression exceeds the size or depth limit."));
        }
        self.whitespace();
        let mut value = match self.peek() {
            Some('~' | '!') => {
                self.advance();
                let value = self.unary(depth + 1)?;
                self.node(value.not())?
            }
            Some('(') => {
                self.advance();
                let value = self.binary(1, depth + 1)?;
                self.whitespace();
                if self.peek() != Some(')') {
                    return Err(self.error("Expected closing parenthesis at character:"));
                }
                self.advance();
                value
            }
            Some('0' | '1') => {
                let value = self.peek() == Some('1');
                self.advance();
                self.node(Node::Constant(value))?
            }
            Some(c) if c.is_alphabetic() => {
                let start = self.position;
                while self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    self.advance();
                }
                let name = &self.text[start..self.position];
                let index = self
                    .inputs
                    .iter()
                    .position(|input| input == name)
                    .ok_or_else(|| Error::detail("Unknown input variable:", name))?;
                self.node(Node::Input(index))?
            }
            _ => return Err(self.error("Expected an operand at character:")),
        };
        loop {
            self.whitespace();
            if self.peek() != Some('\'') {
                break;
            }
            self.advance();
            value = self.node(value.not())?;
        }
        Ok(value)
    }
}
