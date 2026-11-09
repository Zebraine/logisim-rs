use crate::simulation::worker::engine::*;
use logisim_core::{Component, Kind, Point};

pub(super) fn pending_engine() -> Engine {
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
        mode: SimulationMode::default(),
        plugins: Registry::default(),
    }));
    assert!(engine.sim.pending_count() > 0);
    engine
}

pub(super) fn compiled_engine() -> (Engine, Id, Id, Id) {
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
        mode: SimulationMode::default(),
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
