use logisim_core::Gate;

/// Per-bit states; unknown means that the net has no defined driver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bit {
    Zero,
    One,
    Unknown,
    Error,
}

/// A bounded bus, least significant bit first. Unused storage is always unknown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Value {
    width: u8,
    bits: [Bit; 32],
}
impl Value {
    pub fn filled(width: u8, bit: Bit) -> Self {
        let width = width.min(32);
        let mut bits = [Bit::Unknown; 32];
        bits[..usize::from(width)].fill(bit);
        Self { width, bits }
    }
    pub fn known(width: u8, number: u32) -> Self {
        let mut value = Self::filled(width, Bit::Zero);
        for i in 0..value.width {
            if number & (1 << i) != 0 {
                value.bits[usize::from(i)] = Bit::One;
            }
        }
        value
    }
    pub fn width(self) -> u8 {
        self.width
    }
    pub fn bit(self, index: u8) -> Bit {
        if index < self.width {
            self.bits[usize::from(index)]
        } else {
            Bit::Unknown
        }
    }
    pub fn contains(self, bit: Bit) -> bool {
        self.bits[..usize::from(self.width)].contains(&bit)
    }
    pub fn number(self) -> Option<u32> {
        let mut number = 0;
        for i in 0..self.width {
            match self.bit(i) {
                Bit::One => number |= 1 << i,
                Bit::Zero => {}
                _ => return None,
            }
        }
        Some(number)
    }
    pub fn binary(self) -> String {
        (0..self.width)
            .rev()
            .map(|i| match self.bit(i) {
                Bit::Zero => '0',
                Bit::One => '1',
                Bit::Unknown => 'X',
                Bit::Error => 'E',
            })
            .collect()
    }
    pub(crate) fn from_bits(bits: impl Iterator<Item = Bit>) -> Self {
        let mut value = Self::filled(0, Bit::Unknown);
        for (i, bit) in bits.take(32).enumerate() {
            value.bits[i] = bit;
            value.width += 1;
        }
        value
    }
}

/// Inputs omitted by the caller are physically unconnected, not floating wires.
pub(crate) fn evaluate(gate: Gate, width: u8, inputs: &[Value]) -> Value {
    let mut result = Value::filled(width, Bit::Error);
    for i in 0..result.width {
        let bits: Vec<_> = inputs.iter().map(|v| v.bit(i)).collect();
        let base = match gate {
            Gate::Buffer => bits.first().copied().unwrap_or(Bit::Unknown),
            Gate::Not => match bits.first() {
                Some(Bit::Zero) => Bit::One,
                Some(Bit::One) => Bit::Zero,
                _ => Bit::Error,
            },
            Gate::And | Gate::Nand if bits.contains(&Bit::Zero) => Bit::Zero,
            Gate::Or | Gate::Nor if bits.contains(&Bit::One) => Bit::One,
            _ if bits.is_empty() || bits.iter().any(|b| matches!(b, Bit::Unknown | Bit::Error)) => {
                Bit::Error
            }
            Gate::And | Gate::Nand => Bit::One,
            Gate::Or | Gate::Nor => Bit::Zero,
            Gate::Xor | Gate::Xnor => {
                if bits.iter().filter(|b| **b == Bit::One).count() == 1 {
                    Bit::One
                } else {
                    Bit::Zero
                }
            }
        };
        result.bits[usize::from(i)] = if matches!(gate, Gate::Nand | Gate::Nor | Gate::Xnor) {
            match base {
                Bit::Zero => Bit::One,
                Bit::One => Bit::Zero,
                other => other,
            }
        } else {
            base
        };
    }
    result
}
