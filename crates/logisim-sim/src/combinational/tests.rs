use super::*;
use crate::{Bit, Value};
use logisim_core::{Arithmetic, FlipFlop, Gate, Point};

fn add(c: &mut Circuit, kind: Kind, x: i32, y: i32, bits: u8) -> Id {
    let mut component = Component::new(kind, Point::new(x, y));
    component.bits = bits;
    c.add_component(component)
}
fn tunnel(c: &mut Circuit, port: Port, label: &str) -> Id {
    let component = c.component(port.0).unwrap();
    let mut tunnel = Component::new(Kind::Tunnel, component.ports()[port.1]);
    tunnel.bits = component.port_width(port.1).unwrap_or(1);
    tunnel.label = label.into();
    c.add_component(tunnel)
}
fn link(c: &mut Circuit, from: Port, to: Port, label: &str) {
    tunnel(c, from, label);
    tunnel(c, to, label);
}
fn source(port: Port, bit: u8) -> Source {
    Source { port, bit }
}
fn ids(analysis: &Analysis) -> Vec<Vec<Id>> {
    analysis
        .regions
        .iter()
        .map(|r| r.nodes.iter().map(|n| n.component).collect())
        .collect()
}
fn reasons(analysis: &Analysis, id: Id) -> Vec<BoundaryReason> {
    analysis
        .boundaries
        .iter()
        .filter(|b| b.component == id)
        .map(|b| b.reason)
        .collect()
}

#[test]
fn state_cuts_preserve_shared_nodes_topological_order_and_external_readers() {
    let mut c = Circuit::default();
    let state = add(&mut c, Kind::FlipFlop(FlipFlop::D), 0, 500, 1);
    // Reverse creation order exercises topological ordering, not document ordering.
    let b = add(&mut c, Kind::Gate(Gate::Not), 500, 100, 1);
    let d = add(&mut c, Kind::Gate(Gate::Buffer), 800, 100, 1);
    let a = add(&mut c, Kind::Gate(Gate::Buffer), 200, 100, 1);
    let probe = add(&mut c, Kind::Probe, 1000, 100, 1);
    link(&mut c, (state, 0), (a, 1), "old state");
    link(&mut c, (a, 0), (b, 1), "shared");
    tunnel(&mut c, (d, 1), "shared");
    link(&mut c, (b, 0), (state, 3), "next state");
    link(&mut c, (d, 0), (probe, 0), "observe");
    let analysis = Analysis::new(&c);
    assert_eq!(ids(&analysis), vec![vec![a, b, d]]);
    assert_eq!(reasons(&analysis, state), vec![BoundaryReason::State]);
    assert_eq!(analysis.regions[0].inputs, vec![source((state, 0), 0)]);
    assert_eq!(
        analysis.regions[0].outputs,
        vec![source((b, 0), 0), source((d, 0), 0)]
    );
    assert_eq!(
        analysis.regions[0].nodes[1].inputs[0].bits,
        vec![input_bit(vec![source((a, 0), 0)], Bit::Unknown)]
    );
    c.components.reverse();
    c.wires.reverse();
    for component in &mut c.components {
        if component.kind != Kind::Tunnel {
            component.label = "not auto-generated".into();
        }
    }
    assert_eq!(Analysis::new(&c), analysis);
}

#[test]
fn only_feedback_members_are_excluded_not_their_acyclic_consumers() {
    let mut c = Circuit::default();
    let a = add(&mut c, Kind::Gate(Gate::Not), 200, 100, 1);
    let b = add(&mut c, Kind::Gate(Gate::Not), 500, 100, 1);
    let downstream = add(&mut c, Kind::Gate(Gate::Buffer), 800, 100, 1);
    let own_loop = add(&mut c, Kind::Gate(Gate::Buffer), 200, 400, 1);
    link(&mut c, (a, 0), (b, 1), "forward");
    link(&mut c, (b, 0), (a, 1), "back");
    tunnel(&mut c, (downstream, 1), "forward");
    link(&mut c, (own_loop, 0), (own_loop, 1), "self");
    let analysis = Analysis::new(&c);
    assert_eq!(ids(&analysis), vec![vec![downstream]]);
    for id in [a, b, own_loop] {
        assert_eq!(reasons(&analysis, id), vec![BoundaryReason::Feedback]);
    }
    assert_eq!(analysis.regions[0].inputs, vec![source((a, 0), 0)]);
}

#[test]
fn splitters_and_tunnels_keep_source_bit_order_and_partial_exports() {
    let mut c = Circuit::default();
    let input = add(&mut c, Kind::Pin, 0, 0, 4);
    let gate = add(&mut c, Kind::Gate(Gate::Buffer), 300, 0, 4);
    link(&mut c, (input, 0), (gate, 1), "data");
    let mut splitter = Component::new(Kind::Splitter, Point::new(500, 300));
    splitter.bits = 4;
    splitter.splitter.fanout = 2;
    splitter.splitter.mapping = [None; 32];
    for (bit, branch) in [1, 0, 1, 0].into_iter().enumerate() {
        splitter.splitter.mapping[bit] = Some(branch);
    }
    let splitter = c.add_component(splitter);
    link(&mut c, (gate, 0), (splitter, 0), "bus");
    let low = add(&mut c, Kind::Gate(Gate::Not), 800, 200, 2);
    link(&mut c, (splitter, 1), (low, 1), "selected");
    let observe = add(&mut c, Kind::Probe, 1000, 400, 1);
    // Probe has inferred width: attach directly to the two-bit branch via a wire.
    let p = c.component(splitter).unwrap().ports()[2];
    c.add_wire(p, Point::new(1000, p.y));
    c.add_wire(
        Point::new(1000, p.y),
        c.component(observe).unwrap().position,
    );
    let analysis = Analysis::new(&c);
    assert_eq!(ids(&analysis), vec![vec![gate, low]]);
    assert_eq!(
        analysis.regions[0].nodes[1].inputs[0].bits,
        vec![
            input_bit(vec![source((gate, 0), 1)], Bit::Unknown),
            input_bit(vec![source((gate, 0), 3)], Bit::Unknown)
        ]
    );
    assert_eq!(
        analysis.regions[0].outputs,
        vec![source((gate, 0), 0), source((gate, 0), 2)]
    );
    assert_eq!(
        analysis.regions[0].inputs,
        (0..4)
            .map(|bit| source((input, 0), bit))
            .collect::<Vec<_>>()
    );
}

#[test]
fn topology_barriers_are_local_and_repaired_connections_become_candidates() {
    let mut c = Circuit::default();
    let good = add(&mut c, Kind::Gate(Gate::Buffer), 200, 0, 1);
    let conflict = add(&mut c, Kind::Gate(Gate::Buffer), 200, 200, 1);
    let pull = add(&mut c, Kind::Gate(Gate::Buffer), 200, 400, 1);
    let wide = add(&mut c, Kind::Gate(Gate::Buffer), 200, 600, 4);
    let mut removals = BTreeSet::new();
    let position = c.component(conflict).unwrap().ports()[1];
    for _ in 0..2 {
        removals.insert(add(&mut c, Kind::Pin, position.x, position.y, 1));
    }
    let position = c.component(pull).unwrap().ports()[1];
    removals.insert(add(&mut c, Kind::PullResistor, position.x, position.y, 1));
    let position = c.component(wide).unwrap().ports()[1];
    removals.insert(add(&mut c, Kind::Pin, position.x, position.y, 8));
    let analysis = Analysis::new(&c);
    assert_eq!(ids(&analysis), vec![vec![good], vec![conflict], vec![pull]]);
    assert_eq!(reasons(&analysis, conflict), vec![]);
    assert_eq!(reasons(&analysis, pull), vec![]);
    assert!(analysis.boundaries.contains(&Boundary {
        component: wide,
        reason: BoundaryReason::WidthMismatch,
        port: Some(1)
    }));
    c.remove(&removals);
    assert_eq!(
        ids(&Analysis::new(&c)),
        vec![vec![good], vec![conflict], vec![pull], vec![wide]]
    );
}

#[test]
fn different_bits_of_one_driver_shortened_by_splitters_are_multiple_sources() {
    let mut c = Circuit::default();
    let input = add(&mut c, Kind::Pin, 0, 100, 2);
    let mut split = Component::new(Kind::Splitter, Point::new(300, 100));
    split.bits = 2;
    split.splitter.fanout = 2;
    split.splitter.distribute(2, false);
    let split = c.add_component(split);
    link(&mut c, (input, 0), (split, 0), "two bits");
    link(&mut c, (split, 1), (split, 2), "short");
    let gate = add(&mut c, Kind::Gate(Gate::Buffer), 600, 100, 1);
    tunnel(&mut c, (gate, 1), "short");
    let analysis = Analysis::new(&c);
    assert_eq!(ids(&analysis), vec![vec![gate]]);
    assert_eq!(
        analysis.regions[0].nodes[0].inputs[0].bits,
        vec![input_bit(
            vec![source((input, 0), 0), source((input, 0), 1)],
            Bit::Unknown
        )]
    );
    assert_eq!(
        analysis.regions[0].inputs,
        vec![source((input, 0), 0), source((input, 0), 1)]
    );
}

#[test]
fn weak_pull_on_a_splitter_branch_only_supplies_that_bit() {
    let mut c = Circuit::default();
    let mut split = Component::new(Kind::Splitter, Point::new(300, 100));
    split.bits = 2;
    split.splitter.fanout = 2;
    split.splitter.distribute(2, false);
    let end = split.ports()[1];
    let split = c.add_component(split);
    add(&mut c, Kind::PullResistor, end.x, end.y, 1);
    let gate = add(&mut c, Kind::Gate(Gate::Buffer), 600, 100, 2);
    link(&mut c, (split, 0), (gate, 1), "combined");
    let analysis = Analysis::new(&c);
    assert_eq!(ids(&analysis), vec![vec![gate]]);
    assert_eq!(
        analysis.regions[0].nodes[0].inputs[0].bits,
        vec![
            input_bit(vec![], Bit::Zero),
            input_bit(vec![], Bit::Unknown)
        ]
    );
}

#[test]
fn physical_disconnection_and_connected_floating_are_distinct() {
    let mut c = Circuit::default();
    let gate = add(&mut c, Kind::Gate(Gate::Or), 200, 100, 1);
    let end = c.component(gate).unwrap().ports()[2];
    c.add_wire(end, end + Point::new(-100, 0));
    let analysis = Analysis::new(&c);
    let inputs = &analysis.regions[0].nodes[0].inputs;
    assert!(!inputs[0].connected);
    assert!(inputs[1].connected);
    assert_eq!(inputs[0].bits, vec![input_bit(vec![], Bit::Unknown)]);
    assert_eq!(inputs[1].bits, vec![input_bit(vec![], Bit::Unknown)]);
    assert!(analysis.regions[0].inputs.is_empty());
}

#[test]
fn hidden_state_and_opaque_components_are_boundaries_while_rom_is_pure() {
    let mut c = Circuit::default();
    for (i, kind) in [
        Kind::Plexer(Plexer::DisplayDecoder),
        Kind::Ram,
        Kind::Keyboard,
        Kind::Tty,
        Kind::DotMatrix,
        Kind::Plugin,
        Kind::Subcircuit(123),
        Kind::Button,
        Kind::Transistor,
    ]
    .into_iter()
    .enumerate()
    {
        add(&mut c, kind, i as i32 * 300, 500, 1);
    }
    let rom = add(&mut c, Kind::Rom, 200, 0, 4);
    let arithmetic = add(&mut c, Kind::Arithmetic(Arithmetic::Adder), 600, 0, 4);
    link(&mut c, (rom, 0), (arithmetic, 1), "read");
    let analysis = Analysis::new(&c);
    assert_eq!(ids(&analysis), vec![vec![rom, arithmetic]]);
    let reasons: Vec<_> = analysis.boundaries.iter().map(|b| b.reason).collect();
    assert_eq!(
        reasons,
        vec![
            BoundaryReason::State,
            BoundaryReason::State,
            BoundaryReason::State,
            BoundaryReason::State,
            BoundaryReason::State,
            BoundaryReason::Plugin,
            BoundaryReason::Hierarchy,
            BoundaryReason::Interaction,
            BoundaryReason::UnsupportedComponent
        ]
    );
}

#[test]
fn analysis_is_read_only_during_pending_propagation_and_after_document_edits() {
    let mut c = Circuit::default();
    let gate = add(&mut c, Kind::Gate(Gate::Not), 200, 100, 4);
    let position = c.component(gate).unwrap().ports()[1];
    let input = add(&mut c, Kind::Pin, position.x, position.y, 4);
    let mut sim = Simulator::default();
    sim.synchronize(&c);
    sim.advance(64);
    for value in [
        Value::known(4, 5),
        Value::filled(4, Bit::Unknown),
        Value::filled(4, Bit::Error),
        Value::known(4, 10),
    ] {
        sim.external_input(input, value);
        let before = (sim.steps(), sim.pending_count(), sim.port_value(gate, 0));
        assert_eq!(sim.combinational_analysis(), Analysis::new(&c));
        assert_eq!(
            (sim.steps(), sim.pending_count(), sim.port_value(gate, 0)),
            before
        );
        sim.advance(64);
    }
    let mut pin = c.component(input).unwrap().clone();
    pin.bits = 8;
    c.replace_component(pin);
    sim.synchronize(&c);
    assert_eq!(sim.combinational_analysis(), Analysis::new(&c));
    assert!(sim.combinational_analysis().regions.is_empty());
    c.remove(&BTreeSet::from([input]));
    sim.synchronize(&c);
    assert_eq!(ids(&sim.combinational_analysis()), vec![vec![gate]]);
}

#[test]
fn graph_partition_matches_independent_reachability_oracle() {
    let mut random = 23_u64;
    for _ in 0..128 {
        let n = 12;
        let mut edges = vec![vec![]; n];
        let mut reach = vec![vec![false; n]; n];
        for i in 0..n {
            for (j, reachable) in reach[i].iter_mut().enumerate() {
                random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                if random >> 60 == 0 {
                    edges[i].push(j);
                    *reachable = true;
                }
            }
        }
        for k in 0..n {
            for i in 0..n {
                for j in 0..n {
                    reach[i][j] |= reach[i][k] && reach[k][j];
                }
            }
        }
        let (cycles, regions) = graph::partition(&edges);
        assert_eq!(cycles, (0..n).filter(|i| reach[*i][*i]).collect::<Vec<_>>());
        let mut positions = BTreeMap::new();
        for (r, nodes) in regions.iter().enumerate() {
            for (p, &id) in nodes.iter().enumerate() {
                assert!(positions.insert(id, (r, p)).is_none());
                assert!(!reach[id][id]);
            }
            let mut connected = BTreeSet::from([nodes[0]]);
            loop {
                let old = connected.len();
                for &a in nodes {
                    for &b in nodes {
                        if edges[a].contains(&b)
                            && (connected.contains(&a) || connected.contains(&b))
                        {
                            connected.extend([a, b]);
                        }
                    }
                }
                if connected.len() == old {
                    break;
                }
            }
            assert_eq!(connected.len(), nodes.len());
        }
        assert_eq!(positions.len() + cycles.len(), n);
        for (a, to) in edges.iter().enumerate() {
            for b in to {
                if let (Some((ra, pa)), Some((rb, pb))) = (positions.get(&a), positions.get(b)) {
                    assert_eq!(ra, rb);
                    assert!(pa < pb);
                }
            }
        }
    }
}

#[test]
fn long_dependency_chains_and_large_feedback_do_not_use_the_call_stack() {
    let n = 20_000;
    let mut chain: Vec<_> = (0..n)
        .map(|i| if i + 1 < n { vec![i + 1] } else { vec![] })
        .collect();
    assert_eq!(graph::partition(&chain), (vec![], vec![(0..n).collect()]));
    chain[n - 1].push(0);
    assert_eq!(graph::partition(&chain), ((0..n).collect(), vec![]));
}

fn input_bit(sources: Vec<Source>, pull: Bit) -> InputBit {
    InputBit { sources, pull }
}
