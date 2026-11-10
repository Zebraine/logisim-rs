//! Reproducible, editable example documents; never linked into the desktop host.
mod builder;
mod snake;
use std::path::Path;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match args.first().and_then(|a| a.to_str()) {
        Some("generate") if args.len() == 4 => {
            let input = logisim_plugin::Plugin::load(&std::fs::read(&args[1])?)?;
            let display = logisim_plugin::Plugin::load(&std::fs::read(&args[2])?)?;
            let project = snake::generate(&input, &display, &snake::Start::default())?;
            let text = logisim_file::serialize(&project)?;
            let roundtrip = logisim_file::parse(&text)?;
            assert!(roundtrip.warnings.is_empty());
            assert!(
                logisim_file::serialize(&roundtrip.document)? == text,
                "File must roundtrip without changes"
            );
            std::fs::write(&args[3], text)?;
            println!(
                "Generated {} circuits, {} components",
                project.circuits.len(),
                project
                    .circuits
                    .iter()
                    .map(|c| c.circuit.components.len())
                    .sum::<usize>()
            );
        }
        Some("check") if args.len() == 2 => snake::check(Path::new(&args[1]))?,
        _ => {
            return Err(concat!(
                "Usage: snake-example generate input.wasm display.wasm ",
                "examples/games/snake.circ | check examples/games/snake.circ"
            )
            .into());
        }
    }
    Ok(())
}
