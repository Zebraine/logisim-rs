use super::*;

#[test]
fn measures_completed_frames_and_actual_ticks_over_wall_time() {
    let mut rates = RuntimeRates::default();
    let mut view = SimulationView {
        tick_hz: 4096,
        ..Default::default()
    };
    let start = Instant::now();
    rates.present(start, 100, &mut view);
    assert_eq!(view.frames_per_second, None);
    // Multiple UI passes with the same frame counter must not inflate FPS.
    for frame in 1..=60 {
        rates.record_ticks(16);
        let now = start + Duration::from_secs_f64(f64::from(frame) / 60.0);
        rates.present(now, 100 + frame as u64, &mut view);
        rates.present(now, 100 + frame as u64, &mut view);
    }
    assert_eq!(view.frames_per_second, Some(60.0));
    assert_eq!(view.actual_tick_hz, 960.0);
    assert_eq!(view.tick_hz, 4096);
    // Paused/idle work reports its real low redraw rate and zero ticks.
    rates.present(start + Duration::from_secs(2), 164, &mut view);
    assert_eq!(view.frames_per_second, Some(4.0));
    assert_eq!(view.actual_tick_hz, 0.0);
    // A stalled frame uses actual elapsed time, not an assumed one-second window.
    rates.record_ticks(2);
    rates.present(start + Duration::from_secs(4), 165, &mut view);
    assert_eq!(view.frames_per_second, Some(0.5));
    assert_eq!(view.actual_tick_hz, 1.0);
}

#[test]
fn resets_and_restored_counters_do_not_become_measured_ticks() {
    let mut rates = RuntimeRates::default();
    let mut view = SimulationView::default();
    let start = Instant::now();
    rates.present(start, 0, &mut view);
    // Imported/cached counters are presentation data, not telemetry events.
    view.ticks = 1_000_000;
    rates.present(start + Duration::from_secs(1), 10, &mut view);
    assert_eq!(view.actual_tick_hz, 0.0);
    view.ticks = 0;
    rates.record_ticks(4);
    rates.present(start + Duration::from_secs(2), 20, &mut view);
    assert_eq!(view.actual_tick_hz, 4.0);
}
