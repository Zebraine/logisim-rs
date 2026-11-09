use super::*;
use crate::{Circuit, Facing};
#[test]
fn uneven_descending_distribution_reverses_bit_order_not_branch_sizes() {
    let mut splitter = Splitter {
        fanout: 3,
        ..Splitter::default()
    };
    splitter.distribute(8, false);
    assert_eq!(
        &splitter.mapping[..8],
        &[
            Some(0),
            Some(0),
            Some(0),
            Some(1),
            Some(1),
            Some(1),
            Some(2),
            Some(2)
        ]
    );
    splitter.distribute(8, true);
    assert_eq!(
        &splitter.mapping[..8],
        &[
            Some(2),
            Some(2),
            Some(1),
            Some(1),
            Some(1),
            Some(0),
            Some(0),
            Some(0)
        ]
    );
    assert!(splitter.descending);
}
#[test]
fn measured_orientations_keep_reference_branch_numbering() {
    let mut c = Component::new(Kind::Splitter, Point::new(100, 100));
    for (facing, expected) in [
        (Facing::East, [(120, 60), (120, 70), (120, 80), (120, 90)]),
        (
            Facing::South,
            [(140, 120), (130, 120), (120, 120), (110, 120)],
        ),
        (Facing::West, [(80, 110), (80, 120), (80, 130), (80, 140)]),
        (Facing::North, [(90, 80), (80, 80), (70, 80), (60, 80)]),
    ] {
        c.facing = facing;
        assert_eq!(c.ports()[1..], expected.map(|(x, y)| Point::new(x, y)));
    }
}
#[test]
fn centered_even_and_odd_splitters_match_observed_world_coordinates() {
    for appearance in [SplitAppearance::Center, SplitAppearance::Legacy] {
        for n in [2, 3, 4, 5] {
            for facing in Facing::ALL {
                let mut c = Component::new(Kind::Splitter, Point::new(100, 100));
                c.facing = facing;
                c.splitter.fanout = n;
                c.splitter.appearance = appearance;
                let ports = c.ports();
                for branch in 0..i32::from(n) {
                    let vertical = 10 * (branch - i32::from(n) / 2);
                    let horizontal = 10 * (i32::from(n) - 1 - branch - i32::from(n) / 2);
                    let expected = match facing {
                        Facing::East => Point::new(120, 100 + vertical),
                        Facing::West => Point::new(80, 100 + vertical),
                        Facing::South => Point::new(100 + horizontal, 120),
                        Facing::North => Point::new(100 + horizontal, 80),
                    };
                    assert_eq!(
                        ports[branch as usize + 1],
                        expected,
                        "{appearance:?} {n} {facing:?} {branch}"
                    );
                }
            }
        }
    }
}
#[test]
fn editing_splitter_layout_reroutes_surviving_branch_connections() {
    let mut d = Circuit::default();
    let c = Component::new(Kind::Splitter, Point::ZERO);
    let before = c.ports();
    let id = d.add_component(c);
    d.add_wire(before[1], Point::new(100, before[1].y));
    let mut c = d.component(id).unwrap().clone();
    c.splitter.appearance = SplitAppearance::Right;
    c.splitter.fanout = 3;
    c.splitter.distribute(c.bits, false);
    let after = c.ports();
    d.replace_component(c);
    assert!(d.wires.iter().any(|w| w.a == after[1] || w.b == after[1]));
    assert!(
        d.wires
            .iter()
            .all(|w| w.a != w.b && (w.a.x == w.b.x || w.a.y == w.b.y))
    );
}
