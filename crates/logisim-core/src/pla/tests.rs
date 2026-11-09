use super::*;
use crate::{Circuit, Component, Kind, Point};
#[test]
fn resizing_preserves_table_columns_discards_removed_cells_and_keeps_wires() {
    let mut c = Component::new(Kind::PlaRom, Point::new(100, 100));
    c.pla.resize(2, 2, 2);
    c.pla.set_input(0, 0, PlaInput::Direct);
    c.pla.set_input(1, 1, PlaInput::Inverted);
    c.pla.set_output(1, 1, true);
    let original = c.clone();
    let mut circuit = Circuit::default();
    let id = circuit.add_component(c.clone());
    circuit.add_wire(Point::new(100, 100), Point::new(50, 100));
    circuit.add_wire(Point::new(160, 100), Point::new(200, 100));
    let wires = circuit.wires.clone();
    c.id = id;
    c.pla.resize(1, 1, 1);
    c.pla.resize(32, 32, 32);
    assert_eq!(c.pla.input(0, 0), PlaInput::Direct);
    assert_eq!(c.pla.input(1, 1), PlaInput::Disconnected);
    assert!(!c.pla.output(1, 1));
    assert!(original.pla.output(1, 1));
    assert_eq!(
        c.ports(),
        vec![
            Point::new(160, 100),
            Point::new(100, 100),
            Point::new(130, 130)
        ]
    );
    assert_eq!(
        (0..3).map(|i| c.port_width(i)).collect::<Vec<_>>(),
        vec![Some(32), Some(32), Some(1)]
    );
    assert!(c.drives_port(0) && !c.drives_port(1) && !c.drives_port(2));
    assert!(!c.kind.rotatable());
    circuit.replace_component(c);
    assert_eq!(circuit.wires, wires);
}
