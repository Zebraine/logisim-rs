//! Original programmable G16 CPU, assembled solely from ordinary circuit components.
mod alu;
pub mod isa;
mod registers;
use crate::builder::{Builder, E, any, hierarchy};
use isa::*;
use logisim_core::*;

pub fn define(project: &mut Project) -> Result<Id, String> {
    let regs = hierarchy::define(project, "G16 registers", registers::build())?;
    let alu = hierarchy::define(project, "G16 ALU", alu::build())?;
    let mut b = Builder::new("G16 v1 / fetch and execute / 16-bit data, 32-bit instructions");
    let clk = b.input("CLK", 1);
    let reset = b.input("RESET", 1);
    let instruction = b.input("INST", 32);
    let data = b.input("RDATA", 16);
    let pc = E::net("pc", 16);
    let ir = E::net("ir", 32);
    let phase = E::net("phase", 1);
    let halted = E::net("halted", 1);
    let fault = E::net("fault", 1);
    let run = halted.or(&fault).or(&reset).not();
    let execute = run.and(&phase);
    let op = ir.slice(27, 5);
    let d = ir.slice(24, 3);
    let a = ir.slice(21, 3);
    let rb = ir.slice(18, 3);
    let imm = ir.slice(0, 16);
    let valid = any(&ALL
        .iter()
        .map(|i| ir.and(&E::num(32, i.mask())).is(i.opcode << 27))
        .collect::<Vec<_>>());
    let commit = execute.and(&valid);
    b.section("1. Instruction register / phase / checked decoding");
    b.register("ir", &instruction, &run.and(&phase.not()), &reset);
    b.register("phase", &phase.not(), &run, &reset);
    b.register("halted", &E::num(1, 1), &commit.and(&op.is(HALT)), &reset);
    b.register("fault", &E::num(1, 1), &execute.and(&valid.not()), &reset);
    b.section("2. Register file and combinational ALU");
    let va = E::net("A", 16);
    let vb = E::net("B", 16);
    let result = E::net("RESULT", 16);
    let write_ops = [
        LI, ADD, SUB, AND, OR, XOR, SHL, SHR, SAR, SLTU, SLT, ADDI, ANDI, ORI, XORI, LD, JAL,
    ];
    let write = commit.and(&any(&write_ops
        .into_iter()
        .map(|n| op.is(n))
        .collect::<Vec<_>>()));
    let nextpc = pc.add(&E::num(16, 1));
    let value = op
        .is(LD)
        .choose(&data, &op.is(JAL).choose(&nextpc, &result));
    hierarchy::connect(
        &mut b,
        project,
        regs,
        None,
        &[
            ("CLK", clk),
            ("RESET", reset.clone()),
            ("WE", write),
            ("DEST", d.clone()),
            ("RA", a),
            ("RB", op.is(ST).choose(&d, &rb)),
            ("WDATA", value),
        ],
        &[("A", "A"), ("B", "B")],
    );
    b.section("3. ALU / address generation");
    hierarchy::connect(
        &mut b,
        project,
        alu,
        None,
        &[
            ("OP", op.clone()),
            ("A", va.clone()),
            ("B", vb.clone()),
            ("IMM", imm.clone()),
        ],
        &[("RESULT", "RESULT")],
    );
    b.section("4. Program counter / branches / external bus");
    let branch = op
        .is(BZ)
        .and(&va.is(0))
        .or(&op.is(BNZ).and(&va.is(0).not()));
    let direct = any(&[branch, op.is(JMP), op.is(JAL)]);
    let target = op.is(JR).choose(&va, &direct.choose(&imm, &nextpc));
    b.register("pc", &target, &commit.and(&op.is(HALT).not()), &reset);
    b.output("PC", &pc);
    b.output("ADDR", &va.add(&imm));
    b.output("WDATA", &vb);
    b.output("RD", &commit.and(&op.is(LD)));
    b.output("WR", &commit.and(&op.is(ST)));
    b.output("HALTED", &halted);
    b.output("FAULT", &fault);
    b.output("EXEC", &phase);
    hierarchy::define(project, "G16 CPU", b.finish())
}
