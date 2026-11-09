//! Display rules measured from the course probe; this is not an instruction executor.
const REGISTERS: [&str; 32] = [
    "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1", "a0", "a1", "a2", "a3", "a4",
    "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "t3", "t4",
    "t5", "t6",
];
fn signed(value: u32, bits: u32) -> i32 {
    ((value << (32 - bits)) as i32) >> (32 - bits)
}
pub fn decode(word: u32) -> String {
    if word == 0 {
        return "Nop/Invalid Instr".into();
    }
    let rd = REGISTERS[((word >> 7) & 31) as usize];
    let a = REGISTERS[((word >> 15) & 31) as usize];
    let b = REGISTERS[((word >> 20) & 31) as usize];
    let f = ((word >> 12) & 7) as usize;
    let high = word >> 25;
    let imm = signed(word >> 20, 12);
    let small = (word >> 20) & 31;
    match word & 0x7c {
        0 => {
            let name = ["lb", "lh", "lw", "ld", "lbu", "lhu", "lwu", ""][f];
            if name.is_empty() {
                String::new()
            } else {
                format!("{name} {rd},({imm}){a}")
            }
        }
        0x20 if f < 4 => {
            let name = ["sb", "sh", "sw", "sd"][f];
            let offset = signed(((word >> 7) & 31) | ((word >> 25) << 5), 12);
            format!("{name} {b},({offset}){a}")
        }
        0x10 => {
            let name = match f {
                0 => "addi",
                1 => "slli",
                2 => "slti",
                3 => "sltiu",
                4 => "xori",
                5 if high == 0 => "srli",
                5 if high == 32 => "srai",
                6 => "ori",
                7 => "andi",
                _ => return String::new(),
            };
            let value = if matches!(f, 1 | 4 | 6 | 7) {
                small as i32
            } else {
                imm
            };
            format!("{name} {rd},{a},{value}")
        }
        0x30 => {
            let name = match high {
                0 => ["add", "sll", "slt", "sltu", "xor", "srl", "or", "and"][f],
                1 => [
                    "mul", "mulh", "mulhsu", "mulhu", "div", "divu", "rem", "remu",
                ][f],
                32 if f == 0 => "sub",
                32 if f == 5 => "sra",
                _ => return String::new(),
            };
            format!("{name} {rd},{a},{b}")
        }
        0x14 | 0x34 => {
            let name = if word & 0x7c == 0x14 { "auipc" } else { "lui" };
            format!("{name} {rd},{}", (word as i32) >> 12)
        }
        0x60 => {
            let name = ["beq", "bne", "", "", "blt", "bge", "bltu", "bgeu"][f];
            if name.is_empty() {
                return String::new();
            }
            let offset = ((word >> 7) & 1) << 11
                | ((word >> 8) & 15) << 1
                | ((word >> 25) & 63) << 5
                | (word >> 31) << 12;
            format!("{name} {a},{b},{}", signed(offset, 13))
        }
        0x64 => format!("jalr {rd},{a},{imm}"),
        0x6c => {
            let offset = word & 0xff000
                | ((word >> 20) & 1) << 11
                | ((word >> 21) & 1023) << 1
                | (word >> 31) << 20;
            format!("jal {rd},{}", signed(offset, 21))
        }
        0x0c => match f {
            0 => "fence",
            1 => "fence.I",
            _ => "",
        }
        .into(),
        0x70 => {
            if f == 0 {
                return match word >> 20 {
                    0 => "ecall",
                    1 => "break",
                    0x102 => "sret",
                    0x302 => "mret",
                    0x105 => "wfi",
                    _ => "",
                }
                .into();
            }
            let name = [
                "", "csrrw", "csrrs", "csrrc", "", "csrrwi", "csrrsi", "csrrci",
            ][f];
            if name.is_empty() {
                return String::new();
            }
            let source = if f < 4 {
                a.into()
            } else {
                ((word >> 15) & 31).to_string()
            };
            format!("{name} {rd},{},{source}", word >> 20)
        }
        _ => String::new(),
    }
}
#[cfg(test)]
mod tests;
