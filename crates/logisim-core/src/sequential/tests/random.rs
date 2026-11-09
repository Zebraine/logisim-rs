use super::super::*;
#[test]
fn random_fixed_geometry_and_port_widths_survive_property_edits() {
    let mut c = Component::new(Kind::Random, Point::new(200, 200));
    assert_eq!(c.bits, 8);
    assert!(!c.kind.rotatable());
    assert!(c.kind.has_label());
    let ports = [(200, 200), (170, 210), (170, 190), (180, 220)].map(|(x, y)| Point::new(x, y));
    assert_eq!(c.ports(), ports);
    let mut circuit = crate::Circuit::default();
    c.id = circuit.add_component(c.clone());
    for port in ports {
        circuit.add_wire(port, port + Point::new(60, 0));
    }
    let wires = circuit.wires.clone();
    for bits in [1, 8, 32] {
        c.bits = bits;
        c.sequential.random_seed = i32::MIN;
        c.sequential.trigger = Trigger::Falling;
        assert_eq!(c.ports(), ports);
        for i in 0..4 {
            assert_eq!(c.drives_port(i), i == 0);
            assert_eq!(c.port_width(i), Some(if i == 0 { bits } else { 1 }));
        }
        assert_eq!(c.port_width(4), None);
        circuit.replace_component(c.clone());
        assert_eq!(circuit.wires, wires);
    }
}
