use super::*;
use logisim_core::{Component, Kind, Point};

#[test]
fn instance_commands_root_cache_and_tick_telemetry_stay_separate() {
    let mut project = Project::default();
    let root = project.main;
    let child = project.add("child".into()).unwrap();
    let circuit = &mut project.circuit_mut(child).unwrap().circuit;
    let ram = circuit.add_component(Component::new(Kind::Ram, Point::new(300, 100)));
    circuit.add_component(Component::new(Kind::Clock, Point::new(100, 100)));
    let a = project.instance(child, Point::new(100, 100)).unwrap();
    let b = project.instance(child, Point::new(300, 100)).unwrap();
    let a = project.main_circuit_mut().circuit.add_component(a);
    let b = project.main_circuit_mut().circuit.add_component(b);
    let project = Arc::new(project);
    let configure = |revision, root, circuit, path| {
        Command::Configure(Context {
            revision,
            project: Arc::clone(&project),
            root,
            circuit,
            path,
            language: Language::English,
            plugins: Registry::default(),
        })
    };
    let mut engine = Engine::default();
    engine.command(configure(1, root, child, vec![a]));
    engine.sim.advance(4096);
    engine.command(Command::Action(
        1,
        SimulationAction::FillMemory {
            id: ram,
            start: 0,
            end: 1,
            value: 0x42,
        },
    ));
    engine.command(Command::Action(1, SimulationAction::Tick));
    engine.sim.advance(4096);
    assert_eq!(engine.total_ticks, 1);
    engine.command(configure(2, root, child, vec![b]));
    engine.command(Command::Action(
        1,
        SimulationAction::FillMemory {
            id: ram,
            start: 0,
            end: 1,
            value: 0xff,
        },
    ));
    let b_view = engine.snapshot().unwrap().view;
    assert_eq!(b_view.memories[&ram].get(0), 0);
    assert_eq!(b_view.ticks, 1);
    engine.command(configure(3, child, child, vec![]));
    engine.sim.advance(4096);
    assert_eq!(engine.snapshot().unwrap().view.ticks, 0);
    assert_eq!(engine.total_ticks, 1);
    engine.command(configure(4, root, child, vec![a]));
    let a_view = engine.snapshot().unwrap().view;
    assert_eq!(a_view.memories[&ram].get(0), 0x42);
    assert_eq!(a_view.ticks, 1);
    assert_eq!(engine.total_ticks, 1);
    engine.command(Command::Action(4, SimulationAction::Reset));
    assert_eq!(engine.total_ticks, 1);
    assert_eq!(engine.snapshot().unwrap().view.memories[&ram].get(0), 0);
}

#[test]
fn snapshot_diagnostics_use_context_language_on_the_worker() {
    let mut project = Project::default();
    let mut a = Component::new(Kind::Constant, Point::new(100, 100));
    a.constant = 0;
    let mut b = a.clone();
    b.constant = 1;
    project.main_circuit_mut().circuit.add_component(a);
    project.main_circuit_mut().circuit.add_component(b);
    let mut engine = Engine::default();
    engine.command(Command::Configure(Context {
        revision: 1,
        root: project.main,
        circuit: project.main,
        project: Arc::new(project),
        path: vec![],
        language: Language::SimplifiedChinese,
        plugins: Registry::default(),
    }));
    engine.sim.advance(4096);
    let view = engine.snapshot().unwrap().view;
    assert!(!view.issues.is_empty());
    assert!(view.issues[0].message.contains("错误"), "{:?}", view.issues);
}

fn pending_engine() -> Engine {
    let mut project = Project::default();
    project
        .main_circuit_mut()
        .circuit
        .add_component(Component::new(Kind::Clock, Point::new(100, 100)));
    let mut engine = Engine::default();
    engine.command(Command::Configure(Context {
        revision: 1,
        root: project.main,
        circuit: project.main,
        project: Arc::new(project),
        path: vec![],
        language: Language::English,
        plugins: Registry::default(),
    }));
    assert!(engine.sim.pending_count() > 0);
    engine
}

#[test]
fn running_snapshots_wait_for_settling_but_show_continuous_slow_propagation() {
    let mut engine = pending_engine();
    let now = Instant::now();
    assert!(!engine.snapshot_ready(now));
    assert!(!engine.snapshot_ready(now + Duration::from_millis(99)));
    assert!(engine.snapshot_ready(now + Duration::from_millis(100)));
    let snapshot = engine.snapshot().unwrap();
    assert!(snapshot.view.pending > 0); // Progress is real, never relabeled "settled".
    engine.sim.advance(4096);
    assert!(engine.snapshot_ready(now + Duration::from_millis(101)));
    assert_eq!(engine.snapshot().unwrap().view.pending, 0);
    // A new edge after a long idle gets its own settling interval.
    engine.command(Command::Action(1, SimulationAction::Tick));
    assert!(engine.sim.pending_count() > 0);
    assert!(!engine.snapshot_ready(now + Duration::from_secs(10)));
    engine.sim.advance(4096);
    assert!(engine.snapshot_ready(now + Duration::from_secs(10)));
    let snapshot = engine.snapshot().unwrap();
    assert_eq!(snapshot.view.pending, 0);
    assert_eq!(snapshot.view.ticks, 1);
    assert_eq!(snapshot.total_ticks, 1);
}

#[test]
fn paused_steps_and_suspension_do_not_wait_for_settling() {
    let mut engine = pending_engine();
    let now = Instant::now();
    assert!(!engine.snapshot_ready(now));
    engine.command(Command::Action(1, SimulationAction::ToggleEnabled));
    assert!(engine.snapshot_ready(now));
    let before = engine.snapshot().unwrap().view;
    assert!(!before.enabled);
    assert!(before.pending > 0);
    engine.command(Command::Action(1, SimulationAction::Step));
    assert!(engine.snapshot_ready(now));
    assert_eq!(engine.snapshot().unwrap().view.steps, before.steps + 1);
    engine.command(Command::Action(1, SimulationAction::Reset));
    assert!(engine.snapshot_ready(now));
    engine.command(Command::Action(1, SimulationAction::ToggleEnabled));
    assert!(!engine.snapshot_ready(now));
    engine.command(Command::Suspend(true));
    assert!(engine.snapshot_ready(now));
    engine.command(Command::Suspend(false));
    assert!(!engine.snapshot_ready(now));
}

#[test]
fn propagation_limit_bypasses_transient_snapshot_filter() {
    let mut engine = pending_engine();
    let mut project = Project::default();
    let circuit = &mut project.main_circuit_mut().circuit;
    let gate = Component::new(Kind::Gate(logisim_core::Gate::Nor), Point::new(200, 100));
    let ports = gate.ports();
    circuit.add_component(gate);
    circuit.add_route(ports[0], ports[1], false);
    let input = circuit.add_component(Component::new(Kind::Pin, ports[2]));
    engine.command(Command::Configure(Context {
        revision: 2,
        root: project.main,
        circuit: project.main,
        project: Arc::new(project),
        path: vec![],
        language: Language::English,
        plugins: Registry::default(),
    }));
    engine.sim.set_input(input, 1);
    engine.sim.advance(32);
    assert_eq!(engine.sim.pending_count(), 0);
    engine.sim.set_input(input, 0);
    let now = Instant::now();
    assert!(!engine.snapshot_ready(now));
    engine.sim.advance(4096);
    assert!(engine.sim.limited());
    assert!(engine.snapshot_ready(now)); // No wall time needs to pass for a fault.
    let view = engine.snapshot().unwrap().view;
    assert!(view.limited);
    assert!(!view.enabled);
    assert!(!view.issues.is_empty());
}

fn compiled_engine() -> (Engine, Id, Id, Id) {
    use logisim_core::{Circuit, FlipFlop, Gate};
    fn link(c: &mut Circuit, a: (Id, usize), b: (Id, usize)) {
        for port in [a, b] {
            let mut tunnel =
                Component::new(Kind::Tunnel, c.component(port.0).unwrap().ports()[port.1]);
            tunnel.label = format!("{}:{}", a.0, a.1);
            c.add_component(tunnel);
        }
    }
    let mut project = Project::default();
    let c = &mut project.main_circuit_mut().circuit;
    let data = c.add_component(Component::new(Kind::Pin, Point::new(100, 100)));
    let clock = c.add_component(Component::new(Kind::Clock, Point::new(100, 500)));
    let ff = c.add_component(Component::new(
        Kind::FlipFlop(FlipFlop::D),
        Point::new(300, 500),
    ));
    let mut previous = data;
    for i in 0..8 {
        let next = c.add_component(Component::new(
            Kind::Gate(Gate::Buffer),
            Point::new(300 + 200 * i, 100),
        ));
        link(c, (previous, 0), (next, 1));
        previous = next;
    }
    link(c, (previous, 0), (ff, 3));
    link(c, (clock, 0), (ff, 2));
    let mut engine = Engine::default();
    engine.command(Command::Configure(Context {
        revision: 1,
        root: project.main,
        circuit: project.main,
        project: Arc::new(project),
        path: vec![],
        language: Language::English,
        plugins: Registry::default(),
    }));
    (engine, data, clock, ff)
}
pub(super) fn finish(engine: &mut Engine) {
    for _ in 0..128 {
        engine.advance();
        if engine.sim.pending_count() == 0 {
            break;
        }
    }
    assert_eq!(engine.sim.pending_count(), 0);
    assert!(engine.sim.enabled());
}
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
