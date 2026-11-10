//! Ordinary subcircuit definitions and named port bindings for authored examples.
use super::{Builder, E};
use logisim_core::*;
use std::collections::BTreeMap;

pub fn define(project: &mut Project, name: &str, mut circuit: Circuit) -> Result<Id, String> {
    let id = project.add(name.into())?;
    let mut appearance = Appearance {
        shapes: vec![],
        ports: BTreeMap::new(),
        anchor: Point::ZERO,
        facing: Facing::East,
    };
    let mut counts = [0, 0];
    for pin in circuit.components.iter().filter(|c| c.kind == Kind::Pin) {
        let side = usize::from(pin.output);
        counts[side] += 1;
        let y = counts[side] * 40;
        appearance
            .ports
            .insert(pin.id, Point::new(if pin.output { 200 } else { 0 }, y));
        appearance.add(
            Geometry::Text {
                at: Point::new(if pin.output { 190 } else { 10 }, y - 3),
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
    appearance.shapes.insert(
        0,
        Artwork {
            id: (appearance.shapes.len() + 1) as Id,
            geometry: Geometry::Rect {
                a: Point::ZERO,
                b: Point::new(200, (counts[0].max(counts[1]) + 1) * 40),
                radius: 5,
            },
            paint: Paint {
                fill: Some([241, 245, 249]),
                ..Default::default()
            },
        },
    );
    circuit.appearance = Some(appearance);
    project.circuit_mut(id).unwrap().circuit = circuit;
    project.refresh_interfaces(false);
    Ok(id)
}

pub fn connect(
    b: &mut Builder,
    project: &Project,
    definition: Id,
    position: Option<Point>,
    inputs: &[(&str, E)],
    outputs: &[(&str, &str)],
) -> Id {
    let mut component = project
        .instance(definition, position.unwrap_or(Point::ZERO))
        .unwrap();
    component.label = project.circuit(definition).unwrap().name.clone();
    let ports = component.interface.ports.clone();
    let id = if position.is_some() {
        b.circuit.add_component(component)
    } else {
        b.add(component)
    };
    for (port, p) in ports.iter().enumerate() {
        if p.output {
            let name = outputs.iter().find(|(n, _)| *n == p.label).unwrap().1;
            b.attach(id, port, name, true);
        } else {
            let value = &inputs.iter().find(|(n, _)| *n == p.label).unwrap().1;
            b.connect(id, port, value);
        }
    }
    id
}

pub fn select(index: &E, values: &[E]) -> E {
    assert_eq!(values.len(), 1 << index.bits);
    fn tree(index: &E, values: &[E], bit: u8) -> E {
        if values.len() == 1 {
            return values[0].clone();
        }
        let half = values.len() / 2;
        index.slice(bit, 1).choose(
            &tree(index, &values[half..], bit.saturating_sub(1)),
            &tree(index, &values[..half], bit.saturating_sub(1)),
        )
    }
    tree(index, values, index.bits - 1)
}
