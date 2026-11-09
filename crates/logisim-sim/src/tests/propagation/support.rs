use crate::*;
use logisim_core::{Component, Gate, Kind, Point};

pub(super) fn pin(circuit: &mut Circuit, point: Point, bits: u8, output: bool) -> Id {
    let mut pin = Component::new(Kind::Pin, point);
    pin.bits = bits;
    pin.output = output;
    circuit.add_component(pin)
}
pub(super) fn fixture(gate: Gate, count: u8, bits: u8) -> (Circuit, Id, Vec<Id>) {
    let mut circuit = Circuit::default();
    let mut c = Component::new(Kind::Gate(gate), Point::new(200, 100));
    c.bits = bits;
    c.inputs = count;
    let ports = c.ports();
    let gate = circuit.add_component(c);
    let inputs = ports
        .into_iter()
        .skip(1)
        .map(|p| pin(&mut circuit, p, bits, false))
        .collect();
    let output = Point::new(340, 100);
    pin(&mut circuit, output, bits, true);
    circuit.add_wire(Point::new(200, 100), output);
    (circuit, gate, inputs)
}
pub(super) fn run(circuit: &Circuit) -> Simulator {
    let mut sim = Simulator::default();
    sim.synchronize(circuit);
    sim.advance(256);
    sim
}

pub(super) fn source(circuit: &mut Circuit, at: Point, bits: u8, number: u32) -> Id {
    let mut c = Component::new(Kind::Constant, at);
    c.bits = bits;
    c.constant = number;
    circuit.add_component(c)
}
