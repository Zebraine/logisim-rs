mod engine;
mod verify;
use crate::builder::Builder;
pub use engine::Start;
use logisim_core::*;
use logisim_plugin::Plugin;
use std::collections::BTreeMap;
use std::path::Path;
pub use verify::check;

pub fn generate(input: &Plugin, display: &Plugin, start: &Start) -> Result<Project, String> {
    let mut project = Project::default();
    let engine_id = project.add("Snake engine".into())?;
    let mut circuit = engine::build(start);
    let mut appearance = Appearance {
        shapes: vec![],
        ports: BTreeMap::new(),
        anchor: Point::ZERO,
        facing: Facing::East,
    };
    appearance.add(
        Geometry::Rect {
            a: Point::ZERO,
            b: Point::new(180, 520),
            radius: 5,
        },
        Paint {
            fill: Some([241, 245, 249]),
            ..Default::default()
        },
    );
    let mut incoming = 0;
    let mut outgoing = 0;
    for pin in circuit.components.iter().filter(|c| c.kind == Kind::Pin) {
        let y = if pin.output {
            outgoing += 1;
            outgoing * 40
        } else {
            incoming += 1;
            incoming * 60
        };
        let x = if pin.output { 180 } else { 0 };
        appearance.ports.insert(pin.id, Point::new(x, y));
        appearance.add(
            Geometry::Text {
                at: Point::new(if pin.output { 170 } else { 10 }, y - 3),
                text: pin.label.clone(),
                font: ArtworkFont {
                    size: 10,
                    ..Default::default()
                },
                anchor: if pin.output {
                    TextAnchor::End
                } else {
                    TextAnchor::Start
                },
            },
            Paint {
                fill: Some([30, 41, 59]),
                stroke: None,
                width: 1,
            },
        );
    }
    circuit.appearance = Some(appearance);
    project.circuit_mut(engine_id).unwrap().circuit = circuit;
    let mut b = Builder::new("SNAKE / keyboard + pixel display plugins");
    b.text(
        "Poke the keyboard · Arrows: turn · Space: pause · Enter: restart",
        Point::new(30, 50),
        16,
    );
    b.text(
        "Enable Auto ticks at 1024 Hz (about 7 moves/s). Escape releases keyboard focus.",
        Point::new(30, 80),
        14,
    );
    b.text(
        "Green: snake · Red: food · Walls and body collisions end the game",
        Point::new(30, 110),
        14,
    );
    let mut keyboard = input
        .manifest()
        .components()
        .into_iter()
        .find(|c| c.tool.id == "keyboard")
        .ok_or("Missing keyboard tool")?;
    keyboard
        .properties
        .insert("trigger".into(), "falling".into());
    let id = b
        .circuit
        .add_component(Component::from_plugin(keyboard, Point::new(180, 220)));
    for (port, net, driver) in [
        (0, "CLK", false),
        (1, "READ", false),
        (2, "RESET", false),
        (4, "KEY", true),
        (5, "DOWN", true),
        (6, "VALID", true),
    ] {
        b.attach(id, port, net, driver);
    }
    let screen = display
        .manifest()
        .components()
        .into_iter()
        .find(|c| c.tool.id == "framebuffer16")
        .ok_or("Missing framebuffer16 tool")?;
    let id = b
        .circuit
        .add_component(Component::from_plugin(screen, Point::new(620, 220)));
    for (port, net) in ["CLK", "X", "Y", "COLOR", "WRITE", "CLEAR", "PRESENT"]
        .into_iter()
        .enumerate()
    {
        b.attach(id, port, net, false);
    }
    let mut clock = Component::new(Kind::Clock, Point::new(180, 390));
    clock.label = "System clock".into();
    let id = b.circuit.add_component(clock);
    b.attach(id, 0, "CLK", true);
    b.text(
        "Open Snake engine in circuit tree",
        Point::new(1100, 140),
        14,
    );
    let instance = project.instance(engine_id, Point::new(1120, 180)).unwrap();
    let ports: Vec<_> = instance
        .interface
        .ports
        .iter()
        .map(|p| (p.label.clone(), p.output))
        .collect();
    let id = b.circuit.add_component(instance);
    for (port, (net, driver)) in ports.iter().enumerate() {
        b.attach(id, port, net, *driver);
    }
    for (i, (label, color)) in [
        ("PAUSED", [255, 180, 0, 255]),
        ("DEAD", [255, 0, 0, 255]),
        ("WON", [0, 200, 80, 255]),
    ]
    .into_iter()
    .enumerate()
    {
        let mut led = Component::new(Kind::Led, Point::new(650 + i as i32 * 150, 490));
        led.label = label.into();
        led.label_facing = Facing::South;
        led.io.on_color = color;
        let id = b.circuit.add_component(led);
        b.attach(id, 0, label, false);
    }
    let mut score = Component::new(Kind::Probe, Point::new(740, 620));
    score.label = "SCORE".into();
    score.radix = Radix::Unsigned;
    score.facing = Facing::West;
    b.circuit.add_component(score);
    // A probe infers its bus width; this explicit wire/tunnel remains ordinary document data.
    b.tunnel(Point::new(700, 620), "SCORE", 8, Facing::East, false);
    b.circuit
        .add_wire(Point::new(700, 620), Point::new(740, 620));
    b.text(
        "Snake memory is runtime-only. Reset or Enter starts a fresh game.",
        Point::new(30, 760),
        13,
    );
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
