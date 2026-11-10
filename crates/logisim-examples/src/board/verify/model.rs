//! Independent instruction interpreter used only by the circuit checker.
//! It deliberately does not use the circuit's operation table or expression builder.
#[derive(Default)]
pub(super) struct Model {
    pub regs: [u16; 8],
    pub pc: u16,
    pub ram: std::collections::BTreeMap<u16, u16>,
    pub debug: u16,
    pub halted: bool,
    pub fault: bool,
}
impl Model {
    pub fn step(&mut self, word: u32) {
        if self.halted || self.fault {
            return;
        }
        let op = word >> 27;
        let d = ((word >> 24) & 7) as usize;
        let a = self.regs[((word >> 21) & 7) as usize];
        let b = self.regs[((word >> 18) & 7) as usize];
        let imm = word as u16;
        let operands = match op {
            0 | 31 => 0,
            1 | 21 => 0x0700ffff,
            2..=11 => 0x07fc0000,
            12..=17 => 0x07e0ffff,
            18 | 19 => 0x00e0ffff,
            20 => 0x0000ffff,
            22 => 0x00e00000,
            _ => {
                self.fault = true;
                return;
            }
        };
        if word & !operands != op << 27 {
            self.fault = true;
            return;
        }
        let mut pc = self.pc.wrapping_add(1);
        let mut result = None;
        match op {
            0 => {}
            1 => result = Some(imm),
            2 => result = Some(a.wrapping_add(b)),
            3 => result = Some(a.wrapping_sub(b)),
            4 => result = Some(a & b),
            5 => result = Some(a | b),
            6 => result = Some(a ^ b),
            7 => result = Some(a.wrapping_shl((b & 15).into())),
            8 => result = Some(a >> (b & 15)),
            9 => result = Some(((a as i16) >> (b & 15)) as u16),
            10 => result = Some(u16::from(a < b)),
            11 => result = Some(u16::from((a as i16) < (b as i16))),
            12 => result = Some(a.wrapping_add(imm)),
            13 => result = Some(a & imm),
            14 => result = Some(a | imm),
            15 => result = Some(a ^ imm),
            16 => {
                let address = a.wrapping_add(imm);
                result = Some(if address < 4096 {
                    *self.ram.get(&address).unwrap_or(&0)
                } else if address == 0xff30 {
                    self.debug
                } else {
                    0
                });
            }
            17 => {
                let address = a.wrapping_add(imm);
                if address < 4096 {
                    self.ram.insert(address, self.regs[d]);
                } else if address == 0xff30 {
                    self.debug = self.regs[d];
                }
            }
            18 if a == 0 => pc = imm,
            19 if a != 0 => pc = imm,
            18 | 19 => {}
            20 => pc = imm,
            21 => {
                result = Some(pc);
                pc = imm;
            }
            22 => pc = a,
            31 => {
                self.halted = true;
                return;
            }
            _ => unreachable!(),
        }
        if let Some(value) = result
            && d != 0
        {
            self.regs[d] = value;
        }
        self.pc = pc;
    }
}
