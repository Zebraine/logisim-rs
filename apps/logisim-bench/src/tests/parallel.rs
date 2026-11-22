use super::*;
use logisim_core::{Id, Kind, Project};
use logisim_sim::combinational::DataflowPool;

fn instance_paths(project: &Project) -> Vec<Vec<Id>> {
    let mut pending = vec![(Vec::new(), project.main)];
    let mut paths = Vec::new();
    while let Some((path, definition)) = pending.pop() {
        for c in &project.circuit(definition).unwrap().circuit.components {
            if let Kind::Subcircuit(child) = c.kind {
                let mut next = path.clone();
                next.push(c.id);
                pending.push((next, child));
            }
        }
        paths.push(path);
    }
    paths
}

fn compare_settle(
    serial: &mut ProjectSimulator,
    candidate: &mut ProjectSimulator,
    paths: &[Vec<Id>],
) -> Result<()> {
    assert_eq!(serial.settle_compiled()?, candidate.settle_compiled()?);
    assert_eq!(serial.ticks(), candidate.ticks());
    assert_eq!(serial.steps(), candidate.steps());
    assert_eq!(serial.pending_count(), candidate.pending_count());
    for path in paths {
        let diagnostics = |sim: &ProjectSimulator| {
            sim.diagnostics(path)
                .into_iter()
                .map(|d| (d.problem, d.components, d.wires))
                .collect::<Vec<_>>()
        };
        assert_eq!(diagnostics(serial), diagnostics(candidate));
        let a = serial.view(path).unwrap();
        let b = candidate.view(path).unwrap();
        let mut actual = b.signal_nets();
        for expected in a.signal_nets() {
            let net = actual.next().unwrap();
            assert_eq!((expected.wires, expected.ports), (net.wires, net.ports));
            assert_eq!(
                (expected.width_mismatch, expected.value),
                (net.width_mismatch, net.value)
            );
            for &(id, pin) in net.ports {
                assert_eq!(a.drive_value(id, pin), b.drive_value(id, pin));
            }
        }
        assert!(actual.next().is_none());
    }
    Ok(())
}

#[test]
fn every_instance_net_and_drive_matches_across_cpu_and_plugin_state_commits() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in ["single-cycle", "plugin"] {
        let f = Fixture::load(name, &root)?;
        let paths = instance_paths(&f.project);
        for threads in [2, 4] {
            let mut serial = ProjectSimulator::default();
            let mut candidate = ProjectSimulator::default();
            candidate.set_dataflow_pool(DataflowPool::for_threads(threads)?);
            for sim in [&mut serial, &mut candidate] {
                sim.set_plugins(f.plugins.clone());
                sim.synchronize(&f.project, f.project.main);
                measure::reset(&f, sim)?;
            }
            for i in 0..f.stimulus.iterations() {
                for sim in [&mut serial, &mut candidate] {
                    f.stimulus.drive(sim, i);
                }
                if matches!(f.stimulus, workload::Stimulus::Plugin { .. }) {
                    compare_settle(&mut serial, &mut candidate, &paths)?;
                }
                assert!(serial.tick() && candidate.tick());
                compare_settle(&mut serial, &mut candidate, &paths)?;
            }
            assert!(candidate.plugin_failures().is_empty());
        }
    }
    Ok(())
}

#[test]
fn feedback_rejection_preserves_the_next_manual_event_step() -> Result<()> {
    use logisim_core::{Component, Gate, Point};
    use logisim_sim::{combinational::BoundaryReason, stable::Failure};
    let mut project = Project::default();
    let c = &mut project.main_circuit_mut().circuit;
    let gate = Component::new(Kind::Gate(Gate::Nor), Point::new(200, 100));
    let ports = gate.ports();
    let id = c.add_component(gate);
    let pin = c.add_component(Component::new(Kind::Pin, ports[2]));
    c.add_route(ports[0], ports[1], false);
    let mut reference = ProjectSimulator::default();
    let mut candidate = ProjectSimulator::default();
    candidate.set_dataflow_pool(DataflowPool::for_threads(4)?);
    for sim in [&mut reference, &mut candidate] {
        sim.synchronize(&project, project.main);
        sim.set_input(pin, 1);
    }
    let before = (candidate.steps(), candidate.pending_count());
    assert!(matches!(
        candidate.prepare_compiled().unwrap_err().reason,
        Failure::Topology {
            reason: BoundaryReason::Feedback,
            ..
        }
    ));
    assert_eq!(before, (candidate.steps(), candidate.pending_count()));
    for _ in 0..8 {
        assert_eq!(candidate.step(), reference.step());
        assert_eq!(
            candidate.view(&[]).unwrap().drive_value(id, 0),
            reference.view(&[]).unwrap().drive_value(id, 0)
        );
        assert_eq!(candidate.pending_count(), reference.pending_count());
        assert_eq!(candidate.steps(), reference.steps());
    }
    Ok(())
}
