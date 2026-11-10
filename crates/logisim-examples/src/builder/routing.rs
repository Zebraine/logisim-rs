//! Spread nearby terminals before drawing tunnel labels. Routes remain on the grid.
use logisim_core::{Component, Facing, Kind, Point};

pub(super) fn port_side(c: &Component, port: usize) -> Facing {
    let local = c.local_ports()[port];
    if c.kind == Kind::Splitter {
        if port == 0 {
            Facing::East
        } else {
            Facing::West
        }
    } else if c.drives_port(port) {
        Facing::West
    } else if matches!(
        c.kind,
        Kind::Register
            | Kind::Random
            | Kind::Ram
            | Kind::Rom
            | Kind::Arithmetic(_)
            | Kind::Plexer(_)
    ) && local.x != c.local_ports().iter().map(|p| p.x).min().unwrap()
        && local.y != 0
    {
        if local.y > 0 {
            Facing::North
        } else {
            Facing::South
        }
    } else {
        Facing::East
    }
}

pub(super) fn fanout(c: &Component, port: usize, facing: Facing) -> Vec<Point> {
    // In this coordinate frame, every tunnel points right toward the component.
    let inverse = facing.inverse();
    let mut group: Vec<_> = c
        .ports()
        .into_iter()
        .enumerate()
        .filter(|(i, _)| port_side(c, *i) == facing)
        .map(|(i, at)| (i, inverse.transform(at)))
        .collect();
    group.sort_by_key(|(_, p)| p.y);
    let index = group.iter().position(|(i, _)| *i == port).unwrap();
    let at = group[index].1;
    let mut ys = vec![group[0].1.y];
    for (_, p) in &group[1..] {
        ys.push(p.y.max(ys.last().unwrap() + 40));
    }
    let shift = (ys.last().unwrap() - group.last().unwrap().1.y) / 20 * 10;
    let y = ys[index] - shift;
    // Outer routes bend closer to the body, so inner routes can pass outside them.
    let layer = index.min(group.len() - 1 - index) as i32;
    let left = group.iter().map(|(_, p)| p.x).min().unwrap();
    let bend = left - 20 - layer * 10;
    let end = left - 40 - group.len() as i32 / 2 * 10;
    [
        at,
        Point::new(bend, at.y),
        Point::new(bend, y),
        Point::new(end, y),
    ]
    .into_iter()
    .map(|p| facing.transform(p))
    .collect()
}
