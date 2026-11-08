//! End-to-end checks against the separately compiled Wasm, never the native handler.
//! Run with `--features host-check --bin course-plugin-check -- module.wasm [fixture.circ]`.
use logisim_core::{Circuit, Component, Id, Kind, Point, Project, plugin::api};
use logisim_plugin::Registry;
use logisim_sim::{Bit, ProjectSimulator, Simulator};
#[path = "check/scheduling.rs"]
mod scheduling_checks;
#[path = "check/stable.rs"]
mod stable_checks;

fn constant(circuit: &mut Circuit, at: Point, bits: u8, value: u32) {
    let mut c = Component::new(Kind::Constant, at);
    c.bits = bits;
    c.constant = value;
    circuit.add_component(c);
}
fn fixture(tools: &[api::Component]) -> (Project, Id, Id, Id) {
    let mut p = Project::default();
    let circuit = &mut p.main_circuit_mut().circuit;
    let rf = Component::from_plugin(tools[0].clone(), Point::new(320, 200));
    let ports = rf.ports();
    let rf = circuit.add_component(rf);
    constant(circuit, ports[2], 32, 42);
    let clock = circuit.add_component(Component::new(Kind::Pin, ports[3]));
    for (port, descriptor) in ports.iter().zip(&tools[0].tool.ports).skip(4) {
        constant(circuit, *port, descriptor.width, 1);
    }
    let probe = circuit.add_component(Component::from_plugin(
        tools[1].clone(),
        Point::new(500, 150),
    ));
    circuit.add_wire(ports[0], Point::new(500, 150));
    (p, rf, clock, probe)
}
fn output(sim: &Simulator, id: Id) -> Option<u32> {
    sim.port_value(id, 0).unwrap().number()
}
fn text(sim: &Simulator, id: Id, expected: &str) -> bool {
    sim.plugin_graphics(id)
        .unwrap()
        .iter()
        .any(|shape| matches!(shape, api::Shape::Text { text, .. } if text == expected))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().ok_or("Expected module.wasm [fixture.circ]")?;
    let bytes = std::fs::read(path)?;
    let mut registry = Registry::default();
    registry.load(&bytes)?;
    let tools = registry.manifests().next().unwrap().components();
    assert_eq!(tools[0].tool.id, "register-file");
    assert_eq!(tools[1].tool.id, "riscv-probe");
    if registry.manifests().next().unwrap().abi == api::LEGACY_ABI_VERSION {
        for tool in &tools {
            assert!(registry.instantiate_stable(tool)?.is_none());
        }
    } else {
        stable_checks::run(&registry, &tools)?;
        scheduling_checks::run(&registry, &tools);
    }
    let (mut project, rf, clock, probe) = fixture(&tools);
    let serialized = logisim_file::serialize(&project)?;
    if let Some(path) = args.next() {
        std::fs::write(path, &serialized)?;
    }
    let imported = logisim_file::parse(&serialized)?;
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    assert_eq!(logisim_file::serialize(&imported.document)?, serialized);
    let mut sim = Simulator::default();
    sim.synchronize(&project.main_circuit().circuit);
    sim.advance(64);
    assert_eq!(sim.plugin_errors().count(), 2);
    assert_eq!(sim.port_value(rf, 0).unwrap().bit(0), Bit::Error);
    sim.set_plugins(registry.clone());
    sim.advance(64);
    assert_eq!(sim.plugin_errors().count(), 0);
    assert_eq!(output(&sim, rf), Some(0));
    assert_eq!(sim.plugin_graphics(rf).unwrap().len(), 32);
    assert!(!sim.plugin_graphics(probe).unwrap().is_empty());
    sim.set_input(clock, 1);
    sim.advance(64);
    assert_eq!(output(&sim, rf), Some(42));
    assert!(text(&sim, rf, "0000002A"));
    assert_eq!(logisim_file::serialize(&project)?, serialized);
    // Runtime action and purely geometric edit must not reset the instance.
    sim.plugin_action(rf, "write".into(), "1 DEADBEEF".into());
    sim.advance(64);
    assert_eq!(output(&sim, rf), Some(0xDEADBEEF));
    project
        .main_circuit_mut()
        .circuit
        .add_component(Component::new(Kind::Text, Point::new(600, 300)));
    sim.synchronize(&project.main_circuit().circuit);
    sim.advance(64);
    assert_eq!(output(&sim, rf), Some(0xDEADBEEF));
    // Invalid actions fail visibly; Reset constructs a clean VM and clears state.
    sim.plugin_action(rf, "write".into(), "0 FF".into());
    sim.advance(64);
    assert_eq!(sim.plugin_errors().count(), 1);
    assert_eq!(sim.port_value(rf, 0).unwrap().bit(0), Bit::Error);
    sim.reset();
    sim.advance(64);
    assert_eq!(sim.plugin_errors().count(), 0);
    assert_eq!(output(&sim, rf), Some(0));
    sim.plugin_action(rf, "write".into(), "1 ABC".into());
    sim.advance(64);
    // Editing properties initializes a new instance with the new schema values.
    project
        .main_circuit_mut()
        .circuit
        .components
        .iter_mut()
        .find(|c| c.id == rf)
        .unwrap()
        .plugin
        .as_mut()
        .unwrap()
        .properties
        .insert("trigger".into(), "falling".into());
    sim.synchronize(&project.main_circuit().circuit);
    sim.advance(64);
    assert_eq!(output(&sim, rf), Some(0));
    assert!(!sim.limited());
    sim.set_plugins(Registry::default());
    sim.advance(64);
    assert_eq!(sim.plugin_errors().count(), 2);
    assert_eq!(sim.port_value(rf, 0).unwrap().bit(0), Bit::Error);
    sim.set_plugins(registry.clone());
    sim.advance(64);
    assert_eq!(sim.plugin_errors().count(), 0);
    hierarchy(&registry, &tools);
    same_edge(&registry, &tools);
    println!(
        "Wasm integration: loading, graphics, clocks, actions, faults/reset, properties, file roundtrip, hierarchical isolation and same-edge sampling passed"
    );
    Ok(())
}
fn hierarchy(registry: &Registry, tools: &[api::Component]) {
    let (child, rf, _, _) = fixture(tools);
    let mut project = Project::default();
    let definition = project.add("plugin-child".into()).unwrap();
    let mut circuit = child.main_circuit().circuit.clone();
    // Internal constant clock, without a Pin exposed to the parent.
    let clock = circuit
        .components
        .iter_mut()
        .find(|c| c.kind == Kind::Pin)
        .unwrap();
    *clock = Component {
        id: clock.id,
        ..Component::new(Kind::Constant, clock.position)
    };
    project.circuit_mut(definition).unwrap().circuit = circuit;
    let mut ids = vec![];
    for x in [100, 300] {
        let c = project.instance(definition, Point::new(x, 100)).unwrap();
        ids.push(project.main_circuit_mut().circuit.add_component(c));
    }
    let mut sim = ProjectSimulator::default();
    sim.set_plugins(registry.clone());
    sim.synchronize(&project, project.main);
    sim.advance(64);
    sim.plugin_action(&[ids[0]], rf, "write".into(), "1 12345678".into());
    sim.advance(64);
    assert_eq!(output(sim.view(&[ids[0]]).unwrap(), rf), Some(0x12345678));
    assert_eq!(output(sim.view(&[ids[1]]).unwrap(), rf), Some(0));
    sim.reset();
    sim.advance(64);
    assert_eq!(output(sim.view(&[ids[0]]).unwrap(), rf), Some(0));
    assert!(sim.plugin_failures().is_empty());
}
fn same_edge(registry: &Registry, tools: &[api::Component]) {
    let (mut p, first, clock, _) = fixture(tools);
    let c = &mut p.main_circuit_mut().circuit;
    let second = Component::from_plugin(tools[0].clone(), Point::new(800, 200));
    let ports = second.ports();
    let second = c.add_component(second);
    let a = c.component(first).unwrap().ports();
    c.add_wire(a[0], Point::new(ports[2].x, a[0].y));
    c.add_wire(Point::new(ports[2].x, a[0].y), ports[2]);
    c.add_wire(a[3], ports[3]);
    for (port, descriptor) in ports.iter().zip(&tools[0].tool.ports).skip(4) {
        constant(c, *port, descriptor.width, 1);
    }
    let mut sim = Simulator::default();
    sim.set_plugins(registry.clone());
    sim.synchronize(c);
    sim.advance(64);
    sim.set_input(clock, 1);
    sim.advance(64);
    assert_eq!(output(&sim, first), Some(42));
    assert_eq!(output(&sim, second), Some(0));
    sim.set_input(clock, 0);
    sim.advance(64);
    sim.set_input(clock, 1);
    sim.advance(64);
    assert_eq!(output(&sim, second), Some(42));
    assert_eq!(sim.plugin_errors().count(), 0);
}
