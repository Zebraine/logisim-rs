use super::{super::*, support::*};
use logisim_core::{Gate, Kind};

#[cfg(feature = "dataflow-study")]
#[test]
fn a_ready_rom_or_pla_keeps_its_entire_frontier_serial() {
    for kind in [Kind::Rom, Kind::PlaRom] {
        let (mut circuit, _, mut inputs) = fixture(component(kind, 4));
        for y in [600, 900] {
            let mut gate = component(Kind::Gate(Gate::Not), 4);
            gate.position.y = y;
            let mut pin = component(Kind::Pin, 4);
            pin.position = gate.ports()[1];
            inputs.push((circuit.add_component(pin), 4));
            circuit.add_component(gate);
        }
        let plan = CompiledCircuit::new(&circuit);
        let mut serial = plan.evaluator();
        let mut candidate = plan.evaluator();
        let pool = DataflowPool::for_threads(4).unwrap();
        candidate.set_dataflow_pool(pool.as_ref());
        candidate.start_dataflow_study();
        let read = |p: Port| {
            let (_, width) = inputs.iter().find(|(id, _)| *id == p.0)?;
            Some(Value::known(*width, 1))
        };
        assert_eq!(
            serial.evaluate_cached(read),
            candidate.evaluate_cached(read)
        );
        assert_eq!(
            serial.values().collect::<Vec<_>>(),
            candidate.values().collect::<Vec<_>>()
        );
        let trace = candidate.finish_dataflow_study().unwrap();
        assert_eq!(trace.passes.len(), 1);
        assert_eq!(trace.passes[0].excluded, 1);
        assert_eq!(trace.passes[0].layers[0].instructions, 2);
        assert_eq!(trace.dispatches, 0);
    }
}

#[test]
fn independent_lanes_reconverge_with_identical_internal_four_state_drives() {
    let mut c = Circuit::default();
    let mut inputs = Vec::new();
    let mut ends = Vec::new();
    for lane in 0..4 {
        let mut gate = component(Kind::Gate(Gate::Not), 4);
        gate.position.y += 200 * lane;
        let mut pin = component(Kind::Pin, 4);
        pin.position = gate.ports()[1];
        inputs.push(c.add_component(pin));
        ends.push(c.add_component(gate));
    }
    let mut xor = component(Kind::Gate(Gate::Xor), 4);
    xor.position.x += 500;
    let id = c.add_component(xor);
    link(&mut c, (ends[0], 0), (id, 1), "first");
    link(&mut c, (ends[3], 0), (id, 2), "last");
    let plan = CompiledCircuit::new(&c);
    for threads in [1, 2, 4] {
        let pool = DataflowPool::for_threads(threads).unwrap();
        let mut experiment = plan.evaluator();
        experiment.set_dataflow_pool(pool.as_ref());
        let mut serial = plan.evaluator();
        #[cfg(feature = "dataflow-study")]
        experiment.start_dataflow_study();
        for i in 0..16 {
            let read = |p: Port| {
                let lane = inputs.iter().position(|&id| id == p.0).unwrap();
                Some(Value::from_bits((0..4).map(|b| FOUR[(i + b + lane) % 4])))
            };
            assert_eq!(
                serial.evaluate_cached(read),
                experiment.evaluate_cached(read)
            );
            assert_eq!(
                experiment.values().collect::<Vec<_>>(),
                serial.values().collect::<Vec<_>>()
            );
            assert_eq!(
                experiment.changed_values().collect::<BTreeMap<_, _>>(),
                serial.changed_values().collect::<BTreeMap<_, _>>()
            );
            assert_eq!(serial.stats(), experiment.stats());
            if i == 7
                && let Some(pool) = &pool
            {
                pool.cancel();
            }
        }
        #[cfg(feature = "dataflow-study")]
        assert_eq!(
            experiment.finish_dataflow_study().unwrap().dispatches > 0,
            threads > 1
        );
    }
}
