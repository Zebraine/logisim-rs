use super::super::*;
use crate::{Circuit, Component, Facing, Kind};
#[test]
fn selector_group_widths_and_screen_sides_match_runtime_connections() {
    let mut c = Component::new(Kind::Plexer(Plexer::BitSelector), Point::ZERO);
    assert_eq!(c.bits, 8);
    for (width, group, select) in [
        (1, 1, 1),
        (8, 1, 3),
        (8, 3, 2),
        (5, 3, 1),
        (8, 32, 1),
        (32, 1, 5),
        (32, 7, 3),
        (32, 32, 1),
    ] {
        c.bits = width;
        c.plexing.group_bits = group;
        assert_eq!(c.bit_selector_select_bits(), select);
        assert_eq!(c.port_width(0), Some(group));
        assert_eq!(c.port_width(1), Some(width));
        assert_eq!(c.port_width(2), Some(select));
        for (facing, data, bottom, top) in [
            (Facing::East, (-30, 0), (-10, 10), (-10, -10)),
            (Facing::South, (0, -30), (-10, -10), (10, -10)),
            (Facing::West, (30, 0), (10, 10), (10, -10)),
            (Facing::North, (0, 30), (-10, 10), (10, 10)),
        ] {
            c.facing = facing;
            for (tr, s) in [(false, bottom), (true, top)] {
                c.plexing.top_right = tr;
                assert_eq!(
                    c.ports(),
                    vec![
                        Point::ZERO,
                        Point::new(data.0, data.1),
                        Point::new(s.0, s.1)
                    ]
                );
            }
        }
    }
}
#[test]
fn display_screen_order_and_input_format_edits_keep_original_wires() {
    use PlexerRole::*;
    let mut c = Component::new(Kind::Plexer(Plexer::DisplayDecoder), Point::ZERO);
    for (facing, first, data, en, lt, bi, rbi) in [
        (
            Facing::East,
            (0, -30),
            (-40, 0),
            (-20, -40),
            (-30, 40),
            (-20, 40),
            (-10, 40),
        ),
        (
            Facing::South,
            (-30, 0),
            (0, -40),
            (-40, -20),
            (40, -30),
            (40, -20),
            (40, -10),
        ),
        (
            Facing::West,
            (0, -30),
            (40, 0),
            (20, -40),
            (30, 40),
            (20, 40),
            (10, 40),
        ),
        (
            Facing::North,
            (-30, 0),
            (0, 40),
            (-40, 20),
            (40, 30),
            (40, 20),
            (40, 10),
        ),
    ] {
        c.facing = facing;
        for (role, want) in [
            (Output(0), first),
            (Input(0), data),
            (Enable, en),
            (LampTest, lt),
            (Blanking, bi),
            (ZeroBlanking, rbi),
        ] {
            let p = c
                .plexer_terminals()
                .into_iter()
                .find(|p| p.role == role)
                .unwrap();
            assert_eq!(facing.transform(p.offset), Point::new(want.0, want.1));
            assert_eq!(p.bits, if role == Input(0) { 4 } else { 1 });
        }
        c.plexing.multibit = false;
        let ps = c.ports();
        assert_eq!(ps.len(), 15);
        for i in 0..4 {
            assert_eq!(c.port_width(7 + i), Some(1));
        }
        let first_input = match facing {
            Facing::East => Point::new(-40, -30),
            Facing::West => Point::new(40, -30),
            Facing::South => Point::new(-30, -40),
            Facing::North => Point::new(-30, 40),
        };
        assert_eq!(ps[7], first_input);
        c.plexing.multibit = true;
    }
    c.facing = Facing::East;
    c.position = Point::new(200, 200);
    let mut circuit = Circuit::default();
    let id = circuit.add_component(c);
    circuit.add_wire(Point::new(100, 200), Point::new(160, 200));
    circuit.add_wire(Point::new(180, 100), Point::new(180, 160));
    let wires = circuit.wires.clone();
    let mut changed = circuit.component(id).unwrap().clone();
    changed.plexing.multibit = false;
    circuit.replace_component(changed);
    assert_eq!(circuit.wires, wires);
}
