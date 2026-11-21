//! Run from the workspace root. Fixture generation and comparison use no file writes.
use logisim_core::{Component, Kind, Pla, PlaInput, Point, Project};
use logisim_file::{Error, ProjectFormat, native::study};
use std::{env, fs::File, io::Read};

fn memory_board() -> Project {
    let mut p = Project::default();
    let mut rom = Component::new(Kind::Rom, Point::ZERO);
    rom.bits = 32;
    rom.memory.address_bits = 14;
    for i in 0..8192u32 {
        rom.memory
            .contents
            .fill(i, i + 1, (i + 1).wrapping_mul(2654435761));
    }
    let mut pla = Component::new(Kind::PlaRom, Point::new(300, 200));
    pla.pla = Pla::new(32, 32, 32);
    for r in 0..32 {
        for c in 0..32 {
            pla.pla.set_input(
                r,
                c,
                [PlaInput::Disconnected, PlaInput::Direct, PlaInput::Inverted]
                    [(usize::from(r) + usize::from(c)) % 3],
            );
            pla.pla.set_output(r, c, (r + c) % 3 == 0);
        }
    }
    let circuit = &mut p.main_circuit_mut().circuit;
    circuit.add_component(rom);
    circuit.add_component(pla);
    p
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let runs = args
        .next()
        .ok_or("usage: lrs-codec-study RUNS FILE.circ|large-rom-pla ...")?
        .parse()?;
    for sample in args {
        let mut p = if sample == "large-rom-pla" {
            memory_board()
        } else {
            let mut bytes = Vec::new();
            File::open(&sample)?
                .take(logisim_file::MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > logisim_file::MAX_BYTES {
                return Err("Study sample exceeds 8 MiB".into());
            }
            let imported = ProjectFormat::Circ.parse(&bytes)?;
            if imported.partial || !imported.warnings.is_empty() {
                return Err(Box::new(Error(format!(
                    "Incomplete {sample}: {:?}",
                    imported.warnings
                ))));
            }
            imported.document
        };
        // Legacy samples with missing IDs must have identical identities across processes.
        let mut used = std::collections::BTreeSet::new();
        for d in &p.circuits {
            used.extend(d.circuit.agent_ids.circuit);
            used.extend(d.circuit.agent_ids.items.values().copied());
        }
        let mut next = 1u128;
        let mut id = || {
            while !used.insert(next) {
                next += 1;
            }
            next
        };
        for d in &mut p.circuits {
            let c = &mut d.circuit;
            c.agent_ids.circuit.get_or_insert_with(&mut id);
            for item in c
                .components
                .iter()
                .map(|c| c.id)
                .chain(c.wires.iter().map(|w| w.id))
            {
                c.agent_ids.items.entry(item).or_insert_with(&mut id);
            }
        }
        println!(
            "{}",
            serde_json::json!({"sample":sample,"runs":runs,"formats":study::report(&p,runs)?})
        );
    }
    Ok(())
}
