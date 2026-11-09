use crate::simulation::worker::engine::*;
use logisim_core::{Component, Kind, Point};

use super::support::*;

#[test]
fn automatic_propagation_uses_compiled_sampling_and_manual_steps_remain_events() {
    let (mut engine, data, clock, ff) = compiled_engine();
    finish(&mut engine);
    assert_eq!(engine.propagation, Propagation::Compiled);
    for paused in [false, true] {
        engine.command(Command::Action(1, SimulationAction::Reset));
        finish(&mut engine);
        if paused {
            engine.command(Command::Action(1, SimulationAction::ToggleEnabled));
        }
        engine.command(Command::Action(
            1,
            SimulationAction::SetInput { id: data, value: 1 },
        ));
        engine.command(Command::Action(
            1,
            SimulationAction::PokeClock { id: clock },
        ));
        if paused {
            let before = engine.sim.steps();
            engine.command(Command::Action(1, SimulationAction::Step));
            assert_eq!(engine.sim.steps(), before + 1);
            for _ in 0..32 {
                engine.command(Command::Action(1, SimulationAction::Step));
            }
        } else {
            finish(&mut engine);
        }
        assert_eq!(
            engine
                .sim
                .view(&[])
                .unwrap()
                .stored_value(ff)
                .unwrap()
                .number(),
            Some(u32::from(!paused))
        );
    }
}

#[test]
fn unsupported_fallback_is_reselected_after_edits_but_not_view_changes() {
    let (mut engine, _, _, _) = compiled_engine();
    finish(&mut engine);
    let mut context = engine.context.clone().unwrap();
    context.language = Language::SimplifiedChinese;
    engine.command(Command::Configure(context.clone()));
    assert_eq!(engine.propagation, Propagation::Compiled);
    let latch = Arc::make_mut(&mut context.project)
        .main_circuit_mut()
        .circuit
        .add_component(Component::new(
            Kind::Latch(logisim_core::Latch::D),
            Point::new(2000, 500),
        ));
    engine.command(Command::Configure(context.clone()));
    assert_eq!(engine.propagation, Propagation::Unselected);
    finish(&mut engine);
    assert_eq!(engine.propagation, Propagation::Event);
    Arc::make_mut(&mut context.project)
        .main_circuit_mut()
        .circuit
        .components
        .retain(|c| c.id != latch);
    engine.command(Command::Configure(context));
    finish(&mut engine);
    assert_eq!(engine.propagation, Propagation::Compiled);
    engine.command(Command::NewDocument);
    assert_eq!(engine.propagation, Propagation::Unselected);
}

#[test]
fn selected_mode_controls_sampling_and_survives_reset_and_configuration_changes() {
    let (mut engine, data, clock, ff) = compiled_engine();
    for (index, mode) in [
        SimulationMode::Event,
        SimulationMode::Stable,
        SimulationMode::Event,
    ]
    .into_iter()
    .enumerate()
    {
        let mut context = engine.context.clone().unwrap();
        context.mode = mode;
        context.revision = index as u64 + 2;
        let revision = context.revision;
        engine.command(Command::Configure(context.clone()));
        engine.command(Command::Action(revision, SimulationAction::Reset));
        finish(&mut engine);
        let selected = if mode == SimulationMode::Event {
            Propagation::Event
        } else {
            Propagation::Compiled
        };
        assert_eq!(engine.propagation, selected);
        context.language = Language::SimplifiedChinese;
        engine.command(Command::Configure(context));
        engine.command(Command::Action(
            revision,
            SimulationAction::SetInput { id: data, value: 1 },
        ));
        engine.command(Command::Action(
            revision,
            SimulationAction::PokeClock { id: clock },
        ));
        finish(&mut engine);
        // Unequal clock/data path depths distinguish the actual two timing models.
        assert_eq!(
            engine
                .sim
                .view(&[])
                .unwrap()
                .stored_value(ff)
                .unwrap()
                .number(),
            Some(u32::from(mode == SimulationMode::Stable))
        );
        assert_eq!(engine.propagation, selected);
    }
}

#[test]
fn mode_switch_preserves_paused_state_ram_and_tick_accounting() {
    let (mut engine, data, _, ff) = compiled_engine();
    let mut context = engine.context.clone().unwrap();
    let mut memory = Component::new(Kind::Ram, Point::new(4000, 1000));
    memory.memory.interface = logisim_core::MemoryInterface::Separate;
    let ram = Arc::make_mut(&mut context.project)
        .main_circuit_mut()
        .circuit
        .add_component(memory);
    engine.command(Command::Configure(context));
    finish(&mut engine);
    engine.command(Command::Action(
        1,
        SimulationAction::SetInput { id: data, value: 1 },
    ));
    finish(&mut engine);
    engine.command(Command::Action(
        1,
        SimulationAction::FillMemory {
            id: ram,
            start: 0,
            end: 1,
            value: 0x42,
        },
    ));
    finish(&mut engine);
    engine.command(Command::Action(1, SimulationAction::Tick));
    finish(&mut engine);
    engine.command(Command::Action(1, SimulationAction::ToggleEnabled));
    let before = (engine.sim.ticks(), engine.sim.steps(), engine.total_ticks);
    assert_eq!(
        engine
            .sim
            .view(&[])
            .unwrap()
            .stored_value(ff)
            .unwrap()
            .number(),
        Some(1)
    );
    for mode in [SimulationMode::Event, SimulationMode::Stable] {
        let mut context = engine.context.clone().unwrap();
        context.mode = mode;
        context.revision += 1;
        engine.command(Command::Configure(context));
        engine.advance();
        assert!(!engine.sim.enabled());
        assert_eq!(
            before,
            (engine.sim.ticks(), engine.sim.steps(), engine.total_ticks)
        );
        assert_eq!(
            engine
                .sim
                .view(&[])
                .unwrap()
                .stored_value(ff)
                .unwrap()
                .number(),
            Some(1)
        );
        assert_eq!(
            engine
                .sim
                .view(&[])
                .unwrap()
                .memory_contents(ram)
                .unwrap()
                .get(0),
            0x42
        );
    }
}

#[test]
fn switching_to_events_mid_closure_keeps_pending_work_and_manual_step_semantics() {
    let (mut engine, data, _, ff) = compiled_engine();
    finish(&mut engine);
    engine.command(Command::Action(
        1,
        SimulationAction::SetInput { id: data, value: 1 },
    ));
    finish(&mut engine);
    engine.command(Command::Action(1, SimulationAction::Tick));
    assert!(engine.propagation.step(&mut engine.sim));
    assert!(engine.sim.pending_count() > 0);
    engine.command(Command::Action(1, SimulationAction::ToggleEnabled));
    let before = (engine.sim.steps(), engine.sim.ticks(), engine.total_ticks);
    let mut context = engine.context.clone().unwrap();
    context.revision = 2;
    context.mode = SimulationMode::Event;
    engine.command(Command::Configure(context));
    assert_eq!(engine.propagation, Propagation::Event);
    assert!(!engine.sim.enabled());
    assert_eq!(
        before,
        (engine.sim.steps(), engine.sim.ticks(), engine.total_ticks)
    );
    engine.command(Command::Action(2, SimulationAction::Step));
    assert_eq!(engine.sim.steps(), before.0 + 1);
    assert_eq!(
        engine
            .sim
            .view(&[])
            .unwrap()
            .stored_value(ff)
            .unwrap()
            .number(),
        Some(1)
    );
    engine.command(Command::Action(2, SimulationAction::ToggleEnabled));
    finish(&mut engine);
    assert_eq!(engine.sim.ticks(), before.1);
    assert_eq!(engine.total_ticks, before.2);
    let mut context = engine.context.clone().unwrap();
    context.mode = SimulationMode::Stable;
    engine.command(Command::Configure(context));
    engine.command(Command::Action(2, SimulationAction::Tick));
    finish(&mut engine);
    assert_eq!(engine.propagation, Propagation::Compiled);
    assert_eq!(
        engine
            .sim
            .view(&[])
            .unwrap()
            .stored_value(ff)
            .unwrap()
            .number(),
        Some(1)
    );
}
