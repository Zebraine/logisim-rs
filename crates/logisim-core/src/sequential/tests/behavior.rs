use super::super::*;
#[test]
fn counter_and_shift_geometry_is_fixed_across_width_and_mode_edits() {
    for bits in [1, 8, 32] {
        let mut c = Component::new(Kind::Counter, Point::ZERO);
        c.bits = bits;
        let ports = [
            (0, 0),
            (-30, 0),
            (-20, 20),
            (-30, -10),
            (-30, 10),
            (0, 10),
            (-10, -20),
            (-10, 20),
            (-20, -20),
        ]
        .map(|(x, y)| Point::new(x, y));
        assert_eq!(c.ports(), ports);
        for i in 0..9 {
            assert_eq!(c.port_width(i), Some(if i < 2 { bits } else { 1 }));
            assert_eq!(c.drives_port(i), i == 0 || i == 5);
        }
        let mut circuit = crate::Circuit::default();
        let id = circuit.add_component(c.clone());
        for port in ports {
            circuit.add_wire(port, port + Point::new(0, 100));
        }
        let wires = circuit.wires.clone();
        c.id = id;
        c.sequential.counter_old = true;
        assert_eq!(c.ports(), ports[..8]);
        circuit.replace_component(c);
        assert_eq!(circuit.wires, wires);
        let mut r = Component::new(Kind::ShiftRegister, Point::ZERO);
        r.bits = bits;
        assert_eq!(
            r.ports(),
            [(30, 0), (0, 0), (0, 10), (10, -20), (10, 20)].map(|(x, y)| Point::new(x, y))
        );
        assert_eq!(
            (0..5).map(|i| r.port_width(i).unwrap()).collect::<Vec<_>>(),
            [bits, bits, 1, 2, 1]
        );
    }
}
#[test]
fn observed_layouts_keep_port_roles_widths_and_connection_edits() {
    for ff in FlipFlop::ALL {
        let mut c = Component::new(Kind::FlipFlop(ff), Point::ZERO);
        assert_eq!(c.ports()[0], Point::ZERO);
        assert_eq!(c.ports()[1], Point::new(0, 40));
        assert_eq!(c.ports()[2], Point::new(-60, 0));
        assert_eq!(
            c.ports()[3],
            Point::new(-60, if ff.paired() { 20 } else { 40 })
        );
        assert!(c.drives_port(0) && c.drives_port(1) && !c.drives_port(2));
        c.sequential.controls = ControlPosition::Legacy;
        assert_eq!(
            &c.ports()[c.ports().len() - 2..],
            &[Point::new(-40, 60), Point::new(-20, 60)]
        );
        c.sequential.new_layout = false;
        assert_eq!(
            c.ports()[2],
            Point::new(-40, if ff.paired() { 10 } else { 0 })
        );
        assert_eq!(
            &c.ports()[c.ports().len() - 2..],
            &[Point::new(-30, 30), Point::new(-10, 30)]
        );
    }
    let mut r = Component::new(Kind::Register, Point::ZERO);
    r.bits = 32;
    assert_eq!(
        r.ports(),
        [
            (0, 0),
            (-30, 0),
            (-20, 20),
            (-30, 10),
            (-30, -10),
            (-10, -20),
            (-10, 20)
        ]
        .map(|(x, y)| Point::new(x, y))
    );
    assert_eq!(
        (0..7).map(|i| r.port_width(i)).collect::<Vec<_>>(),
        vec![
            Some(32),
            Some(32),
            Some(1),
            Some(1),
            Some(1),
            Some(1),
            Some(1)
        ]
    );
    let mut d = crate::Circuit::default();
    let c = Component::new(Kind::FlipFlop(FlipFlop::D), Point::new(100, 100));
    let old = c.ports();
    let id = d.add_component(c);
    let end = Point::new(10, 80);
    d.add_wire(old[4], end);
    let mut changed = d.component(id).unwrap().clone();
    changed.sequential.controls = ControlPosition::BelowAbove;
    let new = changed.ports();
    d.replace_component(changed);
    assert!(d.wires.iter().any(|w| w.a == new[4] || w.b == new[4]));
    assert!(d.wires.iter().any(|w| w.a == end || w.b == end));
}
