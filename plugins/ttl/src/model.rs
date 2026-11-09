//! DIP geometry and pin assignments observed in the reference application.
use logisim_core::Point;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ttl {
    Ic7400,
    Ic7402,
    Ic7404,
    Ic7408,
    Ic7432,
    Ic7486,
    Ic747266,
}
impl Ttl {
    pub const ALL: [Self; 7] = [
        Self::Ic7400,
        Self::Ic7402,
        Self::Ic7404,
        Self::Ic7408,
        Self::Ic7432,
        Self::Ic7486,
        Self::Ic747266,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Ic7400 => "7400",
            Self::Ic7402 => "7402",
            Self::Ic7404 => "7404",
            Self::Ic7408 => "7408",
            Self::Ic7432 => "7432",
            Self::Ic7486 => "7486",
            Self::Ic747266 => "747266",
        }
    }
    /// Physical pin numbers [input A, input B, output]. Inverters repeat their input.
    pub fn gates(self) -> &'static [[u8; 3]] {
        match self {
            Self::Ic7402 => &[[2, 3, 1], [5, 6, 4], [9, 8, 10], [12, 11, 13]],
            Self::Ic7404 => &[
                [1, 1, 2],
                [3, 3, 4],
                [5, 5, 6],
                [9, 9, 8],
                [11, 11, 10],
                [13, 13, 12],
            ],
            _ => &[[1, 2, 3], [4, 5, 6], [10, 9, 8], [13, 12, 11]],
        }
    }
    pub fn output(self, pin: u8) -> bool {
        self.gates().iter().any(|g| g[2] == pin)
    }
    pub fn pin_position(pin: u8) -> Point {
        assert!((1..=14).contains(&pin));
        if pin <= 7 {
            Point::new(10 + 20 * i32::from(pin - 1), 30)
        } else {
            Point::new(10 + 20 * i32::from(14 - pin), -30)
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Chip {
    pub kind: Ttl,
    pub power: bool,
}
impl Chip {
    pub fn id(self) -> String {
        if self.power {
            format!("{}-powered", self.kind.name())
        } else {
            self.kind.name().into()
        }
    }
    pub fn parse(id: &str) -> Option<Self> {
        let power = id.ends_with("-powered");
        let name = id.strip_suffix("-powered").unwrap_or(id);
        Ttl::ALL
            .into_iter()
            .find(|k| k.name() == name)
            .map(|kind| Self { kind, power })
    }
    pub fn pins(self) -> &'static [u8] {
        if self.power {
            &[1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13, 7, 14]
        } else {
            &[1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13]
        }
    }
    pub fn index(self, pin: u8) -> usize {
        self.pins()
            .iter()
            .position(|p| *p == pin)
            .expect("chip pin")
    }
}
