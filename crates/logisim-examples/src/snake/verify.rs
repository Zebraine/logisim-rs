//! Acceptance uses the serialized document, real Wasm and the ordinary simulator.
mod cases;
use super::*;
use logisim_core::plugin::api::interaction::{InputEvent, Key};
use logisim_sim::ProjectSimulator;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
struct Game {
    sim: ProjectSimulator,
    root: Vec<Id>,
    path: Vec<Id>,
    registers: BTreeMap<String, Id>,
    memories: BTreeMap<String, Id>,
    indicators: BTreeMap<String, Id>,
    keyboard: Id,
    screen: Id,
    stable: bool,
}
impl Game {
    fn new(
        project: &Project,
        plugins: &logisim_plugin::Registry,
        stable: bool,
        root: Vec<Id>,
    ) -> Result<Self> {
        let main = &project.named("main").unwrap().circuit;
        let engine = main
            .components
            .iter()
            .find(|c| matches!(c.kind, Kind::Subcircuit(_)))
            .unwrap()
            .id;
        let mut path = root.clone();
        path.push(engine);
        let def = &project.named("Snake engine").unwrap().circuit;
        let registers = def
            .components
            .iter()
            .filter(|c| c.kind == Kind::Register)
            .map(|c| (c.label.clone(), c.id))
            .collect();
        let memories = def
            .components
            .iter()
            .filter(|c| c.kind == Kind::Ram)
            .map(|c| (c.label.clone(), c.id))
            .collect();
        let indicators = main
            .components
            .iter()
            .filter(|c| matches!(c.kind, Kind::Probe | Kind::Led))
            .map(|c| (c.label.clone(), c.id))
            .collect();
        let plugin_id = |tool: &str| {
            main.components
                .iter()
                .find(|c| c.plugin.as_ref().is_some_and(|p| p.tool.id == tool))
                .unwrap()
                .id
        };
        let mut sim = ProjectSimulator::default();
        sim.set_plugins(plugins.clone());
        sim.synchronize(project, project.main);
        let mut game = Self {
            sim,
            root,
            path,
            registers,
            memories,
            indicators,
            keyboard: plugin_id("keyboard"),
            screen: plugin_id("framebuffer16"),
            stable,
        };
        game.settle()?;
        Ok(game)
    }
    fn settle(&mut self) -> Result<()> {
        if self.stable {
            self.sim.settle_compiled().map_err(|e| format!("{e:?}"))?;
        } else {
            self.sim.advance(100_000);
        }
        if self.sim.limited()
            || self.sim.pending_count() != 0
            || self.sim.error().is_some()
            || !self.sim.plugin_failures().is_empty()
        {
            return Err(format!(
                "Simulation did not settle: {:?} {:?}",
                self.sim.error(),
                self.sim.plugin_failures()
            )
            .into());
        }
        for path in [&self.root, &self.path] {
            let diagnostics = self.sim.diagnostics(path);
            if !diagnostics.is_empty() {
                return Err(format!("Diagnostics at {path:?}: {diagnostics:?}").into());
            }
        }
        Ok(())
    }
    fn cycle(&mut self) -> Result<()> {
        for _ in 0..2 {
            assert!(self.sim.tick());
            self.settle()?;
        }
        Ok(())
    }
    fn until(&mut self, limit: usize, condition: impl Fn(&Self) -> bool) -> Result<()> {
        for _ in 0..limit {
            if condition(self) {
                return Ok(());
            }
            self.cycle()?;
        }
        Err(format!(
            "Timed out at phase {}, head {:02x}",
            self.reg("phase"),
            self.reg("head")
        )
        .into())
    }
    fn reg(&self, name: &str) -> u32 {
        self.sim
            .view(&self.path)
            .unwrap()
            .stored_value(self.registers[name])
            .and_then(|v| v.number())
            .unwrap_or_else(|| panic!("Undefined register {name}"))
    }
    fn word(&self, name: &str, index: u32) -> u32 {
        self.sim
            .view(&self.path)
            .unwrap()
            .memory_contents(self.memories[name])
            .unwrap()
            .get(index)
    }
    fn indicator(&self, name: &str) -> u32 {
        self.sim
            .view(&self.root)
            .unwrap()
            .port_value(self.indicators[name], 0)
            .and_then(|v| v.number())
            .unwrap_or_else(|| panic!("Undefined indicator {name}"))
    }
    fn image(&self) -> Arc<plugin::api::raster::Image> {
        self.sim
            .view(&self.root)
            .unwrap()
            .plugin_raster(self.screen)
            .unwrap()
            .clone()
    }
    fn input(&mut self, event: InputEvent) -> Result<()> {
        self.sim
            .plugin_action(&self.root, self.keyboard, "key".into(), event.argument());
        self.settle()
    }
    fn press(&mut self, key: Key) -> Result<()> {
        self.input(InputEvent::Key { key, pressed: true })?;
        self.input(InputEvent::Key {
            key,
            pressed: false,
        })
    }
    fn ready(&mut self) -> Result<()> {
        self.until(600, |g| g.reg("phase") == 8)
    }
    fn moved(&mut self) -> Result<()> {
        let old = self.reg("head");
        self.until(350, |g| {
            g.reg("head") != old || matches!(g.reg("phase"), 15 | 16)
        })?;
        self.until(350, |g| matches!(g.reg("phase"), 8 | 15 | 16))
    }
    fn assert_body(&self, body: &VecDeque<u8>) {
        assert_eq!(self.reg("length"), body.len() as u32);
        assert_eq!(self.reg("head"), u32::from(*body.back().unwrap()));
        assert_eq!(
            self.reg("write_ptr"),
            (self.reg("tail_ptr") + body.len() as u32) & 255
        );
        for (i, cell) in body.iter().enumerate() {
            assert_eq!(
                self.word("body", (self.reg("tail_ptr") + i as u32) & 255),
                u32::from(*cell)
            );
        }
        let frame = self.image();
        let food = self.reg("food") as u8;
        let won = self.reg("phase") == 15;
        for at in 0..256 {
            let snake = body.contains(&(at as u8));
            assert_eq!(
                self.word("occupied", at),
                u32::from(snake),
                "occupied {at:02x}"
            );
            let expected = if snake {
                [0, 255, 0, 255]
            } else if !won && at == u32::from(food) {
                [255, 0, 0, 255]
            } else {
                [0, 0, 0, 255]
            };
            assert_eq!(
                &frame.pixels[at as usize * 4..at as usize * 4 + 4],
                &expected,
                "pixel {at:02x}"
            );
        }
        assert!(won || !body.contains(&food));
    }
}
fn step(body: &mut VecDeque<u8>, direction: u32, food: u32) {
    let head = *body.back().unwrap();
    let next = match direction {
        0 => head + 1,
        1 => head + 16,
        2 => head - 1,
        _ => head - 16,
    };
    if u32::from(next) != food {
        body.pop_front();
    }
    body.push_back(next);
}
pub fn check(path: &Path) -> Result<()> {
    let (project, plugins) = load(path)?;
    let input_source = project
        .plugin_sources
        .iter()
        .find(|s| s.plugin == "org.logisim-rs.input")
        .unwrap();
    let display_source = project
        .plugin_sources
        .iter()
        .find(|s| s.plugin == "org.logisim-rs.display")
        .unwrap();
    let base = path.parent().unwrap_or(Path::new("."));
    let input = Plugin::load(&std::fs::read(base.join(&input_source.path))?)?;
    let display = Plugin::load(&std::fs::read(base.join(&display_source.path))?)?;
    let expected = logisim_file::serialize(&generate(&input, &display, &Start::default())?)?;
    assert!(
        expected == std::fs::read_to_string(path)?,
        "Example is out of date; regenerate it"
    );
    for stable in [true, false] {
        cases::play(&project, &plugins, stable)?;
        cases::boundaries(&input, &display, &plugins, stable)?;
        println!(
            "{}: controls, queued turns, pause/resume, wall collision, restart and frame/RAM invariants verified",
            if stable { "Stable" } else { "Events" }
        );
    }
    println!(
        "Snake: growth, vacating tail, self collision, full-board win, bounded food search, isolation and portable file roundtrip verified"
    );
    Ok(())
}
