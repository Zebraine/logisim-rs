//! Programmable board: replace the ROM image to change the application.
pub mod io;
mod verify;
use crate::{
    builder::{Builder, E, hierarchy},
    cpu,
};
use logisim_core::*;
use logisim_plugin::Plugin;
use std::path::Path;
pub use verify::check;

pub fn generate(input: &Plugin, display: &Plugin, words: &[u32]) -> Result<Project, String> {
    if words.len() > 65536 {
        return Err("Program exceeds 64K words".into());
    }
    let mut project = Project::default();
    let cpu = cpu::define(&mut project)?;
    let bus = io::define(&mut project)?;
    let mut b = Builder::new("G16 programmable computer / software controls keyboard and pixels");
    b.text(
        "CPU stage: cpu-demo.asm / ROM image: cpu-demo.hex / not yet the Snake program",
        Point::new(30, 50),
        16,
    );
    b.text(
        "Enable Auto ticks at 4096 Hz. Poke keyboard; arrow keys draw pixels. Esc releases focus.",
        Point::new(30, 80),
        14,
    );
    b.text(
        "ROM: load a v2.0 raw hex image, then Reset. r0 = 0; two clock cycles per instruction.",
        Point::new(30, 110),
        14,
    );
    let mut clock = Component::new(Kind::Clock, Point::new(120, 200));
    clock.label = "CPU clock".into();
    let id = b.circuit.add_component(clock);
    b.attach(id, 0, "CLK", true);
    let mut reset = Component::new(Kind::Button, Point::new(120, 290));
    reset.label = "RESET".into();
    let id = b.circuit.add_component(reset);
    b.attach(id, 0, "RESET", true);
    let clk = E::net("CLK", 1);
    let reset = E::net("RESET", 1);
    let mut rom = Component::new(Kind::Rom, Point::new(460, 240));
    rom.bits = 32;
    rom.memory.address_bits = 16;
    rom.memory.select_high = true;
    rom.label = "Program ROM / import .hex".into();
    for (i, word) in words.iter().enumerate() {
        rom.memory.contents.set(i as u32, *word);
    }
    let id = b.circuit.add_component(rom);
    b.attach(id, 0, "INST", true);
    b.connect(id, 1, &E::net("PC", 16));
    b.attach(id, 2, "SELECT", false);
    let mut select = Component::new(Kind::Constant, Point::new(120, 370));
    select.bits = 1;
    select.constant = 1;
    let id = b.circuit.add_component(select);
    b.attach(id, 0, "SELECT", true);
    hierarchy::connect(
        &mut b,
        &project,
        cpu,
        Some(Point::new(220, 470)),
        &[
            ("CLK", clk.clone()),
            ("RESET", reset.clone()),
            ("INST", E::net("INST", 32)),
            ("RDATA", E::net("RDATA", 16)),
        ],
        &[
            ("PC", "PC"),
            ("ADDR", "ADDR"),
            ("WDATA", "WDATA"),
            ("RD", "RD"),
            ("WR", "WR"),
            ("HALTED", "HALTED"),
            ("FAULT", "FAULT"),
            ("EXEC", "EXEC"),
        ],
    );
    hierarchy::connect(
        &mut b,
        &project,
        bus,
        Some(Point::new(740, 470)),
        &[
            ("CLK", clk.clone()),
            ("RESET", reset.clone()),
            ("ADDR", E::net("ADDR", 16)),
            ("WDATA", E::net("WDATA", 16)),
            ("WR", E::net("WR", 1)),
            ("KEY_HELD", E::net("KEY_HELD", 8)),
            ("KEY", E::net("KEY", 3)),
            ("KEY_DOWN", E::net("KEY_DOWN", 1)),
            ("KEY_VALID", E::net("KEY_VALID", 1)),
        ],
        &[
            ("RDATA", "RDATA"),
            ("X", "X"),
            ("Y", "Y"),
            ("COLOR", "COLOR"),
            ("DEBUG", "DEBUG"),
            ("KEY_ACK", "KEY_ACK"),
            ("KEY_CLEAR", "KEY_CLEAR"),
            ("WRITE", "WRITE"),
            ("CLEAR", "CLEAR"),
            ("PRESENT", "PRESENT"),
        ],
    );
    let mut key = input
        .manifest()
        .components()
        .into_iter()
        .find(|c| c.tool.id == "keyboard")
        .ok_or("Missing keyboard tool")?;
    key.properties.insert("trigger".into(), "falling".into());
    let id = b
        .circuit
        .add_component(Component::from_plugin(key, Point::new(730, 210)));
    for (p, n, output) in [
        (0, "CLK", false),
        (1, "KEY_ACK", false),
        (2, "KEY_CLEAR", false),
        (3, "KEY_HELD", true),
        (4, "KEY", true),
        (5, "KEY_DOWN", true),
        (6, "KEY_VALID", true),
    ] {
        b.attach(id, p, n, output);
    }
    let screen = display
        .manifest()
        .components()
        .into_iter()
        .find(|c| c.tool.id == "framebuffer16")
        .ok_or("Missing framebuffer16 tool")?;
    let id = b
        .circuit
        .add_component(Component::from_plugin(screen, Point::new(1260, 210)));
    b.attach(id, 0, "PCLK", false);
    for (p, n) in [
        (1, "X"),
        (2, "Y"),
        (3, "COLOR"),
        (4, "WRITE"),
        (5, "CLEAR"),
        (6, "PRESENT"),
    ] {
        b.attach(id, p, n, false);
    }
    for (i, label) in ["HALTED", "FAULT"].into_iter().enumerate() {
        let mut led = Component::new(Kind::Led, Point::new(1250 + i as i32 * 160, 490));
        led.label = label.into();
        led.label_facing = Facing::South;
        let id = b.circuit.add_component(led);
        b.attach(id, 0, label, false);
    }
    for (i, label) in ["PC", "DEBUG"].into_iter().enumerate() {
        let at = Point::new(1270, 620 + i as i32 * 120);
        let mut probe = Component::new(Kind::Probe, at);
        probe.label = label.into();
        probe.radix = Radix::Hex;
        probe.facing = Facing::West;
        b.circuit.add_component(probe);
        b.tunnel(at - Point::new(40, 0), label, 16, Facing::East, false);
        b.circuit.add_wire(at - Point::new(40, 0), at);
    }
    let invert = Component::new(Kind::Gate(Gate::Not), Point::new(1110, 430));
    let id = b.circuit.add_component(invert);
    b.attach(id, 0, "PCLK", true);
    b.attach(id, 1, "CLK", false);
    project.main_circuit_mut().circuit = b.finish();
    for (plugin, name) in [(input, "input"), (display, "display")] {
        project.plugin_sources.push(plugin::PluginSource {
            plugin: plugin.manifest().id.clone(),
            version: plugin.manifest().version.clone(),
            path: format!("../../plugins/{name}/{name}.wasm"),
            fallback: String::new(),
        });
    }
    project.refresh_interfaces(false);
    project.validate()?;
    Ok(project)
}
pub fn load(
    path: &Path,
) -> Result<(Project, logisim_plugin::Registry), Box<dyn std::error::Error>> {
    let imported = logisim_file::parse(&std::fs::read_to_string(path)?)?;
    if !imported.warnings.is_empty() {
        return Err(format!("Import warnings: {:?}", imported.warnings).into());
    }
    let mut registry = logisim_plugin::Registry::default();
    for source in &imported.document.plugin_sources {
        registry.load(&std::fs::read(
            path.parent().unwrap_or(Path::new(".")).join(&source.path),
        )?)?;
    }
    Ok((imported.document, registry))
}
