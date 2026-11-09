//! Physical pins observed in the running reference's internal view.
pub const DATA: [u8; 8] = [11, 12, 13, 14, 3, 4, 5, 6];
pub const INPUTS: [u8; 14] = [11, 12, 13, 14, 3, 4, 5, 6, 10, 1, 2, 15, 16, 8];
pub const OUTPUTS: [u8; 2] = [9, 7];
pub const LABELS: [&str; 16] = [
    "ShLd", "CK", "P4", "P5", "P6", "P7", "Q7n", "GND", "Q7", "SER", "P0", "P1", "P2", "P3",
    "ClkInh", "Vcc",
];
pub fn position(pin: u8) -> [i32; 2] {
    assert!((1..=16).contains(&pin));
    if pin <= 8 {
        [10 + 20 * i32::from(pin - 1), 30]
    } else {
        [10 + 20 * i32::from(16 - pin), -30]
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Chip {
    pub power: bool,
}
impl Chip {
    pub fn id(self) -> &'static str {
        if self.power { "74165-powered" } else { "74165" }
    }
    pub fn parse(id: &str) -> Option<Self> {
        match id {
            "74165" => Some(Self { power: false }),
            "74165-powered" => Some(Self { power: true }),
            _ => None,
        }
    }
    pub fn pins(self) -> Vec<u8> {
        let mut pins: Vec<_> = (1..16).filter(|p| *p != 8).collect();
        if self.power {
            pins.extend([8, 16]);
        }
        pins
    }
    pub fn index(self, pin: u8) -> usize {
        assert!((1..=16).contains(&pin));
        match pin {
            8 | 16 => {
                assert!(self.power);
                if pin == 8 { 14 } else { 15 }
            }
            _ => usize::from(pin - 1 - u8::from(pin > 8)),
        }
    }
}
