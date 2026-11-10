use super::support::*;
use crate::editor::Tool;
use crate::*;
use egui::{Event, Key, Modifiers};
use logisim_core::{
    Component, Point,
    plugin::api::{
        self,
        interaction::{InputEvent, Interaction, Key as PluginKey, Keyboard},
        presentation::Library,
    },
};
fn setup_input() -> (egui::Context, Workspace, logisim_core::Id) {
    let (ctx, mut app) = setup();
    let manifest = api::Manifest {
        abi: 3,
        id: "test.input".into(),
        version: "1".into(),
        name: "Input".into(),
        tools: vec![api::Tool {
            id: "keys".into(),
            name: "Keys".into(),
            bounds: [[0, 0], [200, 100]],
            ports: vec![],
            graphics: vec![],
            properties: vec![],
            actions: vec![api::Action {
                id: "input".into(),
                label: "Input".into(),
                argument: Some("Event".into()),
            }],
        }],
    };
    let id = app
        .editor
        .session
        .circuit
        .add_component(Component::from_plugin(
            manifest.components().remove(0),
            Point::new(200, 200),
        ));
    app.set_plugin_catalogue(vec![Library {
        manifest,
        presentation: Default::default(),
        interaction: Interaction {
            keyboards: vec![Keyboard {
                tool: "keys".into(),
                action: "input".into(),
                keys: vec![PluginKey::W, PluginKey::ArrowUp, PluginKey::Space],
            }],
        },
    }]);
    app.editor.set_tool(Tool::Poke);
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    let p = app.canvas.screen(Point::new(260, 240));
    click(&ctx, &mut app, p);
    assert!(app.editor.simulation.plugin_keyboard.is_some());
    (ctx, app, id)
}
fn transition(key: Key, pressed: bool, repeat: bool, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat,
        modifiers,
    }
}
fn events(app: &mut Workspace) -> Vec<InputEvent> {
    app.take_simulation_actions()
        .into_iter()
        .map(|a| match a {
            SimulationAction::PluginInput { event, .. } => event,
            other => panic!("Unexpected action {other:?}"),
        })
        .collect()
}
#[test]
fn plugin_keys_preserve_order_ignore_repeat_and_leave_document_untouched() {
    let (ctx, mut app, id) = setup_input();
    let before = app.circuit().clone();
    let camera = app.canvas.camera.offset;
    frame(
        &ctx,
        &mut app,
        vec![
            transition(Key::W, true, false, Modifiers::NONE),
            Event::Text("w".into()),
            transition(Key::W, true, true, Modifiers::NONE),
            transition(Key::ArrowUp, true, false, Modifiers::NONE),
            transition(Key::W, false, false, Modifiers::CTRL),
        ],
        Modifiers::NONE,
    );
    let actions = app.take_simulation_actions();
    let expected = [
        (PluginKey::W, true),
        (PluginKey::ArrowUp, true),
        (PluginKey::W, false),
    ];
    assert_eq!(actions.len(), 3, "{actions:?}");
    for (a, (key, pressed)) in actions.iter().zip(expected) {
        let SimulationAction::PluginInput { target, event } = a else {
            panic!()
        };
        assert_eq!(target.id, id);
        assert_eq!(*event, InputEvent::Key { key, pressed });
        assert!(target.matches(
            app.project(),
            app.simulation_root(),
            app.active_circuit(),
            app.simulation_path()
        ));
    }
    key(&ctx, &mut app, Key::Space, Modifiers::NONE);
    assert_eq!(
        events(&mut app),
        [
            InputEvent::Key {
                key: PluginKey::Space,
                pressed: true
            },
            InputEvent::Key {
                key: PluginKey::Space,
                pressed: false
            }
        ]
    );
    assert_eq!(app.circuit(), &before);
    assert_eq!(app.canvas.camera.offset, camera);
    assert_eq!(app.editor.tool, Tool::Poke);
    key(&ctx, &mut app, Key::R, Modifiers::COMMAND);
    assert_eq!(app.take_simulation_actions(), [SimulationAction::Reset]);
}
#[test]
fn plugin_capture_releases_on_focus_modal_tool_instance_and_definition_changes() {
    for case in 0..9 {
        let (ctx, mut app, _) = setup_input();
        frame(
            &ctx,
            &mut app,
            vec![transition(Key::ArrowUp, true, false, Modifiers::NONE)],
            Modifiers::NONE,
        );
        let first = events(&mut app);
        assert!(
            app.editor.simulation.plugin_keyboard.is_some(),
            "after press {case}: {first:?}"
        );
        match case {
            0 => frame(
                &ctx,
                &mut app,
                vec![Event::WindowFocused(false)],
                Modifiers::NONE,
            ),
            1 => {
                app.settings_open = true;
                frame(&ctx, &mut app, vec![], Modifiers::NONE);
            }
            2 => app.editor.set_tool(Tool::Select),
            3 => key(&ctx, &mut app, Key::Tab, Modifiers::NONE),
            4 => key(&ctx, &mut app, Key::Escape, Modifiers::NONE),
            5 => {
                let p = app.canvas.screen(Point::new(700, 400));
                click(&ctx, &mut app, p);
            }
            6 => {
                app.editor.session.context.path.push(999);
                frame(&ctx, &mut app, vec![], Modifiers::NONE);
            }
            7 => {
                app.set_plugin_catalogue(vec![]);
                frame(&ctx, &mut app, vec![], Modifiers::NONE);
            }
            _ => {
                app.editor.session.circuit.components[0]
                    .plugin
                    .as_mut()
                    .unwrap()
                    .version = "2".into();
                frame(&ctx, &mut app, vec![], Modifiers::NONE);
            }
        }
        assert!(app.editor.simulation.plugin_keyboard.is_none(), "{case}");
        assert_eq!(events(&mut app), [InputEvent::ReleaseAll {}], "{case}");
        frame(&ctx, &mut app, vec![], Modifiers::NONE);
        assert!(events(&mut app).is_empty());
    }
}
