//! Deterministic inputs and output fingerprints from the running reference.
//! Compiled only for tests and the host-side Wasm checker.
use crate::model::{Chip, INPUTS, OUTPUTS};
use logisim_core::{Bit, Value};
use logisim_plugin_api::Signal;
const SERIAL: usize = 8;
const LOAD: usize = 9;
const CLOCK: usize = 10;
const INHIBIT: usize = 11;
const VCC: usize = 12;
const GND: usize = 13;
pub const HASH_START: u64 = 0xcbf29ce484222325;
pub struct Scenario {
    pub name: &'static str,
    pub rows: Vec<String>,
    pub expected: u64,
}
struct Sequence {
    current: [char; 14],
    rows: Vec<String>,
}
impl Sequence {
    fn new() -> Self {
        let mut s = Self {
            current: ['0'; 14],
            rows: vec![],
        };
        s.current[VCC] = '1';
        s.push();
        s
    }
    fn push(&mut self) {
        self.rows.push(self.current.iter().collect());
    }
    fn set(&mut self, i: usize, bit: char) {
        self.current[i] = bit;
        self.push();
    }
    fn data(&mut self, value: [char; 8]) {
        self.current[..8].copy_from_slice(&value);
        self.push();
    }
    fn number(&mut self, value: u8) {
        self.data(std::array::from_fn(|i| {
            if value & (1 << i) != 0 { '1' } else { '0' }
        }));
    }
    fn reset(&mut self, value: u8) {
        for (i, b) in [
            (CLOCK, '0'),
            (VCC, '1'),
            (GND, '0'),
            (INHIBIT, '0'),
            (LOAD, '0'),
        ] {
            self.set(i, b);
        }
        self.number(value);
        for (i, b) in [(SERIAL, '0'), (CLOCK, '1'), (CLOCK, '0')] {
            self.set(i, b);
        }
    }
    fn finish(self, name: &'static str, expected: u64) -> Scenario {
        Scenario {
            name,
            rows: self.rows,
            expected,
        }
    }
}
pub fn scenarios() -> Vec<Scenario> {
    let mut all = vec![];
    let mut s = Sequence::new();
    for initial in [0, 255] {
        for load in "01XE".chars() {
            for inhibit in "01XE".chars() {
                s.reset(initial);
                s.number(initial);
                s.set(SERIAL, if initial == 0 { '1' } else { '0' });
                s.set(LOAD, load);
                s.set(INHIBIT, inhibit);
                s.set(CLOCK, '1');
            }
        }
    }
    all.push(s.finish("controls", 0x8c51c950131c84a4));
    let mut s = Sequence::new();
    for a in "01XE".chars() {
        for b in "01XE".chars() {
            s.reset(0);
            s.set(LOAD, '1');
            s.set(SERIAL, '0');
            s.set(CLOCK, a);
            s.set(SERIAL, '1');
            s.set(CLOCK, b);
        }
    }
    all.push(s.finish("edges", 0x7938629b7703657a));
    let mut s = Sequence::new();
    for pin in 0..8 {
        for value in "01XE".chars() {
            s.reset(0);
            let mut data = ['0'; 8];
            data[pin] = value;
            s.data(data);
            s.set(CLOCK, '1');
            s.set(CLOCK, '0');
            s.set(LOAD, '1');
            s.set(SERIAL, '0');
            for _ in 0..8 {
                s.set(CLOCK, '1');
                s.set(CLOCK, '0');
            }
        }
    }
    for value in "01XE".chars() {
        s.reset(0);
        s.set(LOAD, '1');
        s.set(SERIAL, value);
        s.set(CLOCK, '1');
    }
    all.push(s.finish("data", 0x5e3671128e40f8e7));
    let mut s = Sequence::new();
    for a in "01XE".chars() {
        for b in "01XE".chars() {
            s.reset(255);
            s.set(VCC, a);
            s.set(GND, b);
            s.number(0);
            s.set(CLOCK, '1');
            s.set(VCC, '1');
            s.set(GND, '0');
            s.set(CLOCK, '0');
            s.set(CLOCK, '1');
        }
    }
    all.push(s.finish("power", 0xa44760cea58ccbfd));
    let mut s = Sequence::new();
    for a in "01XE".chars() {
        for b in "01XE".chars() {
            s.reset(0);
            s.set(LOAD, '1');
            s.set(SERIAL, '0');
            s.set(INHIBIT, a);
            s.set(CLOCK, '1');
            s.set(SERIAL, '1');
            s.set(INHIBIT, b);
            s.set(CLOCK, '0');
            s.set(CLOCK, '1');
        }
    }
    all.push(s.finish("inhibit", 0x73811389b20f44e4));
    all
}
pub fn decode(c: char) -> Bit {
    match c {
        '0' => Bit::Zero,
        '1' => Bit::One,
        'X' => Bit::Unknown,
        'E' => Bit::Error,
        _ => panic!("invalid observed bit"),
    }
}
pub fn inputs(chip: Chip, row: &str) -> Vec<Signal> {
    assert_eq!(row.len(), 14);
    let mut values = vec![Value::known(1, 0).into(); chip.pins().len()];
    for (pin, bit) in INPUTS
        .iter()
        .take(if chip.power { 14 } else { 12 })
        .zip(row.chars())
    {
        values[chip.index(*pin)] = Value::filled(1, decode(bit)).into();
    }
    values
}
pub fn fingerprint(hash: &mut u64, chip: Chip, outputs: &[Signal]) {
    for pin in OUTPUTS {
        let b = match Value::from(outputs[chip.index(pin)]).bit(0) {
            Bit::Zero => b'0',
            Bit::One => b'1',
            Bit::Unknown => b'X',
            Bit::Error => b'E',
        };
        *hash = (*hash ^ u64::from(b)).wrapping_mul(0x100000001b3);
    }
}
