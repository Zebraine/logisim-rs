use super::*;
use logisim_core::{Component, Id, Kind, Point, Project};
fn fixture() -> (ProjectSimulator, Id) {
    let mut p = Project::default();
    let circuit = &mut p.main_circuit_mut().circuit;
    let mut counter = Component::new(Kind::Counter, Point::new(200, 200));
    counter.bits = 16;
    counter.sequential.counter_max = 65535;
    counter.sequential.counter_old = false;
    for (index, at) in counter.ports().into_iter().enumerate() {
        if counter.drives_port(index) {
            continue;
        }
        let mut source = Component::new(
            if index == 2 {
                Kind::Clock
            } else {
                Kind::Constant
            },
            at,
        );
        source.bits = counter.port_width(index).unwrap();
        source.constant = u32::from(index == 8);
        circuit.add_component(source);
    }
    let id = circuit.add_component(counter);
    let mut sim = ProjectSimulator::default();
    sim.synchronize(&p, p.main);
    sim.advance(32);
    assert_eq!(sim.pending_count(), 0);
    (sim, id)
}
fn poll(clocks: &mut Clocks, now: Instant, sim: &mut ProjectSimulator) -> Option<Duration> {
    clocks.poll_with_elapsed(now, sim, ProjectSimulator::step, || Duration::ZERO)
}
#[test]
fn defaults_custom_rates_and_invalid_commands() {
    let mut c = Clocks::default();
    let mut view = SimulationView::default();
    c.present(&mut view);
    assert_eq!(view.tick_hz, 4);
    assert!(!view.automatic_ticks);
    for hz in [1, 3, 250, 4096] {
        c.action(&SimulationAction::SetTickRate(hz));
        c.present(&mut view);
        assert_eq!(view.tick_hz, hz);
    }
    for hz in [0, 4097, u32::MAX] {
        c.action(&SimulationAction::SetTickRate(hz));
        c.present(&mut view);
        assert_eq!(view.tick_hz, 4096);
    }
}
#[test]
fn pacing_pauses_without_backlog_and_waits_for_propagation() {
    let (mut sim, id) = fixture();
    let mut c = Clocks::default();
    let start = Instant::now();
    let period = Duration::from_millis(250);
    assert_eq!(poll(&mut c, start, &mut sim), None);
    c.action(&SimulationAction::ToggleTicks);
    assert_eq!(poll(&mut c, start, &mut sim), Some(period));
    poll(&mut c, start + Duration::from_millis(249), &mut sim);
    assert_eq!(sim.ticks(), 0);
    poll(&mut c, start + period, &mut sim);
    assert_eq!(sim.ticks(), 1);
    assert_eq!(
        sim.view(&[]).unwrap().stored_value(id).unwrap().number(),
        Some(1)
    );
    // Manual stimulus still propagating: do not advance to a second clock edge.
    sim.tick();
    assert!(sim.pending_count() > 0);
    poll(&mut c, start + period * 2, &mut sim);
    assert_eq!(sim.ticks(), 2);
    sim.advance(32);
    poll(&mut c, start + Duration::from_secs(20), &mut sim);
    assert_eq!(sim.ticks(), 3); // one edge, never the stalled window's entire backlog
    sim.set_enabled(false);
    assert_eq!(
        poll(&mut c, start + Duration::from_secs(21), &mut sim),
        None
    );
    sim.set_enabled(true);
    assert_eq!(
        poll(&mut c, start + Duration::from_secs(60), &mut sim),
        Some(period)
    );
    assert_eq!(sim.ticks(), 3);
    c.suspend();
    assert_eq!(
        poll(&mut c, start + Duration::from_secs(100), &mut sim),
        Some(period)
    );
    c.action(&SimulationAction::SetTickRate(250));
    assert_eq!(
        poll(&mut c, start + Duration::from_secs(101), &mut sim),
        Some(Duration::from_millis(4))
    );
}
#[test]
fn high_frequency_batches_preserve_every_edge_at_render_frame_cadence() {
    let (mut sim, id) = fixture();
    let mut c = Clocks::default();
    c.action(&SimulationAction::SetTickRate(4096));
    c.action(&SimulationAction::ToggleTicks);
    let start = Instant::now();
    let period = Duration::from_secs_f64(1.0 / 4096.0);
    poll(&mut c, start, &mut sim);
    for frame in 1..=64 {
        poll(&mut c, start + period * (64 * frame), &mut sim);
        assert_eq!(sim.ticks(), u64::from(64 * frame));
        assert_eq!(sim.pending_count(), 0);
        assert_eq!(
            sim.view(&[]).unwrap().stored_value(id).unwrap().number(),
            Some(32 * frame)
        );
    }
    assert_eq!(sim.ticks(), 4096);
    assert!(!sim.limited());
    assert!(sim.diagnostics(&[]).is_empty());
}
#[test]
fn expensive_work_yields_with_the_edge_pending_and_discards_backlog() {
    let (mut sim, _) = fixture();
    let mut c = Clocks::default();
    c.action(&SimulationAction::SetTickRate(4096));
    c.action(&SimulationAction::ToggleTicks);
    let start = Instant::now();
    poll(&mut c, start, &mut sim);
    let busy = start + Duration::from_millis(16);
    let delay = c.poll_with_elapsed(busy, &mut sim, ProjectSimulator::step, || {
        Duration::from_millis(5)
    });
    assert_eq!(sim.ticks(), 1);
    assert!(sim.pending_count() > 0);
    assert_eq!(delay, Some(Duration::from_secs_f64(1.0 / 4096.0)));
    sim.advance(32);
    poll(&mut c, busy + Duration::from_millis(5), &mut sim);
    assert_eq!(sim.ticks(), 1);
    poll(&mut c, busy + Duration::from_millis(6), &mut sim);
    assert_eq!(sim.ticks(), 5);
}
#[test]
fn automatic_clocks_use_the_supplied_compiled_scheduler_for_every_edge() {
    let (mut sim, id) = fixture();
    let mut c = Clocks::default();
    c.action(&SimulationAction::SetTickRate(4096));
    c.action(&SimulationAction::ToggleTicks);
    let start = Instant::now();
    let period = Duration::from_secs_f64(1.0 / 4096.0);
    let mut calls = 0;
    for frame in 0..=8 {
        c.poll_with_elapsed(
            start + period * (64 * frame),
            &mut sim,
            |sim| {
                calls += 1;
                sim.step_compiled().unwrap();
                true
            },
            || Duration::ZERO,
        );
        assert_eq!(sim.ticks(), u64::from(64 * frame));
        assert_eq!(sim.pending_count(), 0);
        assert_eq!(
            sim.view(&[]).unwrap().stored_value(id).unwrap().number(),
            Some(32 * frame)
        );
    }
    assert!(calls >= 512);
    assert!(!sim.limited());
}
