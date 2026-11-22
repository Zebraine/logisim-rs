//! Hot trace only. Never pool nodes across instance, pass or state-commit boundaries.
use super::{Fixture, ProjectSimulator, Result, Value, json, measure};
use logisim_sim::combinational::dataflow::Pass;
use std::collections::BTreeMap;

#[derive(Default, serde::Serialize)]
struct Summary {
    calls: usize,
    eligible: usize,
    excluded: usize,
    span: usize,
    parallel_nodes: usize,
    active_regions_per_call: BTreeMap<usize, usize>,
    nodes_per_layer: BTreeMap<usize, usize>,
    regions_per_layer: BTreeMap<usize, usize>,
    ideal_layer_slots_1_2_4: [usize; 3],
    dispatch_layers_1_2_4: [usize; 3],
    loop_ns: u64,
    multi_node_call_ns: u64,
}
impl Summary {
    fn add(&mut self, pass: &Pass) {
        self.calls += 1;
        self.excluded += pass.excluded;
        self.span += pass.layers.len();
        let ns = pass.body.as_nanos() as u64;
        self.loop_ns += ns;
        if pass.layers.iter().any(|l| l.instructions > 1) {
            self.multi_node_call_ns += ns;
        }
        *self
            .active_regions_per_call
            .entry(pass.active_regions)
            .or_default() += 1;
        for layer in &pass.layers {
            let n = layer.instructions;
            self.eligible += n;
            if n > 1 {
                self.parallel_nodes += n;
            }
            *self.nodes_per_layer.entry(n).or_default() += 1;
            *self.regions_per_layer.entry(layer.regions).or_default() += 1;
            for (i, workers) in [1, 2, 4].into_iter().enumerate() {
                self.ideal_layer_slots_1_2_4[i] += n.div_ceil(workers);
                self.dispatch_layers_1_2_4[i] += usize::from(n.min(workers) > 1);
            }
        }
    }
}

pub fn observe(
    f: &Fixture,
    sim: &mut ProjectSimulator,
    reference: &measure::Trace,
) -> Result<Value> {
    measure::reset(f, sim)?;
    for (_, eval) in sim.study_evaluators() {
        eval.start_dataflow_study();
    }
    let observed = measure::trace(f, sim)?;
    if observed.trace != *reference {
        return Err("Dataflow observation changed the trace".into());
    }
    let mut total = Summary::default();
    let mut instances = vec![];
    for (path, eval) in sim.study_evaluators() {
        let trace = eval
            .finish_dataflow_study()
            .ok_or("Missing prepared evaluator trace")?;
        let mut local = Summary::default();
        for pass in &trace.passes {
            local.add(pass);
            total.add(pass);
        }
        let static_layers: Vec<_> = trace
            .topology
            .layers
            .iter()
            .map(|l| [l.instructions, l.regions])
            .collect();
        instances
            .push(json!({"path":path,"static_layers_nodes_regions": static_layers, "hot":local}));
    }
    if total.eligible + total.excluded != reference.work.evaluated_instructions {
        return Err("Observed instruction count disagrees with stable report".into());
    }
    Ok(json!({"total": total, "instances":instances}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn hot_layers_distinguish_sparse_lanes_from_a_serial_chain() -> Result<()> {
        for (name, static_width, depth) in [("wide", 64, 1), ("deep", 1, 318)] {
            let mut opened = crate::open(Fixture::load(name, Path::new(".")))?;
            let reference = measure::trial(&opened.fixture, &mut opened.sim)?.trace;
            let shape = observe(&opened.fixture, &mut opened.sim, &reference)?;
            let layers = shape["instances"][0]["static_layers_nodes_regions"]
                .as_array()
                .unwrap();
            assert_eq!(layers.len(), depth);
            assert_eq!(layers[0][0], static_width);
            assert_eq!(shape["total"]["eligible"], 512 * depth);
            assert_eq!(shape["total"]["span"], 512 * depth);
            assert_eq!(shape["total"]["parallel_nodes"], 0);
        }
        Ok(())
    }
}
