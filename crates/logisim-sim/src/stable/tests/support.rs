use crate::stable::*;
use logisim_core::{Circuit, Component, Facing, Gate, Point, Project};

pub(super) fn add(c: &mut Circuit, kind: Kind, bits: u8) -> Id {
    let mut component =
        Component::new(kind, Point::new(200 + c.components.len() as i32 * 200, 300));
    component.bits = bits;
    c.add_component(component)
}
pub(super) fn link(c: &mut Circuit, from: Port, to: Port) {
    let label = format!("{}:{}", from.0, from.1);
    for port in [from, to] {
        let component = c.component(port.0).unwrap();
        let mut tunnel = Component::new(Kind::Tunnel, component.ports()[port.1]);
        tunnel.bits = component.port_width(port.1).unwrap();
        tunnel.label = label.clone();
        c.add_component(tunnel);
    }
}
pub(super) fn driven(c: &mut Circuit, port: Port) -> Id {
    let pin = add(
        c,
        Kind::Pin,
        c.component(port.0).unwrap().port_width(port.1).unwrap(),
    );
    link(c, (pin, 0), port);
    pin
}
pub(super) fn chain(c: &mut Circuit, input: Id, count: usize) -> Id {
    let mut previous = input;
    let width = c.component(input).unwrap().port_width(0).unwrap();
    for _ in 0..count {
        let gate = add(c, Kind::Gate(Gate::Buffer), width);
        link(c, (previous, 0), (gate, 1));
        previous = gate;
    }
    previous
}
pub(super) fn stored(sim: &Simulator, id: Id) -> u32 {
    sim.stored_value(id).unwrap().number().unwrap()
}
pub(super) fn output_pin(c: &mut Circuit) -> Id {
    let mut pin = Component::new(Kind::Pin, Point::new(5000, 2000));
    pin.output = true;
    pin.facing = Facing::West;
    c.add_component(pin)
}

pub(super) fn buffered_child(p: &mut Project, depth: usize) -> (Id, Id, Id) {
    let id = p.add(format!("buffer{depth}")).unwrap();
    let c = &mut p.circuit_mut(id).unwrap().circuit;
    let input = add(c, Kind::Pin, 1);
    let buffer = chain(c, input, depth);
    let output = output_pin(c);
    link(c, (buffer, 0), (output, 0));
    (id, input, output)
}
pub(super) fn instance(
    p: &mut Project,
    definition: Id,
    parent: Id,
    position: Point,
) -> (Id, Vec<(Id, usize)>) {
    let comp = p.instance(definition, position).unwrap();
    let pins = comp
        .interface
        .ports
        .iter()
        .enumerate()
        .map(|(i, p)| (p.pin, i))
        .collect();
    (
        p.circuit_mut(parent).unwrap().circuit.add_component(comp),
        pins,
    )
}
