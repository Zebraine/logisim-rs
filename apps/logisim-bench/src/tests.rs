use super::*;
use logisim_core::{Bit, Value};
use std::path::Path;

#[test]
fn arguments_and_nearest_rank() {
    let parse = |s: &str| options(s.split_whitespace().map(str::to_owned));
    assert_eq!(parse("deep").unwrap(), ("deep".into(), 20));
    assert_eq!(parse("plugin --runs 1").unwrap().1, 1);
    for invalid in ["", "wide --runs", "wide --runs 0", "deep --runs 1 extra"] {
        assert!(parse(invalid).is_err());
    }
    let mut samples = (1..=20)
        .rev()
        .map(Duration::from_millis)
        .collect::<Vec<_>>();
    let dist = distribution(&mut samples);
    assert_eq!((dist.p50, dist.p95), (10.0, 19.0));
}

#[test]
fn trace_preserves_four_states_and_bus_width() {
    let digest = |v| {
        let mut d = 0;
        measure::hash_value(&mut d, v);
        d
    };
    let mut hashes =
        [Bit::Zero, Bit::One, Bit::Unknown, Bit::Error].map(|bit| digest(Value::filled(1, bit)));
    hashes.sort();
    assert!(hashes.windows(2).all(|w| w[0] != w[1]));
    assert_ne!(digest(Value::known(1, 0)), digest(Value::known(32, 0)));
}

#[test]
fn synthetic_signals_match_independent_arithmetic_and_inversion() -> Result<()> {
    for name in ["wide", "deep"] {
        let f = Fixture::load(name, Path::new("."))?;
        let mut sim = ProjectSimulator::default();
        sim.synchronize(&f.project, f.project.main);
        let measured = trial(&f, &mut sim)?;
        let mut expected = 0xcbf29ce484222325;
        let mut lanes: Vec<u32> = (1..=f.probes.len() as u32).collect();
        for i in 0..f.stimulus.iterations() {
            if name == "wide" {
                let lane = i % lanes.len();
                lanes[lane] = lane as u32 + 1 + i as u32 + 1;
                for &out in &lanes {
                    measure::hash_value(&mut expected, Value::known(32, out));
                }
            } else {
                // 318 inversions reproduce the one-bit stimulus.
                measure::hash_value(&mut expected, Value::known(1, (i as u32 + 1) & 1));
            }
        }
        assert_eq!(measured.trace.digest, expected);
        assert_eq!(measured.trace.digest, f.stimulus.expected_digest());
        assert_eq!(measured.trace.ticks, 0);
    }
    Ok(())
}

#[test]
fn register_file_rising_edge_trace_matches_readback() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let f = Fixture::load("plugin", &root)?;
    let mut sim = ProjectSimulator::default();
    sim.set_plugins(f.plugins.clone());
    sim.synchronize(&f.project, f.project.main);
    let sample = trial(&f, &mut sim)?;
    let mut expected = 0xcbf29ce484222325;
    for i in 0..512 {
        let last_rising = i & !1;
        measure::hash_value(&mut expected, Value::known(32, 0x5a00_0000 + last_rising));
    }
    assert_eq!(sample.trace.digest, expected);
    assert_eq!(expected, f.stimulus.expected_digest());
    assert_eq!(sample.trace.ticks, 512);
    assert!(sample.trace.work.plugin_samples > 0 && sample.trace.work.plugin_outputs > 0);
    Ok(())
}
