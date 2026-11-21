//! Fixed inputs and observation points for the four stable-engine traces.
use crate::Result;
use logisim_core::{Arithmetic, Circuit, Component, Gate, Id, Kind, Point, Project};
use logisim_plugin::{Plugin, Registry};
use logisim_sim::ProjectSimulator;
use std::{fs, path::Path, time::Instant};

const LANES: usize = 64;
const DEPTH: usize = 5 * LANES - 2;

pub enum Stimulus {
    Wide(Vec<Id>),
    Deep(Id),
    Cpu { reset: Id },
    Plugin { data: Id, write: Id, read: Id },
}

pub struct Fixture {
    pub project: Project,
    pub plugins: Registry,
    pub stimulus: Stimulus,
    pub probes: Vec<Id>,
    pub module_load_ms: Option<f64>,
    pub runtime: Option<(u32, logisim_plugin::WasmEngine)>,
}

fn part(kind: Kind, position: Point, bits: u8) -> Component {
    let mut c = Component::new(kind, position);
    c.bits = bits;
    c
}

fn wide(c: &mut Circuit) -> (Stimulus, Vec<Id>) {
    let mut inputs = Vec::new();
    let mut probes = Vec::new();
    for lane in 0..LANES {
        let adder = part(
            Kind::Arithmetic(Arithmetic::Adder),
            Point::new(100, 100 * lane as i32),
            32,
        );
        let ports = adder.ports();
        c.add_component(adder);
        inputs.push(c.add_component(part(Kind::Pin, ports[1], 32)));
        let mut offset = part(Kind::Constant, ports[2], 32);
        offset.constant = lane as u32 + 1;
        c.add_component(offset);
        let mut carry = part(Kind::Constant, ports[3], 1);
        carry.constant = 0;
        c.add_component(carry);
        probes.push(c.add_component(part(Kind::Probe, ports[0], 32)));
    }
    (Stimulus::Wide(inputs), probes)
}

fn deep(c: &mut Circuit) -> (Stimulus, Vec<Id>) {
    let mut gate = part(Kind::Gate(Gate::Not), Point::ZERO, 1);
    let spacing = -gate.ports()[1].x;
    let input = c.add_component(part(Kind::Pin, gate.ports()[1], 1));
    for i in 0..DEPTH {
        gate.position.x = i as i32 * spacing;
        c.add_component(gate.clone());
    }
    let probe = c.add_component(part(Kind::Probe, gate.ports()[0], 1));
    (Stimulus::Deep(input), vec![probe])
}

fn named(c: &Circuit, kind: Kind, label: &str) -> Result<Id> {
    c.components
        .iter()
        .find(|p| p.kind == kind && p.label == label)
        .map(|p| p.id)
        .ok_or_else(|| format!("Missing {} labeled {label}", kind.name()).into())
}

impl Fixture {
    #[cfg(test)]
    pub fn load(name: &str, root: &Path) -> Result<Self> {
        Self::load_with(name, root, Plugin::load)
    }

    pub fn load_with(
        name: &str,
        root: &Path,
        mut load: impl FnMut(&[u8]) -> std::result::Result<Plugin, String>,
    ) -> Result<Self> {
        let mut project = Project::default();
        let mut plugins = Registry::default();
        let mut module_load_ms = None;
        let mut runtime = None;
        let (stimulus, probes) = match name {
            "wide" => wide(&mut project.main_circuit_mut().circuit),
            "deep" => deep(&mut project.main_circuit_mut().circuit),
            "single-cycle" | "plugin" => {
                let path = root.join(if name == "plugin" {
                    "examples/agent/register-file.circ"
                } else {
                    "examples/course/single-cycle.circ"
                });
                #[cfg(feature = "plugin-metrics")]
                let read = logisim_plugin::metrics::timer("project_read");
                let xml = fs::read_to_string(&path)?;
                #[cfg(feature = "plugin-metrics")]
                drop(read);
                #[cfg(feature = "plugin-metrics")]
                let parse = logisim_plugin::metrics::timer("project_parse");
                let import = logisim_file::parse(&xml)?;
                #[cfg(feature = "plugin-metrics")]
                drop(parse);
                if import.partial || !import.warnings.is_empty() {
                    return Err(format!(
                        "Incomplete fixture {}: {:?}",
                        path.display(),
                        import.warnings
                    )
                    .into());
                }
                project = import.document;
                let start = Instant::now();
                for source in &project.plugin_sources {
                    let definitions = project
                        .circuits
                        .iter()
                        .flat_map(|d| &d.circuit.components)
                        .filter_map(|c| c.plugin.as_ref())
                        .filter(|p| p.plugin == source.plugin && p.version == source.version);
                    #[cfg(feature = "plugin-metrics")]
                    let read = logisim_plugin::metrics::timer("module_read");
                    let bytes = fs::read(path.parent().unwrap().join(&source.path))?;
                    #[cfg(feature = "plugin-metrics")]
                    drop(read);
                    #[cfg(feature = "plugin-metrics")]
                    let _span = logisim_plugin::metrics::timer("load_dependency");
                    let plugin = load(&bytes)?;
                    plugin.check_dependency(&source.plugin, &source.version, definitions)?;
                    runtime = Some((plugin.manifest().abi, plugin.engine()));
                    plugins.insert(plugin)?;
                }
                module_load_ms = Some(start.elapsed().as_secs_f64() * 1000.0);
                let c = &project.main_circuit().circuit;
                if name == "plugin" {
                    (
                        Stimulus::Plugin {
                            data: named(c, Kind::Pin, "W")?,
                            write: named(c, Kind::Pin, "Write address")?,
                            read: named(c, Kind::Pin, "Read address A")?,
                        },
                        vec![named(c, Kind::Probe, "Read A")?],
                    )
                } else {
                    let labels = [
                        "PC", "IR", "RegWrite", "RDin", "MemWrite", "MDin", "LedData",
                    ];
                    (
                        Stimulus::Cpu {
                            reset: named(c, Kind::Button, "复位")?,
                        },
                        labels
                            .iter()
                            .map(|l| named(c, Kind::Pin, l))
                            .collect::<Result<_>>()?,
                    )
                }
            }
            _ => return Err(format!("Unknown workload: {name}").into()),
        };
        Ok(Self {
            project,
            plugins,
            stimulus,
            probes,
            module_load_ms,
            runtime,
        })
    }
}

impl Stimulus {
    pub fn clocked(&self) -> bool {
        matches!(self, Self::Cpu { .. } | Self::Plugin { .. })
    }

    // v0.4.0 trace fingerprints. Review signal changes before rebaselining.
    pub fn expected_digest(&self) -> u64 {
        match self {
            Self::Wide(_) => 0xa817e833dd81da75,
            Self::Deep(_) => 0x17090361c775bb25,
            Self::Cpu { .. } => 0x73171884c0465aad,
            Self::Plugin { .. } => 0xa2689cffb70ff825,
        }
    }

    pub fn iterations(&self) -> usize {
        match self {
            Self::Cpu { .. } => 128,
            _ => 512,
        }
    }

    pub fn reset_inputs(&self, sim: &mut ProjectSimulator) -> Result<()> {
        match *self {
            Self::Cpu { reset } => {
                sim.set_button(&[], reset, true);
                sim.settle_compiled()?;
                sim.set_button(&[], reset, false);
            }
            Self::Plugin { write, read, .. } => {
                sim.set_input(write, 1);
                sim.set_input(read, 1);
            }
            _ => {}
        }
        sim.settle_compiled()?;
        Ok(())
    }

    pub fn drive(&self, sim: &mut ProjectSimulator, i: usize) {
        match self {
            Self::Wide(inputs) => sim.set_input(inputs[i % LANES], i as u32 + 1),
            Self::Deep(id) => sim.set_input(*id, (i as u32 + 1) & 1),
            Self::Plugin { data, .. } => sim.set_input(*data, 0x5a00_0000 + i as u32),
            Self::Cpu { .. } => {}
        }
    }
}
