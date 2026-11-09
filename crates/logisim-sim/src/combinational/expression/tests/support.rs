use super::super::*;
use logisim_core::{Component, Id, Kind, Point};

pub(super) const FOUR: [Bit; 4] = [Bit::Zero, Bit::One, Bit::Unknown, Bit::Error];

pub(super) fn component(kind: Kind, width: u8) -> Component {
    let mut c = Component::new(kind, Point::new(300, 300));
    c.bits = width;
    c
}

pub(super) fn fixture(c: Component) -> (Circuit, Id, Vec<(Id, u8)>) {
    let mut circuit = Circuit::default();
    let mut inputs = vec![];
    for (i, point) in c.ports().into_iter().enumerate() {
        if !c.drives_port(i) {
            let mut pin = Component::new(Kind::Pin, point);
            pin.bits = c.port_width(i).unwrap();
            let width = pin.bits;
            inputs.push((circuit.add_component(pin), width));
        }
    }
    let id = circuit.add_component(c);
    (circuit, id, inputs)
}

pub(super) fn link(c: &mut Circuit, from: Port, to: Port, label: &str) {
    for port in [from, to] {
        let comp = c.component(port.0).unwrap();
        let mut t = Component::new(Kind::Tunnel, comp.ports()[port.1]);
        t.bits = comp.port_width(port.1).unwrap();
        t.label = label.into();
        c.add_component(t);
    }
}
