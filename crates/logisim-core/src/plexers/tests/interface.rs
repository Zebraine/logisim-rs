use crate::{Component, Facing, Kind, Plexer, PlexerRole, Point};

#[test]
fn logical_interface_keeps_port_order_widths_and_missing_role_boundaries() {
    use PlexerRole::*;
    for (kind, expected) in [
        (
            Plexer::Multiplexer,
            vec![
                (Output(0), 8),
                (Select, 2),
                (Enable, 1),
                (Input(0), 8),
                (Input(1), 8),
                (Input(2), 8),
                (Input(3), 8),
            ],
        ),
        (
            Plexer::Demultiplexer,
            vec![
                (Input(0), 8),
                (Select, 2),
                (Enable, 1),
                (Output(0), 8),
                (Output(1), 8),
                (Output(2), 8),
                (Output(3), 8),
            ],
        ),
        (
            Plexer::Decoder,
            vec![
                (Select, 2),
                (Enable, 1),
                (Output(0), 1),
                (Output(1), 1),
                (Output(2), 1),
                (Output(3), 1),
            ],
        ),
        (
            Plexer::PriorityEncoder,
            vec![
                (Output(0), 2),
                (Group, 1),
                (EnableOut, 1),
                (Enable, 1),
                (Input(0), 1),
                (Input(1), 1),
                (Input(2), 1),
                (Input(3), 1),
            ],
        ),
        (
            Plexer::BitSelector,
            vec![(Output(0), 3), (Input(0), 8), (Select, 2)],
        ),
        (
            Plexer::DisplayDecoder,
            vec![
                (Output(0), 1),
                (Output(1), 1),
                (Output(2), 1),
                (Output(3), 1),
                (Output(4), 1),
                (Output(5), 1),
                (Output(6), 1),
                (Input(0), 4),
                (Enable, 1),
                (LampTest, 1),
                (Blanking, 1),
                (ZeroBlanking, 1),
            ],
        ),
    ] {
        let mut c = Component::new(Kind::Plexer(kind), Point::ZERO);
        c.bits = 8;
        c.plexing.select_bits = 2;
        c.plexing.group_bits = 3;
        c.plexing.enable = true;
        for facing in Facing::ALL {
            c.facing = facing;
            for top_right in [false, true] {
                c.plexing.top_right = top_right;
                let logic = c.plexer_logic().unwrap();
                assert_eq!(
                    logic.ports().map(|p| (p.role, p.bits)).collect::<Vec<_>>(),
                    expected
                );
                assert_eq!(logic.port_count(), expected.len());
                for (i, &(role, bits)) in expected.iter().enumerate() {
                    assert_eq!(logic.port_index(role), Some(i));
                    assert_eq!(c.port_width(i), Some(bits));
                }
                for index in [expected.len(), usize::MAX] {
                    assert_eq!(logic.port(index), None);
                    assert_eq!(c.port_width(index), None);
                    assert!(!c.drives_port(index));
                }
                assert_eq!(logic.port_index(Input(32)), None);
                assert_eq!(logic.port_index(Output(32)), None);
                let missing = match kind {
                    Plexer::PriorityEncoder | Plexer::DisplayDecoder => Select,
                    _ => Group,
                };
                assert_eq!(logic.port_index(missing), None);
            }
        }
        c.plexing.enable = false;
        let logic = c.plexer_logic().unwrap();
        match kind {
            Plexer::Multiplexer => assert_eq!(logic.port_index(Input(0)), Some(2)),
            Plexer::Demultiplexer => assert_eq!(logic.port_index(Output(0)), Some(2)),
            Plexer::Decoder => assert_eq!(logic.port_index(Output(0)), Some(1)),
            Plexer::PriorityEncoder => assert_eq!(logic.port_index(Enable), Some(3)),
            Plexer::BitSelector => assert_eq!(logic.port_index(Enable), None),
            Plexer::DisplayDecoder => {
                c.plexing.multibit = false;
                let logic = c.plexer_logic().unwrap();
                assert_eq!(logic.port_count(), 15);
                assert_eq!(logic.port_index(Enable), Some(11));
                assert_eq!(logic.port_index(ZeroBlanking), Some(14));
                for i in 0..4 {
                    assert_eq!(logic.port_index(Input(i)), Some(7 + usize::from(i)));
                    assert_eq!(logic.port(7 + usize::from(i)).unwrap().bits, 1);
                }
            }
        }
    }
}
