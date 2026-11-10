//! The game is a synchronous circuit: every expression becomes gates/plexers/arithmetic.
use crate::builder::{Builder, E, any};
use logisim_core::*;
pub struct Start {
    pub body: Vec<u8>,
    pub direction: u32,
    pub delay: u32,
}
impl Default for Start {
    fn default() -> Self {
        Self {
            body: vec![0x85, 0x86, 0x87],
            direction: 0,
            delay: 63,
        }
    }
}
pub fn build(start: &Start) -> Circuit {
    assert!(
        !start.body.is_empty()
            && start.body.len() <= 256
            && start.direction < 4
            && start.delay < 256
    );
    let distinct: std::collections::BTreeSet<_> = start.body.iter().collect();
    assert_eq!(distinct.len(), start.body.len());
    for pair in start.body.windows(2) {
        let [a, b] = [pair[0], pair[1]];
        assert_eq!((a & 15).abs_diff(b & 15) + (a >> 4).abs_diff(b >> 4), 1);
    }
    let mut b = Builder::new("Snake engine / editable state machine and memories");
    b.input("CLK", 1);
    let key = b.input("KEY", 3);
    let down = b.input("DOWN", 1);
    let valid = b.input("VALID", 1);
    let s = E::net("phase", 5);
    let init = s.is(0);
    let boot = s.is(1);
    let seed = s.is(4);
    let search = s.is(5);
    let food_draw = s.is(6);
    let present = s.is(7);
    let wait = s.is(8);
    let check = s.is(9);
    let erase = s.is(10);
    let write = s.is(11);
    let post = s.is(12);
    let finish_frame = s.is(13);
    let won = s.is(15);
    let dead = s.is(16);
    let zero = E::num(1, 0);
    let one = E::num(1, 1);
    let head = E::net("head", 8);
    let dir = E::net("dir", 2);
    let paused = E::net("paused", 1);
    let locked = E::net("turn_lock", 1);
    let len = E::net("length", 9);
    let index = E::net("init_idx", 8);
    let initial = E::num(9, start.body.len() as u32);
    let last = index.is(start.body.len() as u32 - 1);
    let boot_done = boot.and(&last);
    let tailptr = E::net("tail_ptr", 8);
    let writeptr = E::net("write_ptr", 8);
    let tail = E::net("body", 8);
    let oldtail = E::net("old_tail", 8);
    let nexthead = E::net("next_head", 8);
    let ate = E::net("ate", 1);
    let food = E::net("food", 8);
    let candidate = E::net("candidate", 8);
    let occupied = E::net("occupied", 1);
    let tries = E::net("tries", 8);
    let timer = E::net("timer", 8);
    b.section("1. Input handshake / one turn per move");
    let arrow = key.slice(2, 1).not();
    let wanted = key.is(0).choose(
        &E::num(2, 3),
        &key.is(1).choose(
            &E::num(2, 1),
            &key.is(2).choose(&E::num(2, 2), &E::num(2, 0)),
        ),
    );
    let turn = wanted
        .eq(&dir)
        .not()
        .and(&wanted.eq(&dir.xor(&E::num(2, 2))).not());
    let blocked = wait
        .and(&paused.not())
        .and(&locked)
        .and(&arrow)
        .and(&down)
        .and(&turn);
    let consume = any(&[wait.clone(), dead.clone(), won.clone()])
        .and(&valid)
        .and(&blocked.not());
    let press = consume.and(&down);
    let restart = press.and(&key.is(5));
    let pause = press.and(&key.is(4)).and(&wait);
    let change = press
        .and(&arrow)
        .and(&turn)
        .and(&wait)
        .and(&paused.not())
        .and(&locked.not());
    b.register("ack", &consume, &one, &init);
    b.output("READ", &E::net("ack", 1));
    b.output("RESET", &init);
    b.register(
        "dir",
        &boot_done.choose(&E::num(2, start.direction), &wanted),
        &boot_done.or(&change),
        &init,
    );
    b.register("paused", &paused.not(), &pause, &init);
    b.register(
        "turn_lock",
        &present.or(&pause).choose(&zero, &one),
        &any(&[present.clone(), pause.clone(), change]),
        &init,
    );
    b.register(
        "timer",
        &wait.choose(&timer.add(&E::num(8, 1)), &E::num(8, 0)),
        &paused.not().or(&wait.not()),
        &init,
    );
    b.section("2. Coordinates / boundary and tail-aware collision");
    let x = head.slice(0, 4);
    let y = head.slice(4, 4);
    let nx = dir.is(0).choose(
        &x.add(&E::num(4, 1)),
        &dir.is(2).choose(&x.sub(&E::num(4, 1)), &x),
    );
    let ny = dir.is(1).choose(
        &y.add(&E::num(4, 1)),
        &dir.is(3).choose(&y.sub(&E::num(4, 1)), &y),
    );
    let target = E::join(&ny, &nx);
    let eating = target.eq(&food);
    let wall = any(&[
        dir.is(0).and(&x.is(15)),
        dir.is(2).and(&x.is(0)),
        dir.is(1).and(&y.is(15)),
        dir.is(3).and(&y.is(0)),
    ]);
    let collision = wall.or(&occupied.and(&target.eq(&tail).and(&eating.not()).not()));
    let pass = check.and(&collision.not());
    b.register("next_head", &target, &pass, &init);
    b.register("old_tail", &tail, &pass, &init);
    b.register("ate", &eating, &pass, &init);
    let initial_cell = E::net("initial_cell", 8);
    b.register(
        "head",
        &boot_done.choose(&initial_cell, &nexthead),
        &boot_done.or(&write),
        &init,
    );
    b.register(
        "length",
        &boot_done.choose(&initial, &len.add(&E::num(9, 1))),
        &boot_done.or(&write.and(&ate)),
        &init,
    );
    b.register(
        "write_ptr",
        &boot_done.choose(&initial.slice(0, 8), &writeptr.add(&E::num(8, 1))),
        &boot_done.or(&write),
        &init,
    );
    b.register(
        "tail_ptr",
        &tailptr.add(&E::num(8, 1)),
        &write.and(&ate.not()),
        &init,
    );
    b.section("3. Initial path ROM / body ring RAM / occupancy RAM");
    let mut rom = Component::new(Kind::Rom, Point::ZERO);
    rom.label = "Initial path (tail to head)".into();
    rom.memory.select_high = true;
    for (i, v) in start.body.iter().enumerate() {
        rom.memory.contents.set(i as u32, u32::from(*v));
    }
    let id = b.add(rom);
    b.attach(id, 0, "initial_cell", true);
    b.connect(id, 1, &index);
    b.connect(id, 2, &one);
    b.register("init_idx", &index.add(&E::num(8, 1)), &boot, &init);
    b.memory(
        "body",
        8,
        &boot.choose(&index, &write.choose(&writeptr, &tailptr)),
        &boot.choose(&initial_cell, &nexthead),
        &boot.or(&write),
        &init,
    );
    let occ_address = boot.choose(
        &initial_cell,
        &search.choose(
            &candidate,
            &check.choose(&target, &erase.choose(&oldtail, &nexthead)),
        ),
    );
    let tail_remove = erase.and(&ate.not());
    b.memory(
        "occupied",
        1,
        &occ_address,
        &erase.choose(&zero, &one),
        &any(&[boot.clone(), write.clone(), tail_remove.clone()]),
        &init,
    );
    b.section("4. Random candidate / bounded free-cell scan");
    let mut rng = Component::new(Kind::Random, Point::ZERO);
    rng.label = "random".into();
    rng.sequential.random_seed = 12345;
    let id = b.add(rng);
    b.attach(id, 0, "random", true);
    b.connect(id, 1, &one);
    b.connect(id, 2, &E::net("CLK", 1));
    b.connect(id, 3, &init);
    let advance = search.and(&occupied);
    let found = search.and(&occupied.not());
    b.register(
        "candidate",
        &seed.choose(&E::net("random", 8), &candidate.add(&E::num(8, 1))),
        &seed.or(&advance),
        &init,
    );
    b.register(
        "tries",
        &tries.add(&E::num(8, 1)),
        &advance,
        &init.or(&seed),
    );
    b.register("food", &candidate, &found, &init);
    b.section("5. State machine / explicit frame presentation");
    let ready = wait
        .and(&timer.is(start.delay))
        .and(&paused.not())
        .and(&pause.not());
    let after_boot = if start.body.len() == 256 { 13 } else { 4 };
    let choices = [
        (init.clone(), E::num(5, 1)),
        (boot_done.clone(), E::num(5, after_boot)),
        (seed.clone(), E::num(5, 5)),
        (
            search.clone(),
            occupied.choose(
                &tries.is(255).choose(&E::num(5, 13), &E::num(5, 5)),
                &E::num(5, 6),
            ),
        ),
        (food_draw.clone(), E::num(5, 7)),
        (present.clone(), E::num(5, 8)),
        (ready, E::num(5, 9)),
        (
            check.clone(),
            collision.choose(&E::num(5, 16), &E::num(5, 10)),
        ),
        (erase.clone(), E::num(5, 11)),
        (finish_frame.clone(), E::num(5, 15)),
        (write.clone(), E::num(5, 12)),
        (
            post.clone(),
            ate.choose(
                &len.is(256).choose(&E::num(5, 13), &E::num(5, 4)),
                &E::num(5, 7),
            ),
        ),
    ];
    let next = choices
        .into_iter()
        .rev()
        .fold(s.clone(), |other, (cond, next)| cond.choose(&next, &other));
    b.register("phase", &restart.choose(&E::num(5, 0), &next), &one, &zero);
    let draw_at = boot.choose(
        &initial_cell,
        &food_draw.choose(&food, &erase.choose(&oldtail, &nexthead)),
    );
    b.output("X", &draw_at.slice(0, 4).widen(5));
    b.output("Y", &draw_at.slice(4, 4).widen(5));
    b.output(
        "COLOR",
        &food_draw.choose(
            &E::num(8, 224),
            &init.or(&erase).choose(&E::num(8, 0), &E::num(8, 28)),
        ),
    );
    b.output(
        "WRITE",
        &any(&[boot.clone(), food_draw, tail_remove, write]),
    );
    b.output("CLEAR", &init);
    b.output("PRESENT", &present.or(&finish_frame));
    b.output("PAUSED", &paused);
    b.output("DEAD", &dead);
    b.output("WON", &won);
    b.output(
        "SCORE",
        &init
            .or(&boot)
            .choose(&E::num(8, 0), &len.sub(&initial).slice(0, 8)),
    );
    b.finish()
}
