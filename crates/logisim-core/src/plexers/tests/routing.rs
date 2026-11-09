use super::super::*;
use crate::{Circuit, Component, Facing, Kind};
fn terminal(c: &Component, role: PlexerRole) -> Point {
    c.position
        + c.facing.transform(
            c.plexer_terminals()
                .into_iter()
                .find(|p| p.role == role)
                .unwrap()
                .offset,
        )
}
#[test]
fn observed_default_and_rotated_numbering_and_control_sides() {
    use PlexerRole::*;
    let mut mux = Component::new(Kind::Plexer(Plexer::Multiplexer), Point::ZERO);
    assert_eq!(
        mux.ports(),
        [(0, 0), (-20, 20), (-30, -10), (-30, 10)].map(|(x, y)| Point::new(x, y))
    );
    mux.plexing.select_bits = 2;
    mux.plexing.enable = true;
    for (facing, input, select, enable) in [
        (Facing::East, (-40, -20), (-20, 20), (-10, 20)),
        (Facing::South, (-20, -40), (-20, -20), (-20, -10)),
        (Facing::West, (40, -20), (20, 20), (10, 20)),
        (Facing::North, (-20, 40), (-20, 20), (-20, 10)),
    ] {
        mux.facing = facing;
        assert_eq!(terminal(&mux, Input(0)), Point::new(input.0, input.1));
        assert_eq!(terminal(&mux, Select), Point::new(select.0, select.1));
        assert_eq!(terminal(&mux, Enable), Point::new(enable.0, enable.1));
    }
    let mut dec = Component::new(Kind::Plexer(Plexer::Decoder), Point::ZERO);
    assert_eq!(terminal(&dec, Output(0)), Point::new(10, -30));
    dec.plexing.select_bits = 2;
    dec.facing = Facing::South;
    assert_eq!(terminal(&dec, Output(0)), Point::new(0, 20));
    assert_eq!(terminal(&dec, Output(3)), Point::new(30, 20));
    dec.plexing.top_right = true;
    assert_eq!(terminal(&dec, Output(0)), Point::new(-40, 20));
    let enc = Component::new(Kind::Plexer(Plexer::PriorityEncoder), Point::ZERO);
    assert_eq!(terminal(&enc, Input(0)), Point::new(-40, -30));
    assert_eq!(terminal(&enc, Enable), Point::new(-20, 50));
    assert_eq!(terminal(&enc, EnableOut), Point::new(-20, -40));
    assert_eq!(terminal(&enc, Group), Point::new(0, 10));
}
#[test]
fn edits_preserve_semantic_connections_when_control_and_data_counts_change() {
    use PlexerRole::*;
    let mut circuit = Circuit::default();
    let mut c = Component::new(Kind::Plexer(Plexer::Multiplexer), Point::new(300, 300));
    let data = terminal(&c, Input(1));
    let sel = terminal(&c, Select);
    let id = circuit.add_component(c.clone());
    circuit.add_wire(data, Point::new(100, data.y));
    circuit.add_wire(sel, Point::new(sel.x, 500));
    c.id = id;
    c.plexing.enable = true;
    c.plexing.select_bits = 3;
    c.plexing.top_right = true;
    let new_data = terminal(&c, Input(1));
    let new_select = terminal(&c, Select);
    let en = terminal(&c, Enable);
    circuit.replace_component(c);
    assert!(
        circuit
            .wires
            .iter()
            .any(|w| w.a == new_data || w.b == new_data)
    );
    assert!(
        circuit
            .wires
            .iter()
            .any(|w| w.a == new_select || w.b == new_select)
    );
    assert!(!circuit.wires.iter().any(|w| w.a == en || w.b == en));
}
