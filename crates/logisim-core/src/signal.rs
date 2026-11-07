//! Four-state bus values shared by native component implementations.
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
    pub fn from_bits(bits: impl Iterator<Item = Bit>) -> Self {
        let mut value = Self::filled(0, Bit::Unknown);
        for (i, bit) in bits.take(32).enumerate() {
            value.bits[i] = bit;
            value.width += 1;
        }
        value
    }
}
