use super::*;
use logisim_core::{Component, Point, plugin::api::interaction::Key};
use logisim_plugin::Plugin;

fn setup() -> (Engine, Context, Box<PluginInputTarget>, Id) {
    let plugin = Plugin::load(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../plugins/input/input.wasm"
    )))
    .unwrap();
    let definition = plugin.manifest().components().remove(0);
    let binding = plugin.library().interaction.keyboards.remove(0);
    let mut plugins = Registry::default();
    plugins.insert(plugin).unwrap();
    let mut project = Project::default();
    let child = project.add("Keys".into()).unwrap();
    let id = project
        .circuit_mut(child)
        .unwrap()
        .circuit
        .add_component(Component::from_plugin(definition.clone(), Point::ZERO));
    let a = project.instance(child, Point::ZERO).unwrap();
    let b = project.instance(child, Point::new(400, 0)).unwrap();
    let first = project.main_circuit_mut().circuit.add_component(a);
    let second = project.main_circuit_mut().circuit.add_component(b);
    let context = Context {
        revision: 1,
        root: project.main,
        circuit: child,
        path: vec![first],
        project: Arc::new(project),
        plugins,
        language: Language::English,
        mode: SimulationMode::default(),
    };
    let target = Box::new(PluginInputTarget {
        root: context.root,
        circuit: child,
        path: vec![first],
        id,
        component: Arc::new(definition),
        binding,
    });
    let mut engine = Engine::default();
    engine.command(Command::Configure(context.clone()));
    settle(&mut engine);
    (engine, context, target, second)
}
fn settle(engine: &mut Engine) {
    for _ in 0..128 {
        engine.advance();
        if engine.sim.pending_count() == 0 {
            break;
        }
    }
    assert_eq!(engine.sim.pending_count(), 0);
    assert!(engine.sim.error().is_none());
}
fn value(engine: &Engine, target: &PluginInputTarget, port: usize) -> u32 {
    engine
        .sim
        .view(&target.path)
        .unwrap()
        .port_value(target.id, port)
        .unwrap()
        .number()
        .unwrap()
}
fn send(engine: &mut Engine, revision: u64, target: &PluginInputTarget, event: InputEvent) {
    engine.command(Command::Action(
        revision,
        SimulationAction::PluginInput {
            target: Box::new(target.clone()),
            event,
        },
    ));
    settle(engine);
}
#[test]
fn input_is_ordered_and_old_instance_is_released_before_context_switch() {
    let (mut engine, mut context, target, second) = setup();
    send(
        &mut engine,
        1,
        &target,
        InputEvent::Key {
            key: Key::ArrowUp,
            pressed: true,
        },
    );
    assert_eq!(value(&engine, &target, 3), 1);
    send(
        &mut engine,
        1,
        &target,
        InputEvent::Key {
            key: Key::ArrowUp,
            pressed: false,
        },
    );
    assert_eq!(value(&engine, &target, 3), 0);
    assert_eq!(value(&engine, &target, 8), 2);
    send(
        &mut engine,
        1,
        &target,
        InputEvent::Key {
            key: Key::ArrowUp,
            pressed: true,
        },
    );
    context.path = vec![second];
    context.revision = 2;
    engine.command(Command::Configure(context.clone()));
    settle(&mut engine);
    assert_eq!(value(&engine, &target, 3), 0);
    assert_eq!(value(&engine, &target, 8), 0);
    let mut new = target.clone();
    new.path = vec![second];
    for revision in [1, 2] {
        send(
            &mut engine,
            revision,
            &target,
            InputEvent::Key {
                key: Key::ArrowUp,
                pressed: true,
            },
        );
    }
    assert_eq!(value(&engine, &target, 3), 0);
    assert_eq!(value(&engine, &new, 3), 0);
    send(
        &mut engine,
        2,
        &new,
        InputEvent::Key {
            key: Key::ArrowRight,
            pressed: true,
        },
    );
    assert_eq!(value(&engine, &new, 3), 8);
    context.path = target.path.clone();
    context.revision = 3;
    engine.command(Command::Configure(context));
    settle(&mut engine);
    assert_eq!(value(&engine, &new, 3), 0);
    assert_eq!(value(&engine, &target, 3), 0);
}
#[test]
fn suspension_releases_keys_and_stale_or_forged_targets_are_rejected() {
    let (mut engine, mut context, target, _) = setup();
    send(
        &mut engine,
        1,
        &target,
        InputEvent::Key {
            key: Key::Space,
            pressed: true,
        },
    );
    engine.command(Command::Suspend(true));
    engine.sim.advance(128);
    assert_eq!(value(&engine, &target, 3), 0);
    engine.command(Command::Action(
        1,
        SimulationAction::PluginInput {
            target: target.clone(),
            event: InputEvent::Key {
                key: Key::Space,
                pressed: true,
            },
        },
    ));
    engine.command(Command::Suspend(false));
    settle(&mut engine);
    assert_eq!(value(&engine, &target, 3), 0);
    for change in 0..4 {
        let mut bad = target.clone();
        match change {
            0 => bad.root += 100,
            1 => bad.circuit += 100,
            2 => Arc::make_mut(&mut bad.component).version = "bad".into(),
            _ => bad.binding.action = "missing".into(),
        }
        send(
            &mut engine,
            1,
            &bad,
            InputEvent::Key {
                key: Key::Space,
                pressed: true,
            },
        );
        assert_eq!(value(&engine, &target, 3), 0);
    }
    send(
        &mut engine,
        1,
        &target,
        InputEvent::Key {
            key: Key::Space,
            pressed: true,
        },
    );
    context.revision = 2;
    context.language = Language::SimplifiedChinese;
    engine.command(Command::Configure(context.clone()));
    settle(&mut engine);
    assert_eq!(value(&engine, &target, 3), 16);
    context.revision = 3;
    Arc::make_mut(&mut context.project)
        .circuit_mut(context.circuit)
        .unwrap()
        .circuit
        .components[0]
        .plugin
        .as_mut()
        .unwrap()
        .properties
        .insert("key0".into(), "W".into());
    engine.command(Command::Configure(context));
    settle(&mut engine);
    assert_eq!(value(&engine, &target, 3), 0);
}

#[test]
fn switching_roots_does_not_leave_held_keys_in_cached_instances() {
    let (mut engine, mut context, target, _) = setup();
    send(
        &mut engine,
        1,
        &target,
        InputEvent::Key {
            key: Key::Enter,
            pressed: true,
        },
    );
    let root = Arc::make_mut(&mut context.project)
        .add("Other".into())
        .unwrap();
    let mut away = context.clone();
    away.revision = 2;
    away.root = root;
    away.circuit = root;
    away.path.clear();
    engine.command(Command::Configure(away));
    settle(&mut engine);
    context.revision = 3;
    engine.command(Command::Configure(context));
    settle(&mut engine);
    assert_eq!(value(&engine, &target, 3), 0);
    assert_eq!(value(&engine, &target, 8), 0);
}
