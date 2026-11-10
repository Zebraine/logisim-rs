use super::isa::*;
use crate::builder::{Builder, E};
use logisim_core::Circuit;
pub(super) fn build() -> Circuit {
    let mut b = Builder::new("G16 ALU / wrapping arithmetic / shifts use B[3:0]");
    let op = b.input("OP", 5);
    let a = b.input("A", 16);
    let rhs = b.input("B", 16);
    let imm = b.input("IMM", 16);
    b.section("Operations / result selection");
    let amount = rhs.slice(0, 4);
    let operations = [
        (LI, imm.clone()),
        (ADD, a.add(&rhs)),
        (SUB, a.sub(&rhs)),
        (AND, a.and(&rhs)),
        (OR, a.or(&rhs)),
        (XOR, a.xor(&rhs)),
        (SHL, a.shift(&amount, 0)),
        (SHR, a.shift(&amount, 1)),
        (SAR, a.shift(&amount, 2)),
        (SLTU, a.less(&rhs, false).widen(16)),
        (SLT, a.less(&rhs, true).widen(16)),
        (ADDI, a.add(&imm)),
        (ANDI, a.and(&imm)),
        (ORI, a.or(&imm)),
        (XORI, a.xor(&imm)),
    ];
    let value = operations
        .into_iter()
        .rev()
        .fold(E::num(16, 0), |other, (code, value)| {
            op.is(code).choose(&value, &other)
        });
    b.output("RESULT", &value);
    b.finish()
}
