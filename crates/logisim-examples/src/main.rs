//! Reproducible circuit authoring and assembly tools; never linked into the desktop host.
mod assembler;
mod board;
mod builder;
mod cpu;
use std::path::Path;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match args.first().and_then(|s| s.to_str()) {
        Some("assemble") if args.len() == 3 => {
            let words = assembler::assemble(&std::fs::read_to_string(&args[1])?)?;
            std::fs::write(&args[2], assembler::hex(&words))?;
            println!("Assembled {} words", words.len());
        }
        Some("generate") if args.len() == 5 => {
            let image = logisim_file::parse_hex(&std::fs::read_to_string(&args[1])?, 16, 32)
                .map_err(|e| format!("Invalid hex image: {e:?}"))?;
            let input = logisim_plugin::Plugin::load(&std::fs::read(&args[2])?)?;
            let display = logisim_plugin::Plugin::load(&std::fs::read(&args[3])?)?;
            let words: Vec<_> = (0..65536).map(|i| image.get(i)).collect();
            let project = board::generate(&input, &display, &words)?;
            let text = logisim_file::serialize(&project)?;
            let parsed = logisim_file::parse(&text)?;
            assert!(parsed.warnings.is_empty());
            assert_eq!(
                logisim_file::serialize(&parsed.document)?,
                text,
                "Roundtrip changes circuit"
            );
            std::fs::write(&args[4], text)?;
            println!("Generated {} circuit definitions", project.circuits.len());
        }
        Some("check") if args.len() == 2 => board::check(Path::new(&args[1]))?,
        _ => {
            return Err(concat!(
                "Usage: cpu-tool assemble program.asm program.hex | ",
                "generate program.hex input.wasm display.wasm game.circ | ",
                "check examples/game"
            )
            .into());
        }
    }
    Ok(())
}
