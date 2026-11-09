//! Actual-Wasm network regressions retained from the native TTL implementation.
use logisim_core::plugin::api;
use logisim_core::{Bit, Circuit, Component, Facing, Id, Kind, Point, Value};
use logisim_plugin::Registry;
use logisim_sim::Simulator;
use logisim_ttl_plugin::model::{Chip, Ttl};
pub fn run(registry: &Registry) {
    bus_and_edits(registry);
    hierarchy(registry);
}
fn definition(registry: &Registry, chip: Chip) -> api::Component {
    registry
        .manifests()
        .next()
        .unwrap()
        .components()
        .into_iter()
        .find(|c| c.tool.id == chip.id())
        .unwrap()
}
fn index(c: &Component, pin: u8) -> usize {
    c.plugin
        .as_ref()
        .unwrap()
        .tool
        .ports
        .iter()
        .position(|p| p.id == format!("pin-{pin}"))
        .unwrap()
}
fn setup(
    registry: &Registry,
    kind: Ttl,
    facing: Facing,
    power: bool,
) -> (Circuit, Component, Vec<(u8, Id)>, Simulator) {
    let mut c = Component::from_plugin(
        definition(registry, Chip { kind, power }),
        Point::new(300, 300),
    );
    c.facing = facing;
    let mut circuit = Circuit::default();
    let mut inputs = vec![];
    for (i, p) in c.ports().into_iter().enumerate() {
        if !c.drives_port(i) {
            inputs.push((
                Chip { kind, power }.pins()[i],
                circuit.add_component(Component::new(Kind::Pin, p)),
            ));
        }
    }
    c.id = circuit.add_component(c.clone());
    let mut sim = Simulator::default();
    sim.set_plugins(registry.clone());
    sim.synchronize(&circuit);
    (circuit, c, inputs, sim)
}
fn set(sim: &mut Simulator, inputs: &[(u8, Id)], pin: u8, v: Bit) {
    sim.set_input(
        inputs.iter().find(|p| p.0 == pin).unwrap().1,
        Value::filled(1, v).number().unwrap(),
    );
}
fn settle(sim: &mut Simulator) {
    sim.advance(128);
    assert_eq!(sim.pending_count(), 0);
    assert!(!sim.limited());
}
fn bit(s: &Simulator, c: &Component, pin: u8) -> Bit {
    s.port_value(c.id, index(c, pin)).unwrap().bit(0)
}
fn decode(c: char) -> Bit {
    match c {
        '0' => Bit::Zero,
        '1' => Bit::One,
        'X' => Bit::Unknown,
        _ => Bit::Error,
    }
}
fn bus_and_edits(registry: &Registry) {
    for kind in Ttl::ALL {
        let (mut circuit, mut c, inputs, mut sim) = setup(registry, kind, Facing::East, true);
        let expected = match kind {
            Ttl::Ic7408 | Ttl::Ic7432 | Ttl::Ic7486 => Bit::Zero,
            _ => Bit::One,
        };
        for vcc in "01".chars() {
            for gnd in "01".chars() {
                set(&mut sim, &inputs, 14, decode(vcc));
                set(&mut sim, &inputs, 7, decode(gnd));
                settle(&mut sim);
                for &[_, _, o] in kind.gates() {
                    assert_eq!(
                        bit(&sim, &c, o),
                        if vcc == '1' && gnd == '0' {
                            expected
                        } else {
                            Bit::Unknown
                        }
                    );
                }
            }
        }
        let output = kind.gates()[0][2];
        let ext = circuit.add_component(Component::new(Kind::Pin, c.ports()[index(&c, output)]));
        sim.synchronize(&circuit);
        for powered in [false, true] {
            for forced in [Bit::Zero, Bit::One] {
                set(
                    &mut sim,
                    &inputs,
                    14,
                    if powered { Bit::One } else { Bit::Zero },
                );
                set(&mut sim, &inputs, 7, Bit::Zero);
                sim.set_input(ext, Value::filled(1, forced).number().unwrap());
                settle(&mut sim);
                assert_eq!(
                    bit(&sim, &c, output),
                    if powered && forced != expected {
                        Bit::Error
                    } else {
                        forced
                    }
                );
            }
        }
        // Display edits leave inputs intact; disabling explicit power recomputes outputs.
        set(&mut sim, &inputs, 14, Bit::Zero);
        settle(&mut sim);
        c.plugin
            .as_mut()
            .unwrap()
            .properties
            .insert("internal".into(), "true".into());
        circuit.replace_component(c.clone());
        sim.synchronize(&circuit);
        settle(&mut sim);
        assert_eq!(bit(&sim, &c, kind.gates()[1][2]), Bit::Unknown);
        c.plugin = Some(definition(registry, Chip { kind, power: false }));
        circuit.replace_component(c.clone());
        sim.synchronize(&circuit);
        settle(&mut sim);
        assert_eq!(bit(&sim, &c, kind.gates()[1][2]), expected);
    }
}
fn hierarchy(registry: &Registry) {
    use logisim_core::Project;
    use logisim_sim::ProjectSimulator;
    let mut child = Circuit::default();
    let mut c = Component::from_plugin(
        definition(
            registry,
            Chip {
                kind: Ttl::Ic7404,
                power: false,
            },
        ),
        Point::new(100, 100),
    );
    c.id = child.add_component(c.clone());
    let input = child.add_component(Component::new(Kind::Pin, c.ports()[index(&c, 1)]));
    let mut out = Component::new(Kind::Pin, c.ports()[index(&c, 2)]);
    out.output = true;
    let output = child.add_component(out);
    let mut project = Project::single("main", Circuit::default());
    let def = project.add("chip".into()).unwrap();
    project.circuit_mut(def).unwrap().circuit = child;
    let root = project.main;
    let mut instances = vec![];
    for i in 0..2 {
        instances.push(
            project
                .circuit_mut(root)
                .unwrap()
                .circuit
                .add_component(Component::new(
                    Kind::Subcircuit(def),
                    Point::new(300, 100 + 200 * i),
                )),
        );
    }
    project.refresh_interfaces(false);
    for (i, &id) in instances.iter().enumerate() {
        let instance = project
            .circuit(root)
            .unwrap()
            .circuit
            .component(id)
            .unwrap();
        let port = instance
            .interface
            .ports
            .iter()
            .position(|p| p.pin == input)
            .unwrap();
        let mut constant = Component::new(Kind::Constant, instance.ports()[port]);
        constant.constant = i as u32;
        project
            .circuit_mut(root)
            .unwrap()
            .circuit
            .add_component(constant);
    }
    let mut sim = ProjectSimulator::default();
    sim.set_plugins(registry.clone());
    sim.synchronize(&project, root);
    sim.advance(128);
    assert!(sim.error().is_none());
    assert_eq!(sim.pending_count(), 0);
    assert!(!sim.limited());
    for (i, &id) in instances.iter().enumerate() {
        assert_eq!(
            sim.view(&[id]).unwrap().port_value(output, 0),
            Some(Value::known(1, 1 - i as u32))
        );
        let instance = project
            .circuit(root)
            .unwrap()
            .circuit
            .component(id)
            .unwrap();
        let port = instance
            .interface
            .ports
            .iter()
            .position(|p| p.pin == output)
            .unwrap();
        assert_eq!(
            sim.view(&[]).unwrap().port_value(id, port),
            Some(Value::known(1, 1 - i as u32))
        );
    }
}
