use super::*;
use logisim_core::{Component, Kind, Point};
use std::time::Instant;

fn workspace() -> (Workspace, Id, Id) {
    let mut project = Project::default();
    let circuit = &mut project.main_circuit_mut().circuit;
    let clock = circuit.add_component(Component::new(Kind::Clock, Point::new(100, 100)));
    let ram = circuit.add_component(Component::new(Kind::Ram, Point::new(400, 300)));
    let mut workspace = Workspace::default();
    workspace.replace_project(project);
    (workspace, clock, ram)
}
fn wait(
    worker: &mut Worker,
    workspace: &mut Workspace,
    predicate: impl Fn(&SimulationView) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        worker.receive(workspace);
        if predicate(workspace.simulation_mut()) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "worker failed to publish expected state"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn simulation_continues_without_gui_and_latest_slot_does_not_queue_frames() {
    let (mut workspace, _, _) = workspace();
    let mut worker = Worker::new(|| {});
    worker.synchronize(&mut workspace, false);
    worker.action(SimulationAction::SetTickRate(32768));
    worker.action(SimulationAction::ToggleTicks);
    // No receive/render calls: the first snapshot must not block computation.
    thread::sleep(Duration::from_millis(150));
    let first = worker.shared.latest.lock().unwrap().take().unwrap();
    assert!(first.view.ticks > 1);
    assert!(first.view.automatic_ticks);
    assert_eq!(first.view.tick_hz, 32768);
    thread::sleep(Duration::from_millis(100));
    let second = worker.shared.latest.lock().unwrap().take().unwrap();
    assert!(second.view.ticks > first.view.ticks);
    assert!(second.total_ticks > first.total_ticks);
    worker.action(SimulationAction::ToggleEnabled);
    wait(&mut worker, &mut workspace, |v| {
        !v.enabled && v.tick_hz == 32768
    });
    let ticks = workspace.simulation_mut().ticks;
    thread::sleep(Duration::from_millis(40));
    worker.receive(&mut workspace);
    assert_eq!(workspace.simulation_mut().ticks, ticks);
    assert!(workspace.simulation_mut().automatic_ticks);
}

#[test]
fn new_context_rejects_old_snapshots_actions_and_memory_targets() {
    let (mut workspace, _, ram) = workspace();
    let mut worker = Worker::new(|| {});
    worker.synchronize(&mut workspace, true);
    wait(&mut worker, &mut workspace, |v| {
        v.memories.contains_key(&ram)
    });
    let old = worker.revision;
    let mut contents = MemoryContents::default();
    contents.set(0, 0x2a);
    assert!(worker.replace_memory(&[], ram, contents.clone()));
    wait(&mut worker, &mut workspace, |v| {
        v.memories[&ram].get(0) == 0x2a
    });
    let total_ticks = worker.reported_ticks;
    worker.new_document();
    // Same IDs in an independently opened document must not inherit old RAM.
    worker.synchronize(&mut workspace, true);
    assert!(workspace.simulation_mut().memories.is_empty());
    assert!(!worker.replace_memory(&[], ram, contents.clone()));
    worker
        .shared
        .send(Command::Action(old, SimulationAction::ToggleEnabled));
    worker
        .shared
        .send(Command::Memory(old, vec![], ram, contents));
    wait(&mut worker, &mut workspace, |v| {
        v.memories.contains_key(&ram)
    });
    assert_eq!(workspace.simulation_mut().memories[&ram].get(0), 0);
    assert!(workspace.simulation_mut().enabled);
    assert_eq!(worker.reported_ticks, total_ticks);
}

#[test]
fn manual_steps_and_memory_commands_keep_order_while_paused() {
    let (mut workspace, clock, ram) = workspace();
    let mut worker = Worker::new(|| {});
    worker.synchronize(&mut workspace, false);
    wait(&mut worker, &mut workspace, |v| {
        v.enabled && v.pending == 0 && v.has_clocks
    });
    worker.action(SimulationAction::ToggleEnabled);
    wait(&mut worker, &mut workspace, |v| !v.enabled);
    // Match exact batch semantics of the original synchronous adapter.
    let mut reference = logisim_sim::ProjectSimulator::default();
    reference.synchronize(workspace.project(), workspace.simulation_root());
    reference.advance(4096);
    reference.set_enabled(false);
    let commands = [
        SimulationAction::PokeClock { id: clock },
        SimulationAction::Step,
        SimulationAction::Step,
        SimulationAction::Reset,
        SimulationAction::PokeClock { id: clock },
        SimulationAction::Step,
        SimulationAction::FillMemory {
            id: ram,
            start: 0,
            end: 1,
            value: 0x12,
        },
        SimulationAction::FillMemory {
            id: ram,
            start: 0,
            end: 1,
            value: 0x34,
        },
    ];
    for action in commands {
        super::super::apply_project(&mut reference, &[], action.clone());
        worker.action(action);
    }
    wait(&mut worker, &mut workspace, |v| {
        v.memories.get(&ram).is_some_and(|m| m.get(0) == 0x34)
    });
    let view = workspace.simulation_mut();
    assert!(!view.enabled);
    assert_eq!(view.steps, reference.steps());
    assert_eq!(view.pending, reference.pending_count());
    assert_eq!(view.ticks, reference.ticks());
    assert_eq!(
        view.ports[&(clock, 0)].binary,
        reference
            .view(&[])
            .unwrap()
            .port_value(clock, 0)
            .unwrap()
            .binary()
    );
}

#[test]
fn file_dialog_suspends_work_and_shutdown_wakes_an_idle_worker() {
    let (mut workspace, _, _) = workspace();
    let mut worker = Worker::new(|| {});
    worker.synchronize(&mut workspace, true);
    worker.action(SimulationAction::ToggleTicks);
    wait(&mut worker, &mut workspace, |v| v.automatic_ticks);
    thread::sleep(Duration::from_millis(300));
    worker.receive(&mut workspace);
    assert_eq!(workspace.simulation_mut().ticks, 0);
    worker.synchronize(&mut workspace, false);
    wait(&mut worker, &mut workspace, |v| v.ticks > 0);
    assert!(workspace.simulation_mut().ticks < 4); // No dialog-time backlog.
    let shared = Arc::clone(&worker.shared);
    drop(worker); // Join proves no background simulation survives the host.
    assert!(shared.inbox.lock().unwrap().stop);
    let idle = Worker::new(|| {});
    drop(idle);
}

#[test]
fn configure_coalescing_never_crosses_an_action_barrier() {
    let shared = Shared::default();
    let context = |revision| Context {
        revision,
        project: Arc::new(Project::default()),
        root: 1,
        circuit: 1,
        path: vec![],
        language: Language::English,
        mode: SimulationMode::default(),
        plugins: Registry::default(),
    };
    shared.send(Command::Configure(context(1)));
    shared.send(Command::Configure(context(2)));
    shared.send(Command::Action(2, SimulationAction::Step));
    shared.send(Command::Configure(context(3)));
    shared.send(Command::Configure(context(4)));
    let mut inbox = shared.inbox.lock().unwrap();
    assert_eq!(inbox.commands.len(), 3);
    assert!(matches!(
        inbox.commands.pop_front(),
        Some(Command::Configure(Context { revision: 2, .. }))
    ));
    assert!(matches!(
        inbox.commands.pop_front(),
        Some(Command::Action(2, SimulationAction::Step))
    ));
    assert!(matches!(
        inbox.commands.pop_front(),
        Some(Command::Configure(Context { revision: 4, .. }))
    ));
}

#[test]
fn stale_snapshot_is_discarded_but_accepted_ticks_are_counted_once() {
    let mut worker = Worker::new(|| {});
    let (mut workspace, _, ram) = workspace();
    worker.revision = 2;
    let snapshot = || Snapshot {
        revision: 1,
        view: SimulationView {
            memories: [(ram, MemoryContents::default())].into(),
            ..Default::default()
        },
        total_ticks: 7,
    };
    *worker.shared.latest.lock().unwrap() = Some(snapshot());
    assert_eq!(worker.receive(&mut workspace), 7);
    assert!(workspace.simulation_mut().memories.is_empty());
    assert!(worker.ready_memories.is_empty());
    *worker.shared.latest.lock().unwrap() = Some(snapshot());
    assert_eq!(worker.receive(&mut workspace), 0);
}

#[test]
fn unexpected_worker_exit_is_reported_and_does_not_accept_more_commands() {
    let (mut workspace, _, ram) = workspace();
    let mut worker = Worker::new(|| panic!("injected worker failure"));
    worker.synchronize(&mut workspace, false);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !worker.stopped() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    worker.receive(&mut workspace);
    assert!(!workspace.simulation_mut().enabled);
    assert_eq!(
        workspace.simulation_mut().issues[0].message,
        "Simulation worker stopped. Restart the application."
    );
    worker.action(SimulationAction::Step);
    assert!(!worker.replace_memory(&[], ram, MemoryContents::default()));
    assert!(worker.shared.inbox.lock().unwrap().commands.is_empty());
}

#[test]
fn final_settled_snapshot_arrives_without_automatic_ticks_or_further_input() {
    let mut project = Project::default();
    let circuit = &mut project.main_circuit_mut().circuit;
    let gate = Component::new(Kind::Gate(logisim_core::Gate::Buffer), Point::new(200, 100));
    let ports = gate.ports();
    let gate = circuit.add_component(gate);
    let input = circuit.add_component(Component::new(Kind::Pin, ports[1]));
    let mut workspace = Workspace::default();
    workspace.replace_project(project);
    let mut worker = Worker::new(|| {});
    worker.synchronize(&mut workspace, false);
    wait(&mut worker, &mut workspace, |v| {
        v.pending == 0 && v.ports.get(&(gate, 0)).is_some_and(|p| p.binary == "0")
    });
    worker.action(SimulationAction::SetInput {
        id: input,
        value: 1,
    });
    wait(&mut worker, &mut workspace, |v| {
        v.pending == 0 && v.ports.get(&(gate, 0)).is_some_and(|p| p.binary == "1")
    });
    assert_eq!(workspace.simulation_mut().ticks, 0);
    assert!(!workspace.simulation_mut().automatic_ticks);
}

#[test]
fn mode_changes_version_commands_without_clearing_the_current_view_or_memory_target() {
    let (mut workspace, _, ram) = workspace();
    let mut worker = Worker::new(|| {});
    worker.synchronize(&mut workspace, true);
    wait(&mut worker, &mut workspace, |v| {
        v.memories.contains_key(&ram)
    });
    worker.action(SimulationAction::FillMemory {
        id: ram,
        start: 0,
        end: 1,
        value: 0x42,
    });
    wait(&mut worker, &mut workspace, |v| {
        v.memories[&ram].get(0) == 0x42
    });
    let old = worker.revision;
    workspace.set_simulation_mode(SimulationMode::Event);
    worker.synchronize(&mut workspace, true);
    assert_eq!(worker.revision, old + 1);
    assert_eq!(worker.context.as_ref().unwrap().mode, SimulationMode::Event);
    assert_eq!(workspace.simulation_mut().memories[&ram].get(0), 0x42);
    assert!(worker.ready_memories.contains(&ram));
    worker
        .shared
        .send(Command::Action(old, SimulationAction::ToggleEnabled));
    worker.action(SimulationAction::FillMemory {
        id: ram,
        start: 0,
        end: 1,
        value: 0x43,
    });
    wait(&mut worker, &mut workspace, |v| {
        v.memories[&ram].get(0) == 0x43
    });
    assert!(workspace.simulation_mut().enabled);
}

#[test]
fn attribute_edits_keep_the_visible_instance_and_reject_old_snapshots() {
    let (mut workspace, clock, ram) = workspace();
    let mut worker = Worker::new(|| {});
    worker.synchronize(&mut workspace, true);
    wait(&mut worker, &mut workspace, |v| {
        v.memories.contains_key(&ram)
    });
    let old = worker.revision;
    let mut project = workspace.project().clone();
    project
        .main_circuit_mut()
        .circuit
        .components
        .iter_mut()
        .find(|c| c.id == ram)
        .unwrap()
        .label = "renamed".into();
    // Apply the document change while preserving the UI state, as an attribute edit does.
    let view = std::mem::take(workspace.simulation_mut());
    workspace.replace_project(project);
    workspace.set_simulation(view);
    worker.synchronize(&mut workspace, true);
    assert_eq!(worker.revision, old + 1);
    assert!(workspace.simulation_mut().memories.contains_key(&ram));
    assert!(workspace.simulation_mut().clocks.contains(&clock));
    assert!(!worker.ready_memories.contains(&ram));
    worker.shared.latest.lock().unwrap().replace(Snapshot {
        revision: old,
        total_ticks: 0,
        view: SimulationView::default(),
    });
    worker.receive(&mut workspace);
    assert!(workspace.simulation_mut().memories.contains_key(&ram));
    let mut project = workspace.project().clone();
    let c = project
        .main_circuit_mut()
        .circuit
        .components
        .iter_mut()
        .find(|c| c.id == ram)
        .unwrap();
    *c = Component {
        id: ram,
        ..Component::new(Kind::Constant, c.position)
    };
    let view = std::mem::take(workspace.simulation_mut());
    workspace.replace_project(project);
    workspace.set_simulation(view);
    worker.synchronize(&mut workspace, true);
    assert!(!workspace.simulation_mut().memories.contains_key(&ram));
    worker.new_document();
    worker.synchronize(&mut workspace, true);
    assert!(workspace.simulation_mut().clocks.is_empty());
}
