//! Word-addressed memory and reusable memory-mapped peripherals, independent of software.
use crate::builder::{Builder, E, hierarchy};
use logisim_core::*;
pub const KEY_HELD: u32 = 0xff00;
pub const KEY_EVENT: u32 = 0xff01;
pub const KEY_ACK: u32 = 0xff02;
pub const KEY_CLEAR: u32 = 0xff03;
pub const X: u32 = 0xff10;
pub const Y: u32 = 0xff11;
pub const COLOR: u32 = 0xff12;
pub const COMMAND: u32 = 0xff13;
pub const TIMER: u32 = 0xff20;
pub const RANDOM: u32 = 0xff21;
pub const DEBUG: u32 = 0xff30;
pub fn define(project: &mut Project) -> Result<Id, String> {
    let mut b = Builder::new("G16 bus / RAM 0000-0fff / IO ff00-ff30 / unmapped reads zero");
    b.input("CLK", 1);
    let reset = b.input("RESET", 1);
    let address = b.input("ADDR", 16);
    let data = b.input("WDATA", 16);
    let wr = b.input("WR", 1).and(&reset.not());
    let held = b.input("KEY_HELD", 8);
    let key = b.input("KEY", 3);
    let down = b.input("KEY_DOWN", 1);
    let valid = b.input("KEY_VALID", 1);
    let event = E::join(&valid, &E::join(&E::num(11, 0), &E::join(&down, &key)));
    let store = |a| wr.and(&address.is(a));
    b.section("1. Data RAM / 4096 x 16 / reset clears contents");
    let ram = address.slice(12, 4).is(0);
    b.memory(
        "RAM",
        16,
        &address.slice(0, 12),
        &data,
        &wr.and(&ram),
        &reset,
    );
    b.section("2. Peripheral registers / command strobes held for one cycle");
    for (name, bits, a) in [
        ("x", 5, X),
        ("y", 5, Y),
        ("color", 8, COLOR),
        ("debug", 16, DEBUG),
    ] {
        b.register(name, &data.slice(0, bits), &store(a), &reset);
    }
    b.register(
        "command",
        &store(COMMAND).choose(&data.slice(0, 3), &E::num(3, 0)),
        &E::num(1, 1),
        &reset,
    );
    b.register("key_ack", &store(KEY_ACK), &E::num(1, 1), &reset);
    b.register("key_clear", &store(KEY_CLEAR), &E::num(1, 1), &reset);
    b.section("3. Cycle counter and random source");
    let timer = E::net("timer", 16);
    b.register("timer", &timer.add(&E::num(16, 1)), &E::num(1, 1), &reset);
    let mut random = Component::new(Kind::Random, Point::ZERO);
    random.bits = 16;
    random.label = "random".into();
    random.sequential.random_seed = 12345;
    let id = b.add(random);
    b.attach(id, 0, "random", true);
    b.connect(id, 1, &E::num(1, 1));
    b.connect(id, 2, &E::net("CLK", 1));
    b.connect(id, 3, &reset);
    b.section("4. Read bus / display and keyboard interface");
    let values = [
        (KEY_HELD, held.widen(16)),
        (KEY_EVENT, event),
        (X, E::net("x", 5).widen(16)),
        (Y, E::net("y", 5).widen(16)),
        (COLOR, E::net("color", 8).widen(16)),
        (TIMER, timer),
        (RANDOM, E::net("random", 16)),
        (DEBUG, E::net("debug", 16)),
    ];
    let result = values
        .into_iter()
        .rev()
        .fold(E::num(16, 0), |other, (a, value)| {
            address.is(a).choose(&value, &other)
        });
    b.output("RDATA", &ram.choose(&E::net("RAM", 16), &result));
    for (port, name, bits) in [
        ("X", "x", 5),
        ("Y", "y", 5),
        ("COLOR", "color", 8),
        ("DEBUG", "debug", 16),
        ("KEY_ACK", "key_ack", 1),
    ] {
        b.output(port, &E::net(name, bits));
    }
    b.output("KEY_CLEAR", &E::net("key_clear", 1).or(&reset));
    let command = E::net("command", 3);
    b.output("WRITE", &command.slice(0, 1));
    b.output("CLEAR", &command.slice(1, 1).or(&reset));
    b.output("PRESENT", &command.slice(2, 1).or(&reset));
    hierarchy::define(project, "G16 memory and IO", b.finish())
}
