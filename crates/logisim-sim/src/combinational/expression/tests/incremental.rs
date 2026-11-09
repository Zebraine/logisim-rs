use super::super::*;
use super::support::{FOUR, component, fixture, link};
use logisim_core::{Arithmetic, Gate, Kind};

fn add(c: &mut Circuit, kind: Kind, width: u8) -> logisim_core::Id {
    let mut comp = component(kind, width);
    comp.position.x += c.components.len() as i32 * 200;
    c.add_component(comp)
}

#[test]
fn unchanged_outputs_cut_off_fanout_and_unrelated_regions_are_skipped() {
    let mut c = Circuit::default();
    let a = add(&mut c, Kind::Pin, 4);
    let b = add(&mut c, Kind::Pin, 4);
    let gate = add(&mut c, Kind::Gate(Gate::And), 4);
    let out = add(&mut c, Kind::Gate(Gate::Not), 4);
    let other = add(&mut c, Kind::Gate(Gate::Buffer), 4);
    link(&mut c, (a, 0), (gate, 1), "a");
    link(&mut c, (b, 0), (gate, 2), "b");
    link(&mut c, (gate, 0), (out, 1), "g");
    link(&mut c, (b, 0), (other, 1), "b");
    let plan = CompiledCircuit::new(&c);
    let mut fast = plan.evaluator();
    let mut full = plan.evaluator();
    for (x, y, count) in [(0, 0, 3), (15, 0, 1), (15, 1, 3), (15, 1, 0), (0, 1, 2)] {
        let read = |p: Port| Some(Value::known(4, if p.0 == a { x } else { y }));
        let before: BTreeMap<_, _> = fast.values().collect();
        fast.evaluate_cached(read).unwrap();
        full.evaluate(read).unwrap();
        assert_eq!(fast.stats().unwrap().instructions, count);
        assert_eq!(
            fast.values().collect::<Vec<_>>(),
            full.values().collect::<Vec<_>>()
        );
        let differences: BTreeMap<_, _> = full
            .values()
            .filter(|(p, v)| before.get(p) != Some(v))
            .collect();
        assert_eq!(
            fast.changed_values().collect::<BTreeMap<_, _>>(),
            differences
        );
        assert_eq!(fast.stats().unwrap().changed_outputs, differences.len());
    }
}

#[test]
fn unequal_paths_reconverge_once_after_all_changed_predecessors() {
    let mut c = Circuit::default();
    let input = add(&mut c, Kind::Pin, 1);
    let mut last = input;
    for depth in 0..12 {
        let buffer = add(&mut c, Kind::Gate(Gate::Buffer), 1);
        link(&mut c, (last, 0), (buffer, 1), &format!("path-{depth}"));
        last = buffer;
    }
    let xor = add(&mut c, Kind::Gate(Gate::Xor), 1);
    link(&mut c, (last, 0), (xor, 1), "end");
    link(&mut c, (input, 0), (xor, 2), "path-0");
    let plan = CompiledCircuit::new(&c);
    let mut fast = plan.evaluator();
    let mut full = plan.evaluator();
    for bit in FOUR.into_iter().chain(FOUR.into_iter().rev()) {
        let read = |_| Some(Value::filled(1, bit));
        fast.evaluate_cached(read).unwrap();
        full.evaluate(read).unwrap();
        assert!(fast.stats().unwrap().instructions <= 13);
        assert_eq!(
            fast.values().collect::<Vec<_>>(),
            full.values().collect::<Vec<_>>()
        );
    }
}

#[test]
fn multi_output_nodes_schedule_only_consumers_of_changed_ports() {
    let (mut c, adder, inputs) = fixture(component(Kind::Arithmetic(Arithmetic::Adder), 4));
    let sum = add(&mut c, Kind::Gate(Gate::Buffer), 4);
    let carry = add(&mut c, Kind::Gate(Gate::Buffer), 1);
    link(&mut c, (adder, 0), (sum, 1), "sum");
    link(&mut c, (adder, 4), (carry, 1), "carry");
    let plan = CompiledCircuit::new(&c);
    assert_eq!(plan.stats().instructions, 3);
    let mut fast = plan.evaluator();
    let mut full = plan.evaluator();
    for (value, count) in [(0, 3), (1, 2), (15, 3), (14, 3), (13, 2)] {
        let read = |p: Port| {
            let (_, width) = inputs.iter().find(|(id, _)| *id == p.0).unwrap();
            Some(Value::known(
                *width,
                if p.0 == inputs[0].0 {
                    value
                } else {
                    u32::from(*width > 1)
                },
            ))
        };
        fast.evaluate_cached(read).unwrap();
        full.evaluate(read).unwrap();
        assert_eq!(fast.stats().unwrap().instructions, count);
        assert_eq!(
            fast.values().collect::<Vec<_>>(),
            full.values().collect::<Vec<_>>()
        );
    }
}

#[test]
fn splitter_bit_dependencies_ignore_unused_changes_and_track_x_to_e() {
    let mut c = Circuit::default();
    let input = add(&mut c, Kind::Pin, 32);
    let mut split = component(Kind::Splitter, 32);
    split.splitter.fanout = 2;
    split.splitter.mapping = [Some(1); 32];
    split.splitter.mapping[0] = Some(0);
    split.splitter.mapping[31] = Some(0);
    let split = c.add_component(split);
    let gate = add(&mut c, Kind::Gate(Gate::Buffer), 2);
    link(&mut c, (input, 0), (split, 0), "bus");
    link(&mut c, (split, 1), (gate, 1), "ends");
    let mut fast = CompiledCircuit::new(&c).evaluator();
    for (value, count) in [
        (Value::known(32, 0), 1),
        (Value::known(32, 0x7fff_fffe), 0),
        (Value::known(32, 0xffff_fffe), 1),
        (
            Value::from_bits((0..32).map(|i| if i == 31 { Bit::Unknown } else { Bit::Zero })),
            1,
        ),
        (
            Value::from_bits((0..32).map(|i| if i == 31 { Bit::Error } else { Bit::Zero })),
            1,
        ),
    ] {
        fast.evaluate_cached(|_| Some(value)).unwrap();
        assert_eq!(fast.stats().unwrap().instructions, count);
        assert_eq!(
            fast.value((gate, 0)),
            Some(Value::from_bits([value.bit(0), value.bit(31)].into_iter()))
        );
    }
}

#[test]
fn failures_invalidate_queued_work_and_recovery_reports_folded_outputs() {
    let (mut c, gate, inputs) = fixture(component(Kind::Gate(Gate::And), 1));
    let constant = add(&mut c, Kind::Power, 1);
    let plan = CompiledCircuit::new(&c);
    let mut fast = plan.evaluator();
    for fail_width in [false, true] {
        fast.evaluate_cached(|_| Some(Value::known(1, 0))).unwrap();
        let result = fast.evaluate_cached(|p| {
            if p.0 == inputs[0].0 {
                Some(Value::known(1, 1))
            } else if fail_width {
                Some(Value::known(2, 0))
            } else {
                None
            }
        });
        assert!(result.is_err());
        assert_eq!(fast.values().count(), 0);
        assert_eq!(fast.changed_values().count(), 0);
        assert_eq!(fast.stats(), None);
        fast.evaluate_cached(|_| Some(Value::known(1, 1))).unwrap();
        assert_eq!(
            fast.stats().unwrap(),
            EvaluationStats {
                instructions: 1,
                changed_outputs: 2
            }
        );
        assert_eq!(
            fast.changed_values().collect::<BTreeMap<_, _>>(),
            BTreeMap::from([
                ((gate, 0), Value::known(1, 1)),
                ((constant, 0), Value::known(1, 1)),
            ])
        );
    }
    let mut separate = plan.evaluator();
    separate
        .evaluate_cached(|_| Some(Value::filled(1, Bit::Error)))
        .unwrap();
    assert_eq!(fast.value((gate, 0)), Some(Value::known(1, 1)));
}
