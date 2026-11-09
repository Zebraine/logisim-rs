use super::*;
#[test]
fn manifest_interfaces_and_contracts_are_bounded_and_stateless() {
    let m = graphics::manifest();
    m.validate().unwrap();
    assert_eq!(m.tools.len(), 14);
    assert!(
        serde_json::to_vec(&Response::Manifest(m.clone()))
            .unwrap()
            .len()
            < MAX_MESSAGE
    );
    for c in m.components() {
        let chip = model::Chip::parse(&c.tool.id).unwrap();
        assert_eq!(c.tool.ports.len(), if chip.power { 14 } else { 12 });
        for (i, p) in c.tool.ports.iter().enumerate() {
            assert_eq!(p.width, 1);
            assert_eq!(p.output, chip.kind.output(chip.pins()[i]));
        }
        let config = StableConfig {
            tool: c.tool.id.clone(),
            properties: c.properties.clone(),
        };
        let contract = LIBRARY.contract(config).unwrap().unwrap();
        contract.validate(&c.tool).unwrap();
        assert_eq!(contract.state_inputs, None);
        assert_eq!(contract.outputs.len(), chip.kind.gates().len());
        let mut internal = c.tool.clone();
        internal.graphics = graphics::drawing(chip, true);
        internal.validate().unwrap();
    }
}
#[test]
fn malformed_requests_do_not_bypass_schema_or_stateless_contract() {
    let c = graphics::manifest().components().remove(0);
    let request = Evaluation {
        tool: c.tool.id.clone(),
        properties: c.properties.clone(),
        inputs: vec![Value::known(1, 0).into(); 12],
        state: vec![],
        event: Event::Reset,
    };
    for bad in [
        Evaluation {
            tool: "unknown".into(),
            ..request.clone()
        },
        Evaluation {
            state: vec![1],
            ..request.clone()
        },
        Evaluation {
            inputs: vec![],
            ..request.clone()
        },
        Evaluation {
            properties: Properties::new(),
            ..request.clone()
        },
        Evaluation {
            event: Event::Action {
                id: "write".into(),
                argument: String::new(),
            },
            ..request.clone()
        },
    ] {
        assert!(matches!(handle(Request::Evaluate(bad)), Response::Error(_)));
    }
    assert!(
        handle_stable_frame(
            &StableConfig {
                tool: c.tool.id,
                properties: c.properties
            },
            frame::Operation::Outputs,
            &request.inputs,
            &[1]
        )
        .is_err()
    );
}

#[test]
fn dip_pin_geometry_roles_and_attribute_edits_preserve_wires() {
    use crate::model::{Chip, Ttl};
    use logisim_core::{Circuit, Component, Facing, Point};
    let physical = [
        (10, 30),
        (30, 30),
        (50, 30),
        (70, 30),
        (90, 30),
        (110, 30),
        (130, 30),
        (130, -30),
        (110, -30),
        (90, -30),
        (70, -30),
        (50, -30),
        (30, -30),
        (10, -30),
    ];
    for kind in Ttl::ALL {
        for facing in Facing::ALL {
            for power in [false, true] {
                let chip = Chip { kind, power };
                let definition = graphics::manifest()
                    .components()
                    .into_iter()
                    .find(|c| c.tool.id == chip.id())
                    .unwrap();
                let mut c = Component::from_plugin(definition, Point::new(300, 300));
                c.facing = facing;
                assert_eq!(c.ports().len(), if power { 14 } else { 12 });
                let outputs: &[u8] = match kind {
                    Ttl::Ic7402 => &[1, 4, 10, 13],
                    Ttl::Ic7404 => &[2, 4, 6, 8, 10, 12],
                    _ => &[3, 6, 8, 11],
                };
                for n in 1..=14 {
                    let index = chip.pins().iter().position(|p| *p == n);
                    if !power && matches!(n, 7 | 14) {
                        assert!(index.is_none());
                        continue;
                    }
                    let i = index.unwrap();
                    let (x, y) = physical[usize::from(n - 1)];
                    assert_eq!(
                        c.ports()[i],
                        c.position + facing.transform(Point::new(x, y))
                    );
                    assert_eq!(c.drives_port(i), outputs.contains(&n));
                    assert_eq!(c.port_width(i), Some(1));
                }
                assert_eq!(c.port_width(c.ports().len()), None);
                assert!(!c.drives_port(c.ports().len()));
                let mut circuit = Circuit::default();
                let id = circuit.add_component(c.clone());
                for point in c.ports() {
                    circuit.add_wire(
                        point,
                        point
                            + facing.transform(Point::new(0, if point.y > 300 { 40 } else { -40 })),
                    );
                }
                let wires = circuit.wires.clone();
                c.id = id;
                c.facing = facing.next();
                let next = Chip {
                    kind,
                    power: !power,
                };
                c.plugin = graphics::manifest()
                    .components()
                    .into_iter()
                    .find(|d| d.tool.id == next.id());
                c.plugin
                    .as_mut()
                    .unwrap()
                    .properties
                    .insert("internal".into(), "true".into());
                circuit.replace_component(c);
                assert_eq!(circuit.wires, wires);
            }
        }
    }
}
