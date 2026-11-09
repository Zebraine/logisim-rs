use logisim_core::{Circuit, Component, Facing, Id, Kind, Point, Project, Value, plugin::api};
use logisim_plugin::Registry;
use logisim_sim::{ProjectSimulator, Simulator};
use logisim_ttl_sequential_plugin::{
    model::{Chip, INPUTS},
    validation::inputs,
};
fn definition(registry: &Registry, chip: Chip) -> api::Component {
    registry
        .manifests()
        .find(|m| m.id == "org.logisim-rs.ttl-sequential")
        .unwrap()
        .components()
        .into_iter()
        .find(|c| c.tool.id == chip.id())
        .unwrap()
}
fn settle(sim: &mut Simulator, stable: bool) {
    if stable {
        sim.settle_compiled().unwrap();
    } else {
        sim.advance(128);
    }
    assert!(!sim.limited());
    assert_eq!(sim.pending_count(), 0);
    assert_eq!(sim.plugin_errors().count(), 0);
}
fn constant(c: &mut Circuit, at: Point, v: u32) {
    let mut x = Component::new(Kind::Constant, at);
    x.constant = v;
    c.add_component(x);
}
fn tunnel(c: &mut Circuit, at: Point, label: &str) {
    let mut t = Component::new(Kind::Tunnel, at);
    t.label = label.into();
    c.add_component(t);
}
pub fn run(registry: &Registry) {
    for power in [false, true] {
        for facing in Facing::ALL {
            for stable in [false, true] {
                let chip = Chip { power };
                let mut c =
                    Component::from_plugin(definition(registry, chip), Point::new(300, 300));
                c.facing = facing;
                let mut circuit = Circuit::default();
                c.id = circuit.add_component(c.clone());
                let drivers: Vec<_> = c
                    .ports()
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !c.drives_port(*i))
                    .map(|(i, p)| (i, circuit.add_component(Component::new(Kind::Pin, *p))))
                    .collect();
                let mut sim = Simulator::default();
                sim.set_plugins(registry.clone());
                sim.synchronize(&circuit);
                for (row, expected) in [
                    ("01101001000010", 0),
                    ("01101001001010", 1),
                    ("01101001010010", 1),
                    ("01101001011010", 0),
                ] {
                    let values = inputs(chip, row);
                    for (i, id) in &drivers {
                        sim.set_input(*id, Value::from(values[*i]).number().unwrap());
                    }
                    settle(&mut sim, stable);
                    assert_eq!(
                        sim.port_value(c.id, chip.index(9)),
                        Some(Value::known(1, expected))
                    );
                }
                c.plugin
                    .as_mut()
                    .unwrap()
                    .properties
                    .insert("internal".into(), "true".into());
                circuit.replace_component(c.clone());
                sim.synchronize(&circuit);
                settle(&mut sim, stable);
                assert!(
                    sim.plugin_graphics(c.id)
                        .unwrap()
                        .iter()
                        .any(|g| matches!(g,api::Shape::Text{text,..} if text=="11010010"))
                );
                c.plugin
                    .as_mut()
                    .unwrap()
                    .properties
                    .insert("internal".into(), "false".into());
                circuit.replace_component(c.clone());
                sim.synchronize(&circuit);
                settle(&mut sim, stable);
                assert!(sim.plugin_graphics(c.id).unwrap().is_empty());
                let clock = drivers.iter().find(|(i, _)| *i == chip.index(2)).unwrap().1;
                sim.set_input(clock, 0);
                settle(&mut sim, stable);
                c.plugin
                    .as_mut()
                    .unwrap()
                    .properties
                    .insert("internal".into(), "true".into());
                circuit.replace_component(c.clone());
                sim.synchronize(&circuit);
                settle(&mut sim, stable);
                assert!(
                    sim.plugin_graphics(c.id)
                        .unwrap()
                        .iter()
                        .any(|g| matches!(g,api::Shape::Text{text,..} if text=="11010010"))
                );
                sim.set_input(clock, 1);
                settle(&mut sim, stable);
                assert!(
                    sim.plugin_graphics(c.id)
                        .unwrap()
                        .iter()
                        .any(|g| matches!(g,api::Shape::Text{text,..} if text=="10100100"))
                );
                let p = c.ports()[chip.index(9)];
                let q = p + facing.transform(Point::new(0, -40));
                circuit.add_wire(p, q);
                circuit.normalize_wires();
                let external = circuit.add_component(Component::new(Kind::Pin, q));
                sim.synchronize(&circuit);
                sim.set_input(external, 1);
                settle(&mut sim, stable);
                assert_eq!(
                    sim.port_value(c.id, chip.index(9)).unwrap().bit(0),
                    logisim_core::Bit::Error
                );
                if power {
                    let vcc = drivers
                        .iter()
                        .find(|(i, _)| *i == chip.index(16))
                        .unwrap()
                        .1;
                    sim.set_input(vcc, 0);
                    settle(&mut sim, stable);
                    assert_eq!(
                        sim.port_value(c.id, chip.index(9)),
                        Some(Value::known(1, 1))
                    );
                    sim.set_input(vcc, 1);
                    settle(&mut sim, stable);
                    assert_eq!(
                        sim.port_value(c.id, chip.index(9)).unwrap().bit(0),
                        logisim_core::Bit::Error
                    );
                }
                let wires = circuit.wires.clone();
                let mut rotated = c.clone();
                rotated.facing =
                    Facing::ALL[(Facing::ALL.iter().position(|f| *f == facing).unwrap() + 1) % 4];
                circuit.replace_component(rotated);
                assert_eq!(circuit.wires, wires);
                circuit.replace_component(c.clone());
                let encoded = logisim_file::serialize(&Project::single("main", circuit)).unwrap();
                let parsed = logisim_file::parse(&encoded).unwrap();
                assert!(parsed.warnings.is_empty());
                assert_eq!(logisim_file::serialize(&parsed.document).unwrap(), encoded);
                sim.set_plugins(Registry::default());
                sim.advance(128);
                assert_eq!(sim.plugin_errors().count(), 1);
                assert_eq!(
                    sim.port_value(c.id, chip.index(9)).unwrap().bit(0),
                    logisim_core::Bit::Error
                );
            }
        }
    }
    for reverse in [false, true] {
        for stable in [false, true] {
            chain(registry, reverse, stable);
            hierarchy(registry, reverse, stable);
        }
    }
}
fn shift(c: &mut Circuit, registry: &Registry, at: Point) -> Component {
    let chip = Chip { power: false };
    let mut x = Component::from_plugin(definition(registry, chip), at);
    x.id = c.add_component(x.clone());
    for pin in INPUTS[..12].iter().filter(|p| ![2, 10].contains(p)) {
        constant(c, x.ports()[chip.index(*pin)], u32::from(*pin == 1));
    }
    x
}
fn chain(registry: &Registry, reverse: bool, stable: bool) {
    let chip = Chip { power: false };
    let mut c = Circuit::default();
    let a = shift(&mut c, registry, Point::new(300, 200));
    let b = shift(&mut c, registry, Point::new(700, 200));
    let data = c.add_component(Component::new(Kind::Pin, a.ports()[chip.index(10)]));
    let clock = c.add_component(Component::new(Kind::Pin, Point::new(100, 100)));
    for at in [
        Point::new(100, 100),
        a.ports()[chip.index(2)],
        b.ports()[chip.index(2)],
    ] {
        tunnel(&mut c, at, "clock");
    }
    for at in [a.ports()[chip.index(9)], b.ports()[chip.index(10)]] {
        tunnel(&mut c, at, "serial");
    }
    if reverse {
        c.components.reverse();
    }
    let mut sim = Simulator::default();
    sim.set_plugins(registry.clone());
    sim.synchronize(&c);
    settle(&mut sim, stable);
    for (value, expected) in [(1, [1, 0]), (0, [0, 1]), (1, [1, 0])] {
        sim.set_input(clock, 0);
        settle(&mut sim, stable);
        sim.set_input(data, value);
        settle(&mut sim, stable);
        sim.set_input(clock, 1);
        settle(&mut sim, stable);
        for (id, q) in [a.id, b.id].into_iter().zip(expected) {
            assert_eq!(sim.port_value(id, chip.index(9)), Some(Value::known(1, q)));
        }
    }
    // History-only commits remain usable when switching execution mode.
    sim.set_input(clock, 0);
    settle(&mut sim, !stable);
    sim.set_input(data, 0);
    settle(&mut sim, !stable);
    sim.set_input(clock, 1);
    settle(&mut sim, !stable);
    assert_eq!(
        sim.port_value(b.id, chip.index(9)),
        Some(Value::known(1, 1))
    );
    sim.reset();
    settle(&mut sim, stable);
    assert_eq!(
        sim.port_value(a.id, chip.index(9)),
        Some(Value::known(1, 0))
    );
}
fn hierarchy(registry: &Registry, reverse: bool, stable: bool) {
    let chip = Chip { power: false };
    let mut child = Circuit::default();
    let x = shift(&mut child, registry, Point::new(300, 200));
    let input = child.add_component(Component::new(Kind::Pin, x.ports()[chip.index(10)]));
    let clock = child.add_component(Component::new(Kind::Pin, x.ports()[chip.index(2)]));
    let mut output = Component::new(Kind::Pin, x.ports()[chip.index(9)]);
    output.output = true;
    let output = child.add_component(output);
    let mut project = Project::default();
    let def = project.add("serial".into()).unwrap();
    project.circuit_mut(def).unwrap().circuit = child;
    let ids: Vec<Id> = [200, 400]
        .into_iter()
        .map(|y| {
            project
                .main_circuit_mut()
                .circuit
                .add_component(Component::new(Kind::Subcircuit(def), Point::new(500, y)))
        })
        .collect();
    project.refresh_interfaces(false);
    let driver = project
        .main_circuit_mut()
        .circuit
        .add_component(Component::new(Kind::Pin, Point::new(100, 100)));
    tunnel(
        &mut project.main_circuit_mut().circuit,
        Point::new(100, 100),
        "clock",
    );
    for (i, id) in ids.iter().enumerate() {
        let x = project
            .main_circuit()
            .circuit
            .component(*id)
            .unwrap()
            .clone();
        let at = |pin| x.ports()[x.interface.ports.iter().position(|p| p.pin == pin).unwrap()];
        constant(&mut project.main_circuit_mut().circuit, at(input), i as u32);
        tunnel(&mut project.main_circuit_mut().circuit, at(clock), "clock");
    }
    if reverse {
        project.main_circuit_mut().circuit.components.reverse();
    }
    let mut sim = ProjectSimulator::default();
    sim.set_plugins(registry.clone());
    sim.synchronize(&project, project.main);
    let settle = |sim: &mut ProjectSimulator| {
        if stable {
            sim.settle_compiled().unwrap();
        } else {
            sim.advance(128);
        }
        assert!(sim.error().is_none());
    };
    settle(&mut sim);
    sim.set_input(driver, 1);
    settle(&mut sim);
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(
            sim.view(&[*id]).unwrap().port_value(output, 0),
            Some(Value::known(1, i as u32))
        );
    }
}
