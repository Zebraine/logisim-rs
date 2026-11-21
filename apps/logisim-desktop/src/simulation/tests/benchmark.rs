//! Opt-in release measurement of the GUI worker's snapshot adapter without a window.
use super::*;
use std::{hint::black_box, path::Path, time::Instant};

#[test]
#[ignore = "manual release benchmark"]
fn single_cycle_snapshot_cost() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let file = root.join("examples/course/single-cycle.circ");
    let project = logisim_file::parse(&std::fs::read_to_string(file).unwrap())
        .unwrap()
        .document;
    let mut registry = logisim_plugin::Registry::default();
    registry
        .load(&std::fs::read(root.join("plugins/course/course.wasm")).unwrap())
        .unwrap();
    let mut sim = ProjectSimulator::default();
    sim.set_plugins(registry);
    sim.synchronize(&project, project.main);
    sim.settle_compiled().unwrap();
    let circuit = &project.main_circuit().circuit;
    let mut samples = Vec::new();
    for i in 0..123 {
        assert!(sim.tick());
        sim.settle_compiled().unwrap();
        let start = Instant::now();
        let snapshot = black_box(present_project(circuit, &sim, &[]));
        let time = start.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(snapshot.ticks, sim.ticks());
        assert_eq!(snapshot.wires.len(), circuit.wires.len());
        if i >= 3 {
            samples.push(time);
        }
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "{}",
        serde_json::json!({"schema":1,"workload":"gui-snapshot","runs":120,"warmup":3,
        "construct_ms":{"p50":samples[59],"p95":samples[113]}})
    );
}
