use super::super::*;
use logisim_core::{Component, Kind, Point, Project, Trigger};

fn settle(s: &mut Simulator) {
    s.advance(128);
    assert_eq!(s.pending_count(), 0);
    assert!(!s.limited());
}
fn fixture(seed: i32, bits: u8, trigger: Trigger) -> (Circuit, Id, [Id; 3], Simulator) {
    let mut c = Component::new(Kind::Random, Point::new(200, 200));
    c.sequential.random_seed = seed;
    c.sequential.trigger = trigger;
    c.bits = bits;
    let mut circuit = Circuit::default();
    let pins = [1, 2, 3].map(|i| circuit.add_component(Component::new(Kind::Pin, c.ports()[i])));
    let id = circuit.add_component(c);
    let mut s = Simulator::default();
    s.synchronize(&circuit);
    settle(&mut s);
    (circuit, id, pins, s)
}
fn drive(s: &mut Simulator, pin: Id, bit: Bit) {
    s.external_input(pin, Value::filled(1, bit));
    settle(s);
}
fn edge(s: &mut Simulator, pin: Id, trigger: Trigger) {
    let (a, b) = if trigger == Trigger::Falling {
        (Bit::One, Bit::Zero)
    } else {
        (Bit::Zero, Bit::One)
    };
    drive(s, pin, a);
    drive(s, pin, b);
}
fn output(s: &Simulator, id: Id, bits: u8, value: u32) {
    assert_eq!(s.port_value(id, 0), Some(Value::known(bits, value)));
    assert_eq!(s.stored_value(id), Some(Value::known(bits, value)));
}

#[test]
fn observed_seed_sequences_both_edges_and_width_truncation() {
    let samples = [
        (
            1,
            [
                0x005deece, 0xb61488df, 0xf4111591, 0x023eaf12, 0xb578fa6a, 0xd38a8b1c, 0xf5d50649,
                0x202e3c08,
            ],
        ),
        (
            12345,
            [
                0xb1b0df3b, 0xc5abf83e, 0x41c70961, 0x9fe4e792, 0xafdba542, 0xeaa904d3, 0x86079727,
                0x7bcc1d18,
            ],
        ),
        (
            -1,
            [
                0xffa21131, 0x51fdfcdd, 0x60f97439, 0xa4cfc986, 0xf5595b32, 0x18a3a1e7, 0xfe452c02,
                0x771a5754,
            ],
        ),
        (
            i32::MIN,
            [
                0xcc980000, 0xb0c142de, 0x84dd44e5, 0xdaff3c4c, 0xd3812ace, 0x364f1682, 0xa1e51925,
                0xd29c49ae,
            ],
        ),
    ];
    for bits in [1, 4, 8, 16, 24, 32] {
        for trigger in [Trigger::Rising, Trigger::Falling] {
            for (seed, expected) in samples {
                let (_, id, p, mut s) = fixture(seed, bits, trigger);
                output(&s, id, bits, seed as u32);
                drive(&mut s, p[0], Bit::One);
                for n in expected {
                    edge(&mut s, p[1], trigger);
                    output(&s, id, bits, n);
                }
                drive(&mut s, p[2], Bit::One);
                output(&s, id, bits, seed as u32);
                edge(&mut s, p[1], trigger);
                output(&s, id, bits, seed as u32);
                drive(&mut s, p[2], Bit::Zero);
                output(&s, id, bits, seed as u32);
                edge(&mut s, p[1], trigger);
                output(&s, id, bits, expected[0]);
            }
        }
    }
}

#[test]
fn four_state_enable_clear_priority_and_unknown_clock_breaks_edges() {
    use Bit::*;
    for trigger in [Trigger::Rising, Trigger::Falling] {
        for enable in [Zero, One, Unknown, Error] {
            for clear in [Zero, One, Unknown, Error] {
                let (_, id, p, mut s) = fixture(1, 32, trigger);
                drive(&mut s, p[0], enable);
                drive(&mut s, p[2], clear);
                edge(&mut s, p[1], trigger);
                output(
                    &s,
                    id,
                    32,
                    if enable == Zero || clear == One {
                        1
                    } else {
                        0x005deece
                    },
                );
            }
        }
        let (_, id, p, mut s) = fixture(1, 32, trigger);
        drive(&mut s, p[0], One);
        for uncertain in [Unknown, Error] {
            let (start, end) = if trigger == Trigger::Rising {
                (Zero, One)
            } else {
                (One, Zero)
            };
            drive(&mut s, p[1], start);
            drive(&mut s, p[1], uncertain);
            drive(&mut s, p[1], end);
            output(&s, id, 32, 1);
        }
        edge(&mut s, p[1], trigger);
        output(&s, id, 32, 0x005deece);
    }
}

#[test]
fn seed_and_width_edits_preserve_sequence_until_reset_and_label_edits_do_not_resample() {
    let (mut circuit, id, p, mut s) = fixture(12345, 8, Trigger::Rising);
    drive(&mut s, p[0], Bit::One);
    edge(&mut s, p[1], Trigger::Rising);
    output(&s, id, 8, 0x3b);
    let mut c = circuit.component(id).unwrap().clone();
    c.sequential.random_seed = 2;
    c.bits = 32;
    circuit.replace_component(c.clone());
    assert!(s.synchronize(&circuit));
    settle(&mut s);
    output(&s, id, 32, 0xb1b0df3b);
    c.label = "user label".into();
    circuit.replace_component(c);
    assert!(!s.synchronize(&circuit));
    edge(&mut s, p[1], Trigger::Rising);
    output(&s, id, 32, 0xc5abf83e);
    drive(&mut s, p[2], Bit::One);
    output(&s, id, 32, 2);
    drive(&mut s, p[2], Bit::Zero);
    edge(&mut s, p[1], Trigger::Rising);
    output(&s, id, 32, 0x00bbdd9c);
    s.reset();
    settle(&mut s);
    output(&s, id, 32, 2);
}

#[test]
fn automatic_seed_clear_is_bounded_and_pause_queues_without_advancing() {
    let (_, id, p, mut s) = fixture(0, 32, Trigger::Rising);
    drive(&mut s, p[0], Bit::One);
    drive(&mut s, p[2], Bit::One);
    let initial = s.port_value(id, 0).unwrap();
    for _ in 0..8 {
        edge(&mut s, p[1], Trigger::Rising);
        assert_eq!(s.port_value(id, 0), Some(initial));
    }
    let (_, id, p, mut s) = fixture(1, 32, Trigger::Rising);
    drive(&mut s, p[0], Bit::One);
    s.set_enabled(false);
    s.external_input(p[1], Value::known(1, 1));
    s.advance(128);
    output(&s, id, 32, 1);
    assert!(s.pending_count() > 0);
    s.set_enabled(true);
    settle(&mut s);
    output(&s, id, 32, 0x005deece);
}

#[test]
fn hierarchical_random_instances_are_independent_and_same_edge_register_samples_old_output() {
    let mut project = Project::single("main", Circuit::default());
    let child = project.add("generator".into()).unwrap();
    let mut random = Component::new(Kind::Random, Point::new(200, 200));
    random.sequential.random_seed = 1;
    random.bits = 32;
    let clock = Component::new(Kind::Clock, random.ports()[2]);
    let def = project.circuit_mut(child).unwrap();
    let clock_id = def.circuit.add_component(clock);
    let random_id = def.circuit.add_component(random);
    project.refresh_interfaces(false);
    let a = project.instance(child, Point::new(100, 100)).unwrap();
    let b = project.instance(child, Point::new(300, 100)).unwrap();
    let root = project.main;
    let main = &mut project.circuit_mut(root).unwrap().circuit;
    let a = main.add_component(a);
    let b = main.add_component(b);
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&project, root);
    sim.advance(256);
    sim.poke_clock(&[a], clock_id);
    sim.advance(256);
    output(sim.view(&[a]).unwrap(), random_id, 32, 0x005deece);
    output(sim.view(&[b]).unwrap(), random_id, 32, 1);

    let (mut circuit, id, p, mut s) = fixture(1, 32, Trigger::Rising);
    let mut register = Component::new(Kind::Register, Point::new(400, 200));
    register.bits = 32;
    let data = register.ports()[1];
    let clk = register.ports()[2];
    let reg = circuit.add_component(register);
    circuit.add_wire(Point::new(200, 200), data);
    circuit.add_wire(Point::new(170, 190), Point::new(140, 190));
    circuit.add_wire(Point::new(140, 190), Point::new(140, 230));
    circuit.add_wire(Point::new(140, 230), Point::new(380, 230));
    circuit.add_wire(Point::new(380, 230), clk);
    s.synchronize(&circuit);
    settle(&mut s);
    drive(&mut s, p[0], Bit::One);
    edge(&mut s, p[1], Trigger::Rising);
    output(&s, id, 32, 0x005deece);
    output(&s, reg, 32, 1);
    edge(&mut s, p[1], Trigger::Rising);
    output(&s, reg, 32, 0x005deece);
}
