//! Observe the executed subgraph of one compiled instance at a time.
//! Unit-cost work and span describe available parallelism, independently of wall time.
use super::Program;
use std::{collections::BTreeSet, time::Duration};

pub struct Layer {
    pub instructions: usize,
    pub regions: usize,
}

pub struct Pass {
    pub layers: Vec<Layer>,
    pub active_regions: usize,
    pub excluded: usize,
    pub body: Duration,
}

pub struct Trace {
    pub dispatches: usize,
    pub dispatched_nodes: usize,
    pub barrier: Duration,
    pub longest_lane: Duration,
    pub topology: Pass,
    pub passes: Vec<Pass>,
}

struct Graph {
    predecessors: Vec<Vec<usize>>,
    region: Vec<Option<usize>>,
}

impl Graph {
    fn new(program: &Program) -> Self {
        let n = program.instructions.len();
        let mut predecessors = vec![Vec::new(); n];
        let mut edges = vec![Vec::new(); n];
        let eligible = |i: usize| program.instructions[i].operation.dataflow_eligible();
        for (i, inst) in program.instructions.iter().enumerate() {
            for slot in inst.outputs.iter().flatten() {
                for consumer in &program.consumers[*slot] {
                    let j = consumer.instruction;
                    debug_assert!(i < j);
                    predecessors[j].push(i);
                    if eligible(i) && eligible(j) {
                        edges[i].push(j);
                    }
                }
            }
        }
        let (feedback, groups) = crate::combinational::graph::partition(&edges);
        debug_assert!(feedback.is_empty());
        let mut region = vec![None; n];
        for (id, group) in groups.iter().enumerate() {
            for &i in group {
                region[i] = eligible(i).then_some(id);
            }
        }
        Self {
            predecessors,
            region,
        }
    }

    fn pass(&self, active: &[usize], body: Duration) -> Pass {
        let mut depths = vec![0; self.region.len()];
        let mut layers: Vec<(usize, BTreeSet<usize>)> = Vec::new();
        let mut regions = BTreeSet::new();
        let mut excluded = 0;
        for &i in active {
            let prior = self.predecessors[i]
                .iter()
                .map(|&p| depths[p])
                .max()
                .unwrap_or(0);
            let Some(region) = self.region[i] else {
                // An excluded ROM/PLA contributes no parallel work, but preserves precedence.
                depths[i] = prior;
                excluded += 1;
                continue;
            };
            depths[i] = prior + 1;
            if layers.len() == prior {
                layers.push(Default::default());
            }
            layers[prior].0 += 1;
            layers[prior].1.insert(region);
            regions.insert(region);
        }
        Pass {
            layers: layers
                .into_iter()
                .map(|(instructions, r)| Layer {
                    instructions,
                    regions: r.len(),
                })
                .collect(),
            active_regions: regions.len(),
            excluded,
            body,
        }
    }
}

pub(super) struct Observer {
    graph: Graph,
    pub(super) active: Vec<usize>,
    pub(super) trace: Trace,
}
impl Observer {
    pub(super) fn new(program: &Program) -> Self {
        let graph = Graph::new(program);
        let mut active: Vec<_> = (0..program.instructions.len()).collect();
        let topology = graph.pass(&active, Duration::ZERO);
        active.clear();
        Self {
            graph,
            active,
            trace: Trace {
                dispatches: 0,
                dispatched_nodes: 0,
                barrier: Duration::ZERO,
                longest_lane: Duration::ZERO,
                topology,
                passes: Vec::new(),
            },
        }
    }

    pub(super) fn finish(&mut self, body: Duration) {
        self.trace.passes.push(self.graph.pass(&self.active, body));
        self.active.clear();
    }
}
