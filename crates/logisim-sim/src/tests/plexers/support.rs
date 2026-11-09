use crate::*;
use logisim_core::{Component, Kind, PlexerRole, Point};
pub(super) fn fixture(mut c: Component) -> (Circuit, Id, Vec<(PlexerRole, Id)>) {
    c.position = Point::new(300, 300);
    let mut circuit = Circuit::default();
    let mut inputs = vec![];
    for p in c.plexer_terminals().iter().filter(|p| !p.output) {
        let mut pin = Component::new(Kind::Pin, c.position + c.facing.transform(p.offset));
        pin.bits = p.bits;
        inputs.push((p.role, circuit.add_component(pin)));
    }
    let id = circuit.add_component(c);
    (circuit, id, inputs)
}
pub(super) fn settle(s: &mut Simulator) {
    s.advance(100);
    assert_eq!(s.pending_count(), 0);
    assert!(!s.limited());
}
pub(super) fn drive(s: &mut Simulator, inputs: &[(PlexerRole, Id)], role: PlexerRole, v: Value) {
    s.external_input(inputs.iter().find(|(r, _)| *r == role).unwrap().1, v);
}
pub(super) fn output(s: &Simulator, id: Id, role: PlexerRole) -> Value {
    let c = s.circuit.component(id).unwrap();
    s.port_value(
        id,
        c.plexer_terminals()
            .iter()
            .position(|p| p.role == role)
            .unwrap(),
    )
    .unwrap()
}
