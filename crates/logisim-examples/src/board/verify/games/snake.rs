use super::*;
use std::collections::VecDeque;

pub(super) fn check(mut g: Game<'_>) -> Result<()> {
    let initial = VecDeque::from([0x85, 0x86, 0x87]);
    assert_board(&g, &initial, true);
    assert_eq!(g.get("STATE"), 1);
    let frame = g.get("FRAME");
    for _ in 0..30 {
        g.loop_once()?;
    }
    assert_eq!(g.get("FRAME"), frame, "Starts paused");
    g.press(Key::Space)?;
    let mut body = initial.clone();
    // Natural timed moves, including eating the initial food and finding another.
    for _ in 0..3 {
        let next = body.back().unwrap() + 1;
        let growing = next == g.get("FOOD");
        if !growing {
            body.pop_front();
        }
        body.push_back(next);
        g.frame()?;
        assert_board(&g, &body, true);
    }
    assert_eq!(g.get("LEN"), 4);
    assert_eq!(g.m.io("debug"), 1);
    // Reverse is refused; only the first perpendicular turn is accepted per move.
    g.press(Key::ArrowLeft)?;
    g.press(Key::ArrowUp)?;
    g.press(Key::ArrowLeft)?;
    assert_eq!(g.get("NEXTDIR"), 3);
    body.pop_front();
    body.push_back(body.back().unwrap() - 16);
    g.due_frame()?;
    assert_board(&g, &body, true);
    g.press(Key::Space)?;
    let frame = g.get("FRAME");
    let moves = g.get("MOVES");
    for _ in 0..50 {
        g.loop_once()?;
    }
    assert_eq!(
        (g.get("STATE"), g.get("FRAME"), g.get("MOVES")),
        (1, frame, moves)
    );
    g.press(Key::Enter)?;
    assert_board(&g, &initial, true);
    assert_eq!((g.get("STATE"), g.get("MOVES")), (1, 0));

    // Ring wrap and entering the tail cell vacated by this very move.
    setup(&mut g, &[0x11, 0x12, 0x22, 0x21], 254, 3, 0x90)?;
    g.due_frame()?;
    assert_board(&g, &VecDeque::from([0x12, 0x22, 0x21, 0x11]), false);
    assert_eq!((g.get("TAIL"), g.get("STATE")), (255, 0));
    assert_eq!(g.m.pixel(1, 1), WHITE);
    // A non-tail occupied cell is a collision; body remains unchanged.
    setup(&mut g, &[0x11, 0x12, 0x22, 0x21], 0, 0, 0x90)?;
    g.due_frame()?;
    assert_eq!(g.get("STATE"), 2);
    assert_board(&g, &VecDeque::from([0x11, 0x12, 0x22, 0x21]), false);
    assert_eq!(g.m.pixel(1, 2), RED);
    for (cells, dir) in [
        ([0x0d, 0x0e, 0x0f], 0),
        ([0xd5, 0xe5, 0xf5], 1),
        ([0x32, 0x31, 0x30], 2),
        ([0x25, 0x15, 0x05], 3),
    ] {
        setup(&mut g, &cells, 0, dir, 0x90)?;
        g.due_frame()?;
        assert_eq!(g.get("STATE"), 2);
    }
    // One remaining free cell: food search must terminate and select it.
    // Then consume it to win; the 255-word ring crosses its end.
    let mut path = Vec::new();
    for y in 0..16 {
        for i in 0..16 {
            path.push(y * 16 + if y % 2 == 0 { i } else { 15 - i });
        }
    }
    setup(&mut g, &path[..254], 250, 2, path[254])?;
    g.due_frame()?;
    assert_eq!(
        (g.get("LEN"), g.get("FOOD"), g.get("STATE")),
        (255, path[255], 0)
    );
    assert_board(&g, &path[..255].iter().copied().collect(), false);
    g.due_frame()?;
    assert_eq!((g.get("LEN"), g.get("STATE")), (256, 3));
    assert_eq!(g.m.io("debug"), 0x30fd);
    for y in 0..16 {
        for x in 0..16 {
            assert_eq!(g.m.pixel(x, y), GREEN);
        }
    }
    let frame = g.get("FRAME");
    for _ in 0..20 {
        g.loop_once()?;
    }
    assert_eq!(g.get("FRAME"), frame);
    g.press(Key::Enter)?;
    assert_board(&g, &initial, true);
    // Physical reset must restart software, clear old RAM and return to paused.
    g.m.reset()?;
    g.at_loop()?;
    assert_board(&g, &initial, true);
    assert_eq!((g.get("STATE"), g.get("MOVES")), (1, 0));
    Ok(())
}
fn setup(g: &mut Game<'_>, body: &[u16], tail: u16, direction: u16, food: u16) -> Result<()> {
    for a in 0..256 {
        g.m.set_word(0x200 + a, 0)?;
    }
    for (i, &p) in body.iter().enumerate() {
        g.m.set_word(0x100 + ((tail as u32 + i as u32) & 255), p)?;
        g.m.set_word(0x200 + u32::from(p), 1)?;
    }
    for (name, value) in [
        ("LEN", body.len() as u16),
        ("TAIL", tail),
        ("HEAD", *body.last().unwrap()),
        ("DIR", direction),
        ("NEXTDIR", direction),
        ("FOOD", food),
        ("STATE", 0),
        ("TURNED", 0),
    ] {
        g.set(name, value)?;
    }
    Ok(())
}
fn assert_board(g: &Game<'_>, body: &VecDeque<u16>, pixels: bool) {
    assert_eq!(g.get("LEN") as usize, body.len());
    assert_eq!(g.get("HEAD"), *body.back().unwrap());
    for (i, &p) in body.iter().enumerate() {
        assert_eq!(
            g.m.word(0x100 + ((u32::from(g.get("TAIL")) + i as u32) & 255)),
            p
        );
    }
    let food = g.get("FOOD");
    if body.len() < 256 {
        assert!(!body.contains(&food));
    }
    for p in 0..256 {
        let occupied = body.contains(&p);
        assert_eq!(
            g.m.word(0x200 + u32::from(p)),
            u16::from(occupied),
            "occupancy {p:02x}"
        );
        if pixels {
            let color = if p == *body.back().unwrap() {
                WHITE
            } else if occupied {
                GREEN
            } else if p == food {
                RED
            } else {
                BLACK
            };
            assert_eq!(
                g.m.pixel((p & 15) as usize, (p >> 4) as usize),
                color,
                "pixel {p:02x}"
            );
        }
    }
}
