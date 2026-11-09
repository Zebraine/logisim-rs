use logisim_core::{Component, Facing, Gate, GateSize, Kind, Point};

#[test]
fn measured_gate_matrix_has_exact_ports_and_rotations() {
    for size in [GateSize::Narrow, GateSize::Medium, GateSize::Wide] {
        for n in 2..=8 {
            let expected: &[i32] = match (size, n) {
                (GateSize::Medium | GateSize::Wide, 2) => &[-20, 20],
                (GateSize::Medium, 3) => &[-20, 0, 20],
                (GateSize::Wide, 3) => &[-30, 0, 30],
                (GateSize::Wide, 4) => &[-30, -10, 10, 30],
                (_, 2) => &[-10, 10],
                (_, 3) => &[-10, 0, 10],
                (_, 4) => &[-20, -10, 10, 20],
                (_, 5) => &[-20, -10, 0, 10, 20],
                (_, 6) => &[-30, -20, -10, 10, 20, 30],
                (_, 7) => &[-30, -20, -10, 0, 10, 20, 30],
                _ => &[-40, -30, -20, -10, 10, 20, 30, 40],
            };
            for g in [
                Gate::And,
                Gate::Or,
                Gate::Nand,
                Gate::Nor,
                Gate::Xor,
                Gate::Xnor,
            ] {
                let width = match size {
                    GateSize::Narrow => 30,
                    GateSize::Medium => 50,
                    GateSize::Wide => 70,
                };
                let depth = width
                    + match g {
                        Gate::Nand | Gate::Nor | Gate::Xor => 10,
                        Gate::Xnor => 20,
                        _ => 0,
                    };
                let mut c = Component::new(Kind::Gate(g), Point::ZERO);
                c.gate_size = size;
                c.inputs = n;
                for facing in Facing::ALL {
                    c.facing = facing;
                    let mut positions: Vec<_> = expected
                        .iter()
                        .map(|y| facing.transform(Point::new(-depth, *y)))
                        .collect();
                    positions.sort_by_key(|p| {
                        if matches!(facing, Facing::East | Facing::West) {
                            p.y
                        } else {
                            p.x
                        }
                    });
                    assert_eq!(c.ports()[1..], positions);
                }
            }
        }
    }
}
