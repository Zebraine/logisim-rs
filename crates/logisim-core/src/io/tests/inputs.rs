use super::super::*;
use crate::Circuit;
#[test]
fn input_geometry_preserves_observed_order_and_existing_wires() {
    for facing in Facing::ALL {
        let mut switch = Component::new(Kind::Switch, Point::new(200, 200));
        switch.facing = facing;
        switch.bits = 32;
        assert_eq!(
            switch.ports(),
            vec![
                switch.position,
                switch.position + facing.transform(Point::new(-20, 0))
            ]
        );
        assert_eq!(switch.port_width(1), Some(32));
        assert!(switch.drives_port(0) && !switch.drives_port(1));
        for count in [1, 4, 32] {
            let mut dip = Component::new(Kind::DipSwitch, Point::ZERO);
            dip.facing = facing;
            dip.io.switch_count = count;
            let ports: Vec<_> = (0..count)
                .map(|i| {
                    if matches!(facing, Facing::East | Facing::West) {
                        Point::new(0, 10 + 20 * i32::from(i))
                    } else {
                        Point::new(10 + 20 * i32::from(i), 0)
                    }
                })
                .collect();
            assert_eq!(dip.ports(), ports);
            for i in 0..usize::from(count) {
                assert!(dip.drives_port(i));
                assert_eq!(dip.port_width(i), Some(1));
            }
            assert!(!dip.drives_port(usize::from(count)));
            let mut circuit = Circuit::default();
            let id = circuit.add_component(dip.clone());
            for p in ports {
                circuit.add_wire(p, p + Point::new(100, 0));
            }
            let wires = circuit.wires.clone();
            dip.id = id;
            dip.io.switch_count = 2;
            circuit.replace_component(dip);
            assert_eq!(circuit.wires, wires);
        }
        let mut slider = Component::new(Kind::Slider, Point::ZERO);
        slider.facing = facing;
        assert_eq!(slider.ports(), vec![Point::ZERO]);
        assert_eq!(slider.port_width(0), Some(8));
        let world = slider.io_body().map(|p| facing.transform(p));
        let expected = match facing {
            Facing::East => [Point::new(-120, -15), Point::new(0, 15)],
            Facing::West => [Point::new(0, -15), Point::new(120, 15)],
            Facing::South => [Point::new(-60, -30), Point::new(60, 0)],
            Facing::North => [Point::new(-60, 0), Point::new(60, 30)],
        };
        assert_eq!(world, expected);
    }
}
#[test]
fn slider_full_width_rounding_and_reverse_direction() {
    let mut c = Component::new(Kind::Slider, Point::ZERO);
    for (bits, middle, almost, max) in [
        (1, 1, 1, 1),
        (4, 8, 15, 15),
        (8, 128, 252, 255),
        (32, 0x80000000, 0xfd70a3d6, u32::MAX),
    ] {
        c.bits = bits;
        assert_eq!(c.slider_value_at(0), 0);
        assert_eq!(c.slider_value_at(50), middle);
        assert_eq!(c.slider_value_at(99), almost);
        assert_eq!(c.slider_value_at(100), max);
        c.io.slider_reverse = true;
        assert_eq!(c.slider_value_at(0), max);
        assert_eq!(c.slider_value_at(1), almost);
        assert_eq!(c.slider_value_at(100), 0);
        c.io.slider_reverse = false;
    }
}
