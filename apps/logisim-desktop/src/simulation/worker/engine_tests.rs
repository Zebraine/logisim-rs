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
