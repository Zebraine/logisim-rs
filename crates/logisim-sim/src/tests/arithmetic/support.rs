use crate::*;
use logisim_core::{Arithmetic, Component, Kind, Point};

pub(super) fn value(text: &str) -> Value {
    Value::from_bits(text.chars().rev().map(|b| match b {
        '0' => Bit::Zero,
        '1' => Bit::One,
        'X' => Bit::Unknown,
        'E' => Bit::Error,
        _ => panic!("bad test signal"),
    }))
}
pub(super) fn fixture(kind: Arithmetic, width: u8) -> (Circuit, Id, Vec<Id>) {
    let mut c = Component::new(Kind::Arithmetic(kind), Point::new(300, 300));
    c.bits = width;
    component_fixture(c)
}
pub(super) fn component_fixture(c: Component) -> (Circuit, Id, Vec<Id>) {
    let mut circuit = Circuit::default();
    let mut inputs = vec![];
    for (i, point) in c
        .ports()
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !c.drives_port(*i))
    {
        let mut pin = Component::new(Kind::Pin, point);
        pin.bits = c.port_width(i).unwrap();
        inputs.push(circuit.add_component(pin));
    }
    let id = circuit.add_component(c);
    (circuit, id, inputs)
}
pub(super) fn run(s: &mut Simulator, circuit: &Circuit, pins: &[Id], values: &[Value]) {
    s.synchronize(circuit);
    for (&id, &v) in pins.iter().zip(values) {
        s.external_input(id, v);
    }
    s.advance(100);
    assert_eq!(s.pending_count(), 0);
    assert!(!s.limited());
}
pub(super) fn output(s: &Simulator, id: Id, port: usize) -> String {
    s.port_value(id, port).unwrap().binary()
}
