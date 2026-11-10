//! Verify actual serialized circuits and Wasm; no CPU emulator is used by the application.
mod cases;
mod games;
mod machine;
mod model;
use super::*;
use crate::assembler;
use machine::Machine;
use model::Model;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn check(path: &Path) -> Result<()> {
    let (_, plugins) = load(&path.join("game.circ"))?;
    let input = Plugin::load(&std::fs::read(path.join("../../plugins/input/input.wasm"))?)?;
    let display = Plugin::load(&std::fs::read(
        path.join("../../plugins/display/display.wasm"),
    )?)?;
    let mut boards = Vec::new();
    for (program, circuit) in [
        ("cpu-demo", None),
        ("snake", Some("game.circ")),
        ("ping-pong", Some("ping-pong.circ")),
    ] {
        let source = std::fs::read_to_string(path.join(format!("{program}.asm")))?;
        let code = assembler::compile(&source)?;
        assert_eq!(
            assembler::hex(&code.words),
            std::fs::read_to_string(path.join(format!("{program}.hex")))?,
            "Stale {program} image"
        );
        let generated = generate(&input, &display, &code.words)?;
        let project = if let Some(circuit) = circuit {
            let (project, _) = load(&path.join(circuit))?;
            let text = std::fs::read_to_string(path.join(circuit))?;
            assert_eq!(
                logisim_file::serialize(&project)?,
                text,
                "File roundtrip {circuit}"
            );
            assert_eq!(
                logisim_file::serialize(&generated)?,
                text,
                "Stale circuit generation {circuit}"
            );
            project
        } else {
            generated
        };
        boards.push((project, code));
    }
    for stable in [true, false] {
        cases::trace(&input, &display, &plugins, stable)?;
        cases::peripherals(&boards[0].0, &plugins, stable)?;
        cases::replacement(&input, &display, &plugins, stable)?;
        games::check(&boards[1..], &plugins, stable)?;
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
