use super::*;

fn check(wires: &[Wire], points: impl IntoIterator<Item = Point>) {
    let index = WireIndex::new(wires);
    for point in points {
        let expected: Vec<_> = wires
            .iter()
            .enumerate()
            .filter(|(_, w)| w.contains(point))
            .map(|(i, _)| i)
            .collect();
        let mut found = vec![];
        index.visit(point, |i, w| {
            assert_eq!(w, &wires[i]);
            found.push(i);
        });
        found.sort_unstable();
        assert_eq!(found, expected, "point {point:?}");
    }
}

#[test]
fn interval_queries_match_linear_search_at_overlaps_crossings_and_coordinate_limits() {
    let mut wires = vec![];
    for (a, b) in [
        (Point::new(i32::MIN, -10), Point::new(i32::MAX, -10)),
        (Point::new(-10, i32::MAX), Point::new(-10, i32::MIN)),
        (Point::new(0, 10), Point::new(0, -10)),
        (Point::new(-10, 0), Point::new(10, 0)),
        (Point::new(10, 0), Point::new(-10, 0)),
        (Point::new(-5, 0), Point::new(5, 0)),
        (Point::new(0, 0), Point::new(0, 0)),
        (Point::new(20, 0), Point::new(30, 0)),
    ] {
        wires.push(Wire {
            id: wires.len() as u64 + 1,
            a,
            b,
        });
    }
    let coords = [i32::MIN, -11, -10, -5, 0, 5, 10, 11, 15, 20, 30, i32::MAX];
    let points: Vec<_> = coords
        .into_iter()
        .flat_map(|x| coords.map(|y| Point::new(x, y)))
        .collect();
    check(&wires, points.clone());
    wires.reverse();
    wires[0].a.x = -30;
    check(&wires, points.clone());
    wires.truncate(3);
    check(&wires, points.clone());
    check(&[], points);
}

#[test]
fn randomized_point_queries_preserve_every_segment_and_duplicate_geometry() {
    let mut state = 12_u64;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((state >> 32) % 21) as i32 - 10
    };
    for count in 0..128 {
        let wires: Vec<_> = (0..count)
            .map(|i| {
                let a = Point::new(next() * 10, next() * 10);
                let b = if next() % 2 == 0 {
                    Point::new(a.x, next() * 10)
                } else {
                    Point::new(next() * 10, a.y)
                };
                Wire { id: i + 1, a, b }
            })
            .collect();
        let points = wires
            .iter()
            .flat_map(|w| [w.a, w.b, Point::new(w.a.x, 0), Point::new(0, w.a.y)])
            .chain((0..50).map(|_| Point::new(next() * 10, next() * 10)))
            .collect::<Vec<_>>();
        check(&wires, points);
    }
}

#[test]
fn sparse_and_nested_long_intervals_do_not_require_coordinate_cells() {
    let mut wires: Vec<_> = (0..10_000)
        .map(|i| Wire {
            id: i as u64 + 1,
            a: Point::new(i * 100_000, 0),
            b: Point::new(i * 100_000 + 50_000, 0),
        })
        .collect();
    check(
        &wires,
        [
            Point::ZERO,
            Point::new(75_000, 0),
            Point::new(999_950_000, 0),
            Point::new(i32::MAX, 0),
        ],
    );
    wires.extend((0..512).map(|i| Wire {
        id: 10_001 + i as u64,
        a: Point::new(-1_000_000 + i, -10),
        b: Point::new(1_000_000 - i, -10),
    }));
    check(
        &wires,
        (-1_000_001..=-999_480)
            .map(|x| Point::new(x, -10))
            .chain([Point::new(0, -10), Point::new(999_900, -10)]),
    );
}
