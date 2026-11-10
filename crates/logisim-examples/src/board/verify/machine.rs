use super::*;
use logisim_core::plugin::api::interaction::{InputEvent, Key};
use logisim_sim::ProjectSimulator;
use std::collections::BTreeMap;
pub(super) struct Machine {
    pub sim: ProjectSimulator,
    pub root: Vec<Id>,
    pub cpu: Vec<Id>,
    pub regs: Vec<Id>,
    pub io: Vec<Id>,
    cpu_ids: BTreeMap<String, Id>,
    reg_ids: BTreeMap<String, Id>,
    io_ids: BTreeMap<String, Id>,
    keyboard: Id,
    screen: Id,
    reset: Id,
    ram: Id,
    stable: bool,
}
impl Machine {
    pub fn new(
        project: &Project,
        plugins: &logisim_plugin::Registry,
        stable: bool,
        root: Vec<Id>,
    ) -> Result<Self> {
        let def = |name: &str| &project.named(name).unwrap().circuit;
        let instance = |parent: &str, child: &str| {
            let id = project.named(child).unwrap().id;
            def(parent)
                .components
                .iter()
                .find(|c| c.kind == Kind::Subcircuit(id))
                .unwrap()
                .id
        };
        let mut cpu = root.clone();
        cpu.push(instance("main", "G16 CPU"));
        let mut regs = cpu.clone();
        regs.push(instance("G16 CPU", "G16 registers"));
        let mut io = root.clone();
        io.push(instance("main", "G16 memory and IO"));
        let ids = |name: &str| {
            def(name)
                .components
                .iter()
                .filter(|c| c.kind == Kind::Register)
                .map(|c| (c.label.clone(), c.id))
                .collect()
        };
        let plugin = |tool: &str| {
            def("main")
                .components
                .iter()
                .find(|c| c.plugin.as_ref().is_some_and(|p| p.tool.id == tool))
                .unwrap()
                .id
        };
        let mut sim = ProjectSimulator::default();
        sim.set_plugins(plugins.clone());
        sim.synchronize(project, project.main);
        let mut m = Self {
            sim,
            root,
            cpu,
            regs,
            io,
            cpu_ids: ids("G16 CPU"),
            reg_ids: ids("G16 registers"),
            io_ids: ids("G16 memory and IO"),
            keyboard: plugin("keyboard"),
            screen: plugin("framebuffer16"),
            reset: def("main")
                .components
                .iter()
                .find(|c| c.kind == Kind::Button)
                .unwrap()
                .id,
            ram: def("G16 memory and IO")
                .components
                .iter()
                .find(|c| c.kind == Kind::Ram)
                .unwrap()
                .id,
            stable,
        };
        m.settle()?;
        Ok(m)
    }
    pub fn settle(&mut self) -> Result<()> {
        if self.stable {
            self.sim.settle_compiled().map_err(|e| format!("{e:?}"))?;
        } else {
            self.sim.advance(200_000);
        }
        if self.sim.pending_count() != 0
            || self.sim.limited()
            || self.sim.error().is_some()
            || !self.sim.plugin_failures().is_empty()
        {
            return Err(format!(
                "Did not settle: {:?} {:?}",
                self.sim.error(),
                self.sim.plugin_failures()
            )
            .into());
        }
        for path in [&self.root, &self.cpu, &self.regs, &self.io] {
            assert!(
                self.sim.diagnostics(path).is_empty(),
                "Diagnostics at {path:?}: {:?}",
                self.sim.diagnostics(path)
            );
        }
        Ok(())
    }
    pub fn cycle(&mut self) -> Result<()> {
        for _ in 0..2 {
            assert!(self.sim.tick());
            self.settle()?;
        }
        Ok(())
    }
    pub fn instruction(&mut self) -> Result<()> {
        self.cycle()?;
        self.cycle()
    }
    pub fn until(&mut self, limit: usize, predicate: impl Fn(&Self) -> bool) -> Result<()> {
        for _ in 0..limit {
            if predicate(self) {
                return Ok(());
            }
            self.cycle()?;
        }
        Err(format!(
            "Timeout pc={} phase={} fault={}",
            self.state("pc"),
            self.state("phase"),
            self.state("fault")
        )
        .into())
    }
    fn stored(&self, path: &[Id], id: Id) -> u32 {
        self.sim
            .view(path)
            .unwrap()
            .stored_value(id)
            .and_then(|v| v.number())
            .expect("Undefined register")
    }
    pub fn state(&self, name: &str) -> u32 {
        self.stored(&self.cpu, self.cpu_ids[name])
    }
    pub fn register(&self, index: usize) -> u16 {
        if index == 0 {
            0
        } else {
            self.stored(&self.regs, self.reg_ids[&format!("r{index}")]) as u16
        }
    }
    pub fn io(&self, name: &str) -> u32 {
        self.stored(&self.io, self.io_ids[name])
    }
    pub fn word(&self, address: u32) -> u16 {
        self.sim
            .view(&self.io)
            .unwrap()
            .memory_contents(self.ram)
            .unwrap()
            .get(address) as u16
    }
    pub fn press(&mut self, key: Key) -> Result<()> {
        for pressed in [true, false] {
            self.key(key, pressed)?;
        }
        Ok(())
    }
    pub fn key(&mut self, key: Key, pressed: bool) -> Result<()> {
        self.sim.plugin_action(
            &self.root,
            self.keyboard,
            "key".into(),
            InputEvent::Key { key, pressed }.argument(),
        );
        self.settle()
    }
    pub fn set_word(&mut self, address: u32, value: u16) -> Result<()> {
        self.sim
            .set_memory_word(&self.io, self.ram, address, value.into());
        self.settle()
    }
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        let image = self
            .sim
            .view(&self.root)
            .unwrap()
            .plugin_raster(self.screen)
            .unwrap();
        image.pixels[(y * 16 + x) * 4..(y * 16 + x + 1) * 4]
            .try_into()
            .unwrap()
    }
    pub fn reset(&mut self) -> Result<()> {
        self.sim.set_button(&self.root, self.reset, true);
        self.settle()?;
        self.cycle()?;
        self.sim.set_button(&self.root, self.reset, false);
        self.settle()
    }
}
