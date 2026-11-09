use logisim_core::{Bit, Circuit, Component, Facing, Id, Kind, Point, Project, Value, plugin::api};
use logisim_plugin::Registry;
use logisim_sim::{ProjectSimulator, Simulator};
use logisim_ttl_combinational_plugin::{
    model::{Chip, Kind as ChipKind},
    validation::{decode, signals},
};
fn definition(registry: &Registry, chip: Chip) -> api::Component {
    registry
        .manifests()
        .find(|m| m.id == "org.logisim-rs.ttl-combinational")
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
    assert_eq!(sim.plugin_errors().count(), 0);
}
pub fn run(registry: &Registry) {
    for kind in ChipKind::ALL {
        for power in [false, true] {
            for facing in Facing::ALL {
                for stable in [false, true] {
                    let chip = Chip { kind, power };
                    let (values, expected) = match kind {
                        ChipKind::Decoder7447 => ("1010111", "1011011"),
                        ChipKind::Comparator7485 => ("01011001010", "100"),
                        ChipKind::Buffer74125 => ("01010101", "1111"),
                        ChipKind::Adder74283 => ("111110001", "10001"),
                    };
                    let mut c =
                        Component::from_plugin(definition(registry, chip), Point::new(300, 300));
                    c.facing = facing;
                    let mut circuit = Circuit::default();
                    c.id = circuit.add_component(c.clone());
                    let inputs = signals(chip, values);
                    let mut drivers = vec![];
                    for (i, port) in c.ports().iter().enumerate() {
                        if !c.drives_port(i) {
                            drivers
                                .push((i, circuit.add_component(Component::new(Kind::Pin, *port))));
                        }
                    }
                    let mut sim = Simulator::default();
                    sim.set_plugins(registry.clone());
                    sim.synchronize(&circuit);
                    for (i, id) in &drivers {
                        sim.set_input(*id, Value::from(inputs[*i]).number().unwrap());
                    }
                    settle(&mut sim, stable);
                    for (pin, value) in kind.outputs().iter().zip(expected.chars()) {
                        assert_eq!(
                            sim.port_value(c.id, chip.index(*pin)),
                            Some(Value::filled(1, decode(value)))
                        );
                    }
                    // Display edits and geometry changes retain original wire coordinates.
                    let p = c.ports()[chip.index(kind.outputs()[0])];
                    circuit.add_wire(
                        p,
                        p + facing.transform(Point::new(
                            0,
                            kind.position(kind.outputs()[0])[1].signum() * 40,
                        )),
                    );
                    circuit.normalize_wires();
                    let wires = circuit.wires.clone();
                    c.plugin
                        .as_mut()
                        .unwrap()
                        .properties
                        .insert("internal".into(), "true".into());
                    circuit.replace_component(c.clone());
                    assert_eq!(circuit.wires, wires);
                    sim.synchronize(&circuit);
                    settle(&mut sim, stable);
                    assert!(!sim.plugin_graphics(c.id).unwrap().is_empty());
                    let external = circuit.add_component(Component::new(Kind::Pin, p));
                    sim.synchronize(&circuit);
                    sim.set_input(external, u32::from(expected.starts_with('0')));
                    settle(&mut sim, stable);
                    assert_eq!(
                        sim.port_value(c.id, chip.index(kind.outputs()[0]))
                            .unwrap()
                            .bit(0),
                        Bit::Error
                    );
                    if power {
                        let vcc = drivers
                            .iter()
                            .find(|(i, _)| *i == chip.index(kind.count()))
                            .unwrap()
                            .1;
                        sim.set_input(vcc, 0);
                        settle(&mut sim, stable);
                        assert_eq!(
                            sim.port_value(c.id, chip.index(kind.outputs()[0]))
                                .unwrap()
                                .number(),
                            Some(u32::from(expected.starts_with('0')))
                        );
                    } else if kind == ChipKind::Buffer74125 {
                        let enable = drivers.iter().find(|(i, _)| *i == chip.index(1)).unwrap().1;
                        sim.set_input(enable, 1);
                        settle(&mut sim, stable);
                        assert_eq!(
                            sim.port_value(c.id, chip.index(3)).unwrap().number(),
                            Some(0)
                        );
                    }
                    let project = Project::single("main", circuit);
                    let encoded = logisim_file::serialize(&project).unwrap();
                    let parsed = logisim_file::parse(&encoded).unwrap();
                    assert!(parsed.warnings.is_empty());
                    assert_eq!(logisim_file::serialize(&parsed.document).unwrap(), encoded);
                    sim.set_plugins(Registry::default());
                    sim.advance(128);
                    assert_eq!(sim.plugin_errors().count(), 1);
                    for pin in kind.outputs() {
                        assert_eq!(
                            sim.port_value(c.id, chip.index(*pin)).unwrap().bit(0),
                            Bit::Error
                        );
                    }
                }
            }
        }
    }
    hierarchy(registry);
}
fn hierarchy(registry: &Registry) {
    let chip = Chip {
        kind: ChipKind::Buffer74125,
        power: false,
    };
    let mut c = Component::from_plugin(definition(registry, chip), Point::new(100, 100));
    let mut child = Circuit::default();
    c.id = child.add_component(c.clone());
    let input = child.add_component(Component::new(Kind::Pin, c.ports()[chip.index(2)]));
    let mut out = Component::new(Kind::Pin, c.ports()[chip.index(3)]);
    out.output = true;
    let output = child.add_component(out);
    for pin in [1, 4, 5, 10, 9, 13, 12] {
        let mut constant = Component::new(Kind::Constant, c.ports()[chip.index(pin)]);
        constant.constant = 0;
        child.add_component(constant);
    }
    let mut project = Project::default();
    let def = project.add("buffer".into()).unwrap();
    project.circuit_mut(def).unwrap().circuit = child;
    let mut instances: Vec<Id> = vec![];
    for y in [100, 300] {
        instances.push(
            project
                .main_circuit_mut()
                .circuit
                .add_component(Component::new(Kind::Subcircuit(def), Point::new(400, y))),
        );
    }
    project.refresh_interfaces(false);
    for (i, id) in instances.iter().enumerate() {
        let c = project.main_circuit().circuit.component(*id).unwrap();
        let at = c.ports()[c
            .interface
            .ports
            .iter()
            .position(|p| p.pin == input)
            .unwrap()];
        let mut constant = Component::new(Kind::Constant, at);
        constant.constant = i as u32;
        project.main_circuit_mut().circuit.add_component(constant);
    }
    for stable in [false, true] {
        let mut sim = ProjectSimulator::default();
        sim.set_plugins(registry.clone());
        sim.synchronize(&project, project.main);
        if stable {
            sim.settle_compiled().unwrap();
        } else {
            sim.advance(128);
        }
        assert!(sim.error().is_none());
        for (i, id) in instances.iter().enumerate() {
            assert_eq!(
                sim.view(&[*id]).unwrap().port_value(output, 0),
                Some(Value::known(1, i as u32))
            );
        }
    }
}
