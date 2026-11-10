use super::*;

pub(super) fn check(mut g: Game<'_>) -> Result<()> {
    let mut model = Pong::initial();
    model.assert(&g);
    let frame = g.get("FRAME");
    for _ in 0..30 {
        g.loop_once()?;
    }
    assert_eq!(g.get("FRAME"), frame);
    g.press(Key::Space)?;
    model.state = 0;
    g.m.key(Key::ArrowUp, true)?;
    g.m.key(Key::X, true)?;
    // Held keys, paddle clamps, score transitions, full-frame comparison.
    for i in 0..40 {
        model.step(-1, 1);
        if i < 2 {
            g.frame()?;
        } else {
            g.due_frame()?;
        }
        model.assert(&g);
    }
    assert_eq!((model.left, model.right), (2, 12));
    g.m.key(Key::ArrowDown, true)?;
    g.m.key(Key::Z, true)?;
    for _ in 0..4 {
        model.step(0, 0);
        g.due_frame()?;
        model.assert(&g);
    }
    for key in [Key::ArrowUp, Key::ArrowDown, Key::Z, Key::X] {
        g.m.key(key, false)?;
    }
    // Drain FIFO before checking pause rather than bypassing its ACK protocol.
    for _ in 0..12 {
        // Avoid a timed frame while draining unrelated release events.
        g.set("LAST", g.m.io("timer") as u16)?;
        g.loop_once()?;
    }
    g.press(Key::Space)?;
    model.state = 1;
    let frame = g.get("FRAME");
    for _ in 0..60 {
        g.loop_once()?;
    }
    assert_eq!(g.get("FRAME"), frame);
    model.assert(&g);
    g.press(Key::Enter)?;
    Pong::initial().assert(&g);

    // Independent game model, seeded at wall/corner/paddle/goal boundaries.
    for (x, y, dx, dy, left, right) in [
        (8, 2, 1, -1, 6, 6),
        (8, 15, -1, 1, 6, 6),
        (1, 3, -1, 1, 3, 6),
        (1, 5, -1, 1, 3, 6),
        (14, 3, 1, 1, 6, 3),
        (14, 5, 1, 1, 6, 3),
        (1, 2, -1, -1, 2, 6),
        (14, 15, 1, 1, 6, 12),
        (1, 2, -1, -1, 12, 6),
        (14, 15, 1, 1, 6, 2),
    ] {
        model = Pong {
            x,
            y,
            dx,
            dy,
            left,
            right,
            state: 0,
            divider: 1,
            ..Pong::initial()
        };
        model.seed(&mut g)?;
        model.step(0, 0);
        g.due_frame()?;
        model.assert(&g);
    }
    for left_wins in [true, false] {
        model = Pong {
            x: if left_wins { 14 } else { 1 },
            y: 3,
            dx: if left_wins { 1 } else { -1 },
            dy: -1,
            left: 12,
            right: 12,
            left_score: 6,
            right_score: 6,
            state: 0,
            divider: 1,
        };
        model.seed(&mut g)?;
        model.step(0, 0);
        g.due_frame()?;
        model.assert(&g);
        assert_eq!(model.state, if left_wins { 2 } else { 3 });
        let frame = g.get("FRAME");
        g.press(Key::Space)?;
        for _ in 0..30 {
            g.loop_once()?;
        }
        assert_eq!(
            g.get("FRAME"),
            frame,
            "Winning state is final until restart"
        );
        g.press(Key::Enter)?;
        Pong::initial().assert(&g);
    }
    g.m.reset()?;
    g.at_loop()?;
    Pong::initial().assert(&g);
    Ok(())
}
struct Pong {
    left: i16,
    right: i16,
    x: i16,
    y: i16,
    dx: i16,
    dy: i16,
    left_score: i16,
    right_score: i16,
    state: i16,
    divider: i16,
}
impl Pong {
    fn initial() -> Self {
        Self {
            left: 6,
            right: 6,
            x: 8,
            y: 8,
            dx: 1,
            dy: 1,
            left_score: 0,
            right_score: 0,
            state: 1,
            divider: 0,
        }
    }
    fn step(&mut self, left: i16, right: i16) {
        self.left = (self.left + left).clamp(2, 12);
        self.right = (self.right + right).clamp(2, 12);
        self.divider ^= 1;
        if self.divider != 0 {
            return;
        }
        let mut y = self.y + self.dy;
        if !(2..=15).contains(&y) {
            self.dy = -self.dy;
            y = self.y + self.dy;
        }
        let mut x = self.x + self.dx;
        if x == 0 || x == 15 {
            let paddle = if x == 0 { self.left } else { self.right };
            if (paddle..paddle + 4).contains(&y) {
                self.dx = -self.dx;
                x += self.dx;
                self.dy = if y - paddle < 2 { -1 } else { 1 };
            } else {
                if x == 0 {
                    self.right_score += 1;
                    self.dx = -1;
                } else {
                    self.left_score += 1;
                    self.dx = 1;
                }
                self.state = if self.left_score == 7 {
                    2
                } else if self.right_score == 7 {
                    3
                } else {
                    0
                };
                self.x = 8;
                self.y = 8;
                self.dy = 1;
                self.divider = 0;
                return;
            }
        }
        self.x = x;
        self.y = y;
    }
    fn values(&self) -> [(&'static str, u16); 10] {
        [
            ("LEFT", self.left as u16),
            ("RIGHT", self.right as u16),
            ("BX", self.x as u16),
            ("BY", self.y as u16),
            ("DX", self.dx as u16),
            ("DY", self.dy as u16),
            ("LSCORE", self.left_score as u16),
            ("RSCORE", self.right_score as u16),
            ("STATE", self.state as u16),
            ("DIVIDER", self.divider as u16),
        ]
    }
    fn seed(&self, g: &mut Game<'_>) -> Result<()> {
        for (n, v) in self.values() {
            g.set(n, v)?;
        }
        Ok(())
    }
    fn assert(&self, g: &Game<'_>) {
        for (n, v) in self.values() {
            assert_eq!(g.get(n), v, "Pong {n}");
        }
        assert_eq!(
            g.m.io("debug"),
            (self.state as u32) * 0x1000 + (self.left_score as u32) * 256 + self.right_score as u32
        );
        for y in 0..16i16 {
            for x in 0..16i16 {
                let color = if x == self.x && y == self.y {
                    match self.state {
                        2 => GREEN,
                        3 => BLUE,
                        _ => WHITE,
                    }
                } else if (x == 0 && (self.left..self.left + 4).contains(&y))
                    || (y == 0 && x < self.left_score)
                {
                    GREEN
                } else if (x == 15 && (self.right..self.right + 4).contains(&y))
                    || (y == 0 && x >= 16 - self.right_score)
                {
                    BLUE
                } else {
                    BLACK
                };
                assert_eq!(
                    g.m.pixel(x as usize, y as usize),
                    color,
                    "Pong pixel ({x},{y})"
                );
            }
        }
    }
}
