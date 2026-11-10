//! A small authoring DSL, lowered entirely to editable built-in components.
use std::sync::Arc;
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct E {
    pub bits: u8,
    pub op: Arc<Op>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    Net(String),
    Constant(u32),
    Not(E),
    And(E, E),
    Or(E, E),
    Xor(E, E),
    Add(E, E),
    Sub(E, E),
    Eq(E, E),
    Mux(E, E, E),
    Slice(E, u8),
    Join(E, E),
}
impl E {
    fn new(bits: u8, op: Op) -> Self {
        assert!((1..=32).contains(&bits));
        Self {
            bits,
            op: Arc::new(op),
        }
    }
    pub fn net(name: &str, bits: u8) -> Self {
        Self::new(bits, Op::Net(name.into()))
    }
    pub fn num(bits: u8, value: u32) -> Self {
        assert!(bits == 32 || value < 1 << bits);
        Self::new(bits, Op::Constant(value))
    }
    fn same(&self, b: &Self) {
        assert_eq!(self.bits, b.bits);
    }
    pub fn not(&self) -> Self {
        Self::new(self.bits, Op::Not(self.clone()))
    }
    pub fn and(&self, b: &Self) -> Self {
        self.same(b);
        Self::new(self.bits, Op::And(self.clone(), b.clone()))
    }
    pub fn or(&self, b: &Self) -> Self {
        self.same(b);
        Self::new(self.bits, Op::Or(self.clone(), b.clone()))
    }
    pub fn xor(&self, b: &Self) -> Self {
        self.same(b);
        Self::new(self.bits, Op::Xor(self.clone(), b.clone()))
    }
    pub fn add(&self, b: &Self) -> Self {
        self.same(b);
        Self::new(self.bits, Op::Add(self.clone(), b.clone()))
    }
    pub fn sub(&self, b: &Self) -> Self {
        self.same(b);
        Self::new(self.bits, Op::Sub(self.clone(), b.clone()))
    }
    pub fn eq(&self, b: &Self) -> Self {
        self.same(b);
        Self::new(1, Op::Eq(self.clone(), b.clone()))
    }
    pub fn is(&self, n: u32) -> Self {
        self.eq(&Self::num(self.bits, n))
    }
    /// Condition selects yes when one, no when zero.
    pub fn choose(&self, yes: &Self, no: &Self) -> Self {
        assert_eq!(self.bits, 1);
        yes.same(no);
        Self::new(yes.bits, Op::Mux(self.clone(), yes.clone(), no.clone()))
    }
    pub fn slice(&self, start: u8, bits: u8) -> Self {
        assert!(start + bits <= self.bits);
        Self::new(bits, Op::Slice(self.clone(), start))
    }
    pub fn join(high: &Self, low: &Self) -> Self {
        Self::new(high.bits + low.bits, Op::Join(high.clone(), low.clone()))
    }
    pub fn widen(&self, bits: u8) -> Self {
        assert!(bits >= self.bits);
        if bits == self.bits {
            self.clone()
        } else {
            Self::join(&Self::num(bits - self.bits, 0), self)
        }
    }
}
pub fn any(xs: &[E]) -> E {
    xs.iter()
        .cloned()
        .reduce(|a, b| a.or(&b))
        .unwrap_or(E::num(1, 0))
}
