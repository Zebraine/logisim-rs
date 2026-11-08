use super::*;
use logisim_core::{Gate, Value};
use logisim_sim::stable::{Failure, Report};

type Port = (Id, usize);
fn add(c: &mut Circuit, kind: Kind, bits: u8) -> Id {
    let mut value = Component::new(kind, Point::new(200 + c.components.len() as i32 * 300, 600));
    value.bits = bits;
    c.add_component(value)
}
fn link(c: &mut Circuit, a: Port, b: Port) {
    for port in [a, b] {
        let component = c.component(port.0).unwrap();
        let mut tunnel = Component::new(Kind::Tunnel, component.ports()[port.1]);
        tunnel.bits = component.port_width(port.1).unwrap();
        tunnel.label = format!("{}:{}", a.0, a.1);
        c.add_component(tunnel);
    }
}
fn pin(c: &mut Circuit, target: Port) -> Id {
    let id = add(
        c,
        Kind::Pin,
        c.component(target.0).unwrap().port_width(target.1).unwrap(),
    );
    link(c, (id, 0), target);
    id
}
fn register_file(c: &mut Circuit, tool: &api::Component) -> Id {
    let id = c.add_component(Component::from_plugin(
        tool.clone(),
        Point::new(300 + c.components.len() as i32 * 300, 200),
    ));
    let ports = c.component(id).unwrap().ports();
    for &i in &[4, 5, 6, 7] {
        constant(c, ports[i], tool.tool.ports[i].width, 1);
    }
    id
}
fn idle(report: Report) {
    assert_eq!(report.evaluated_plugins, 0);
    assert_eq!(report.sampled_plugins, 0);
    assert_eq!(report.sampled_components, 0);
}
pub fn run(registry: &Registry, tools: &[api::Component]) {
    for reverse in [false, true] {
        mixed_chain(registry, tools, reverse);
        siblings(registry, tools, reverse);
    }
    invalidation(registry, tools);
    println!(
        "Wasm stable scheduling: mixed same-edge chain, hierarchy, dependency caches, actions, properties, registry changes, reset and event stepping passed"
    );
}
fn mixed_chain(registry: &Registry, tools: &[api::Component], reverse: bool) {
    let mut c = Circuit::default();
    let first = register_file(&mut c, &tools[0]);
    let second = register_file(&mut c, &tools[0]);
    let reg = add(&mut c, Kind::Register, 32);
    let data = pin(&mut c, (first, 2));
    let clk = pin(&mut c, (first, 3));
    link(&mut c, (clk, 0), (second, 3));
    link(&mut c, (clk, 0), (reg, 2));
    let mut previous = first;
    for _ in 0..8 {
        let buffer = add(&mut c, Kind::Gate(Gate::Buffer), 32);
        link(&mut c, (previous, 0), (buffer, 1));
        previous = buffer;
    }
    link(&mut c, (previous, 0), (second, 2));
    link(&mut c, (second, 0), (reg, 1));
    let ports = c.component(reg).unwrap().ports();
    for (i, value) in [(3, 1), (4, 0), (5, 0), (6, 0)] {
        constant(&mut c, ports[i], 1, value);
    }
    if reverse {
        c.components.reverse();
    }
    let mut sim = Simulator::default();
    sim.set_plugins(registry.clone());
    sim.synchronize(&c);
    sim.settle_compiled().unwrap();
    idle(sim.settle_compiled().unwrap());
    sim.set_input(data, 42);
    let report = sim.settle_compiled().unwrap();
    assert_eq!(report.evaluated_plugins, 0); // write data cannot affect reads before sampling
    assert_eq!(report.sampled_plugins, 1);
    for (expected_second, expected_reg) in [(0, 0), (42, 0), (42, 42)] {
        sim.set_input(clk, 1);
        sim.settle_compiled().unwrap();
        assert_eq!(output(&sim, first), Some(42));
        assert_eq!(output(&sim, second), Some(expected_second));
        assert_eq!(sim.stored_value(reg), Some(Value::known(32, expected_reg)));
        idle(sim.settle_compiled().unwrap());
        sim.set_input(clk, 0);
        sim.settle_compiled().unwrap();
    }
    // Resume event stepping, then re-enter stable mode after a real event edge.
    sim.set_input(data, 17);
    sim.advance(128);
    sim.set_input(clk, 1);
    sim.advance(128);
    assert_eq!(output(&sim, first), Some(17));
    sim.settle_compiled().unwrap();
    assert_eq!(output(&sim, second), Some(42));
    sim.reset();
    sim.settle_compiled().unwrap();
    assert_eq!(output(&sim, first), Some(0));
    assert_eq!(output(&sim, second), Some(0));
    assert_eq!(sim.plugin_errors().count(), 0);
}
fn siblings(registry: &Registry, tools: &[api::Component], reverse: bool) {
    let mut p = Project::default();
    let child = p.add("stateful-plugin".into()).unwrap();
    let c = &mut p.circuit_mut(child).unwrap().circuit;
    let rf = register_file(c, &tools[0]);
    let data = pin(c, (rf, 2));
    let clock = pin(c, (rf, 3));
    let out = add(c, Kind::Pin, 32);
    c.components
        .iter_mut()
        .find(|c| c.id == out)
        .unwrap()
        .output = true;
    link(c, (rf, 0), (out, 0));
    let mut instances = vec![];
    let mut interface = vec![];
    for x in [300, 900] {
        let value = p.instance(child, Point::new(x, 1000)).unwrap();
        interface = value.interface.ports.clone();
        instances.push(p.main_circuit_mut().circuit.add_component(value));
    }
    let port = |id| interface.iter().position(|p| p.pin == id).unwrap();
    let c = &mut p.main_circuit_mut().circuit;
    let clk = pin(c, (instances[0], port(clock)));
    link(c, (clk, 0), (instances[1], port(clock)));
    let d = pin(c, (instances[0], port(data)));
    link(c, (instances[0], port(out)), (instances[1], port(data)));
    if reverse {
        c.components.reverse();
        p.circuit_mut(child).unwrap().circuit.components.reverse();
    }
    let mut sim = ProjectSimulator::default();
    sim.set_plugins(registry.clone());
    sim.synchronize(&p, p.main);
    sim.settle_compiled().unwrap();
    sim.set_input(d, 42);
    for expected in [0, 42] {
        sim.set_input(clk, 1);
        sim.settle_compiled().unwrap();
        assert_eq!(output(sim.view(&[instances[0]]).unwrap(), rf), Some(42));
        assert_eq!(
            output(sim.view(&[instances[1]]).unwrap(), rf),
            Some(expected)
        );
        idle(sim.settle_compiled().unwrap());
        sim.set_input(clk, 0);
        sim.settle_compiled().unwrap();
    }
    sim.plugin_action(&[instances[0]], rf, "write".into(), "1 ABC".into());
    sim.settle_compiled().unwrap();
    assert_eq!(output(sim.view(&[instances[0]]).unwrap(), rf), Some(0xabc));
    assert_eq!(output(sim.view(&[instances[1]]).unwrap(), rf), Some(42));
    sim.set_plugins(Registry::default());
    assert!(sim.settle_compiled().is_err());
    sim.set_plugins(registry.clone());
    sim.settle_compiled().unwrap();
    assert_eq!(output(sim.view(&[instances[0]]).unwrap(), rf), Some(0));
    sim.reset();
    sim.settle_compiled().unwrap();
    idle(sim.settle_compiled().unwrap());
}
fn invalidation(registry: &Registry, tools: &[api::Component]) {
    let (mut p, rf, clk, _) = fixture(tools);
    let c = &mut p.main_circuit_mut().circuit;
    let mut sim = Simulator::default();
    sim.set_plugins(registry.clone());
    sim.synchronize(c);
    sim.settle_compiled().unwrap();
    sim.set_input(clk, 1);
    sim.settle_compiled().unwrap();
    sim.plugin_action(rf, "write".into(), "1 ABC".into());
    sim.settle_compiled().unwrap();
    assert_eq!(output(&sim, rf), Some(0xabc));
    // Visual reindexing retains state and cached compiled plugin metadata by ID.
    c.add_component(Component::new(Kind::Text, Point::new(2000, 2000)));
    let text = c.components.pop().unwrap();
    c.components.insert(0, text);
    assert!(!sim.synchronize(c));
    sim.settle_compiled().unwrap();
    assert_eq!(output(&sim, rf), Some(0xabc));
    // Electrical edits also retain this instance's words while rematerializing nets.
    let at = c.component(rf).unwrap().ports()[0];
    let probe = Component::new(Kind::Probe, at);
    c.add_component(probe);
    assert!(sim.synchronize(c));
    sim.settle_compiled().unwrap();
    assert_eq!(output(&sim, rf), Some(0xabc));
    for trigger in ["high", "falling"] {
        c.components
            .iter_mut()
            .find(|c| c.id == rf)
            .unwrap()
            .plugin
            .as_mut()
            .unwrap()
            .properties
            .insert("trigger".into(), trigger.into());
        sim.synchronize(c);
        if trigger == "high" {
            let before = (sim.steps(), sim.pending_count());
            assert_eq!(
                sim.settle_compiled().unwrap_err().reason,
                Failure::UnsupportedComponent(rf)
            );
            assert_eq!(before, (sim.steps(), sim.pending_count()));
        } else {
            sim.settle_compiled().unwrap();
            assert_eq!(output(&sim, rf), Some(0));
        }
    }
    sim.set_plugins(Registry::default());
    assert!(matches!(
        sim.settle_compiled().unwrap_err().reason,
        Failure::Plugin { .. }
    ));
    sim.set_plugins(registry.clone());
    sim.settle_compiled().unwrap();
    idle(sim.settle_compiled().unwrap());
}
