//! Verify actual serialized circuits and Wasm; no CPU emulator is used by the application.
mod cases;
mod machine;
mod model;
use super::*;
use crate::assembler;
use machine::Machine;
use model::Model;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn check(path: &Path) -> Result<()> {
    let (project, plugins) = load(&path.join("game.circ"))?;
    let text = std::fs::read_to_string(path.join("game.circ"))?;
    assert_eq!(logisim_file::serialize(&project)?, text, "File roundtrip");
    let words = assembler::assemble(&std::fs::read_to_string(path.join("cpu-demo.asm"))?)?;
    assert_eq!(
        assembler::hex(&words),
        std::fs::read_to_string(path.join("cpu-demo.hex"))?,
        "Stale assembly image"
    );
    let input = Plugin::load(&std::fs::read(path.join("../../plugins/input/input.wasm"))?)?;
    let display = Plugin::load(&std::fs::read(
        path.join("../../plugins/display/display.wasm"),
    )?)?;
    assert_eq!(
        logisim_file::serialize(&generate(&input, &display, &words)?)?,
        text,
        "Stale circuit generation"
    );
    for stable in [true, false] {
        cases::trace(&input, &display, &plugins, stable)?;
        cases::peripherals(&project, &plugins, stable)?;
        cases::replacement(&input, &display, &plugins, stable)?;
        println!(
            "{}: ISA traces, memory, branches, faults, reset, isolation, keyboard, framebuffer and program replacement passed",
            if stable { "Stable" } else { "Events" }
        );
    }
    println!("G16: assembly/hex/ROM consistency and portable file roundtrip passed");
    Ok(())
}
fn compare(m: &Machine, model: &Model) {
    assert_eq!(m.state("pc"), u32::from(model.pc), "pc");
    assert_eq!(m.state("halted"), u32::from(model.halted), "halted");
    assert_eq!(m.state("fault"), u32::from(model.fault), "fault");
    for i in 0..8 {
        assert_eq!(m.register(i), model.regs[i], "r{i} at pc {}", model.pc);
    }
    for a in 0..4096 {
        assert_eq!(
            m.word(a),
            *model.ram.get(&(a as u16)).unwrap_or(&0),
            "RAM {a:04x}"
        );
    }
    assert_eq!(m.io("debug"), u32::from(model.debug));
}
