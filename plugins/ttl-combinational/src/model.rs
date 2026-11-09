//! Pin assignments from runtime inspection of the reference application's internal view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Decoder7447,
    Comparator7485,
    Buffer74125,
    Adder74283,
}
impl Kind {
    pub const ALL: [Self; 4] = [
        Self::Decoder7447,
        Self::Comparator7485,
        Self::Buffer74125,
        Self::Adder74283,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Decoder7447 => "7447",
            Self::Comparator7485 => "7485",
            Self::Buffer74125 => "74125",
            Self::Adder74283 => "74283",
        }
    }
    pub fn count(self) -> u8 {
        if self == Self::Buffer74125 { 14 } else { 16 }
    }
    /// Function order, least significant bit first for operands and arithmetic outputs.
    pub fn inputs(self) -> &'static [u8] {
        match self {
            Self::Decoder7447 => &[7, 1, 2, 6, 3, 4, 5],
            Self::Comparator7485 => &[10, 12, 13, 15, 9, 11, 14, 1, 2, 3, 4],
            Self::Buffer74125 => &[1, 2, 4, 5, 10, 9, 13, 12],
            Self::Adder74283 => &[5, 3, 14, 12, 6, 2, 15, 11, 7],
        }
    }
    pub fn outputs(self) -> &'static [u8] {
        match self {
            Self::Decoder7447 => &[13, 12, 11, 10, 9, 15, 14],
            Self::Comparator7485 => &[5, 6, 7],
            Self::Buffer74125 => &[3, 6, 8, 11],
            Self::Adder74283 => &[4, 1, 13, 10, 9],
        }
    }
    pub fn labels(self) -> &'static [&'static str] {
        match self {
            Self::Decoder7447 => &[
                "B", "C", "LT", "BI", "RBI", "D", "A", "GND", "e", "d", "c", "b", "a", "g", "f",
                "Vcc",
            ],
            Self::Comparator7485 => &[
                "B3", "I<", "I=", "I>", "O>", "O=", "O<", "GND", "B0", "A0", "B1", "A1", "A2",
                "B2", "A3", "Vcc",
            ],
            Self::Buffer74125 => &[
                "/OE1", "A1", "Y1", "/OE2", "A2", "Y2", "GND", "Y3", "A3", "/OE3", "Y4", "A4",
                "/OE4", "Vcc",
            ],
            Self::Adder74283 => &[
                "S2", "B2", "A2", "S1", "A1", "B1", "Cin", "GND", "C4", "S4", "B4", "A4", "S3",
                "A3", "B3", "Vcc",
            ],
        }
    }
    pub fn position(self, pin: u8) -> [i32; 2] {
        let n = self.count();
        assert!((1..=n).contains(&pin));
        if pin <= n / 2 {
            [10 + 20 * i32::from(pin - 1), 30]
        } else {
            [10 + 20 * i32::from(n - pin), -30]
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Chip {
    pub kind: Kind,
    pub power: bool,
}
impl Chip {
    pub fn id(self) -> String {
        format!(
            "{}{}",
            self.kind.name(),
            if self.power { "-powered" } else { "" }
        )
    }
    pub fn parse(id: &str) -> Option<Self> {
        let power = id.ends_with("-powered");
        let name = id.strip_suffix("-powered").unwrap_or(id);
        Kind::ALL
            .into_iter()
            .find(|k| k.name() == name)
            .map(|kind| Self { kind, power })
    }
    pub fn pins(self) -> Vec<u8> {
        let n = self.kind.count();
        let mut pins: Vec<_> = (1..n).filter(|p| *p != n / 2).collect();
        if self.power {
            pins.extend([n / 2, n]);
        }
        pins
    }
    pub fn index(self, pin: u8) -> usize {
        let n = self.kind.count();
        assert!((1..=n).contains(&pin));
        if pin == n / 2 {
            assert!(self.power);
            usize::from(n - 2)
        } else if pin == n {
            assert!(self.power);
            usize::from(n - 1)
        } else {
            usize::from(pin - 1 - u8::from(pin > n / 2))
        }
    }
}
