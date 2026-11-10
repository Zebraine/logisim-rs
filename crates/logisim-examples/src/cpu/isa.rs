//! G16 v1: 16-bit words, eight registers, fixed 32-bit instructions.
//! OP[31:27], D[26:24], A[23:21], B[20:18], reserved[17:16], immediate[15:0].
//! Unused fields must be zero. Addresses count words; arithmetic wraps at 16 bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    None,
    Ri,
    Rri,
    Rrr,
    Ai,
    I,
    A,
}
#[derive(Clone, Copy)]
pub struct Instruction {
    pub name: &'static str,
    pub opcode: u32,
    pub format: Format,
}
impl Instruction {
    pub fn mask(self) -> u32 {
        let fields = match self.format {
            Format::None => 0,
            Format::Ri => 0x0700_ffff,
            Format::Rri => 0x07e0_ffff,
            Format::Rrr => 0x07fc_0000,
            Format::Ai => 0x00e0_ffff,
            Format::I => 0x0000_ffff,
            Format::A => 0x00e0_0000,
        };
        !fields
    }
}
pub const NOP: u32 = 0;
pub const LI: u32 = 1;
pub const ADD: u32 = 2;
pub const SUB: u32 = 3;
pub const AND: u32 = 4;
pub const OR: u32 = 5;
pub const XOR: u32 = 6;
pub const SHL: u32 = 7;
pub const SHR: u32 = 8;
pub const SAR: u32 = 9;
pub const SLTU: u32 = 10;
pub const SLT: u32 = 11;
pub const ADDI: u32 = 12;
pub const ANDI: u32 = 13;
pub const ORI: u32 = 14;
pub const XORI: u32 = 15;
pub const LD: u32 = 16;
pub const ST: u32 = 17;
pub const BZ: u32 = 18;
pub const BNZ: u32 = 19;
pub const JMP: u32 = 20;
pub const JAL: u32 = 21;
pub const JR: u32 = 22;
pub const HALT: u32 = 31;
macro_rules! instructions {
    ($($name:ident : $format:ident),* $(,)?) => {
        pub const ALL: &[Instruction] = &[$(Instruction {
            name: stringify!($name),
            opcode: $name,
            format: Format::$format,
        }),*];
    };
}
instructions! {NOP:None, LI:Ri, ADD:Rrr, SUB:Rrr, AND:Rrr, OR:Rrr, XOR:Rrr,
SHL:Rrr, SHR:Rrr, SAR:Rrr, SLTU:Rrr, SLT:Rrr, ADDI:Rri, ANDI:Rri, ORI:Rri,
XORI:Rri, LD:Rri, ST:Rri, BZ:Ai, BNZ:Ai, JMP:I, JAL:Ri, JR:A, HALT:None}
