use crate::tests::support::widgets::*;
use crate::{editor::Tool, *};
use egui::{Context, Event, Modifiers, PointerButton};
use logisim_core::Point;

fn manifest() -> logisim_core::plugin::api::Manifest {
    use logisim_core::plugin::api;
    let tool = api::Tool {
        id: "first".into(),
        name: "Plugin tool A".into(),
        bounds: [[-20, -20], [20, 20]],
        ports: vec![api::Port {
            id: "out".into(),
            label: "Output".into(),
            at: [20, 0],
            width: 4,
            output: true,
        }],
        graphics: vec![api::Shape::Text {
            at: [-20, -20],
            text: "Pin".into(),
            size: 10,
            color: [0, 0, 0, 255],
        }],
        properties: vec![api::Property {
            id: "mode".into(),
            label: "Address".into(),
            default: "first".into(),
            kind: api::PropertyKind::Choice {
                values: vec!["first".into(), "second".into()],
            },
        }],
        actions: vec![api::Action {
            id: "go".into(),
            label: "User action".into(),
            argument: None,
        }],
    };
    let manifest = api::Manifest {
        abi: 1,
        id: "test.graphics".into(),
        version: "1".into(),
        name: "User library".into(),
        tools: vec![
            tool.clone(),
            api::Tool {
                id: "second".into(),
                name: "Plugin tool B".into(),
                ..tool
            },
        ],
    };
    manifest.validate().unwrap();
    manifest
}

#[test]
fn generic_plugin_palette_properties_actions_and_strings_in_both_languages() {
    let manifest = manifest();
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        app.set_plugin_catalogue(vec![manifest.clone()]);
        click(&ctx, &mut app, "Plugin tool A");
        assert_eq!(
            app.editor.template.plugin.as_ref().unwrap().tool.id,
            "first"
        );
        click(&ctx, &mut app, "Plugin tool B");
        assert_eq!(
            app.editor.template.plugin.as_ref().unwrap().tool.id,
            "second"
        );
        let pos = app.canvas.screen(Point::new(200, 200));
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut app,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        app.editor.set_tool(Tool::Select);
        let c = app
            .editor
            .session
            .circuit
            .components
            .last()
            .unwrap()
            .clone();
        assert_eq!(c.plugin.as_ref().unwrap().tool.id, "second");
        assert_eq!(c.ports(), vec![c.position + Point::new(20, 0)]);
        app.editor.session.selection = [c.id].into();
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, "Address")); // Plugin-owned text is never translated.
        assert!(has(&labels, "Pin"));
        click(&ctx, &mut app, "first");
        click(&ctx, &mut app, "second");
        app.finish_edit();
        assert_eq!(
            app.editor
                .session
                .circuit
                .component(c.id)
                .unwrap()
                .plugin
                .as_ref()
                .unwrap()
                .properties["mode"],
            "second"
        );
        app.editor.undo();
        assert_eq!(
            app.editor
                .session
                .circuit
                .component(c.id)
                .unwrap()
                .plugin
                .as_ref()
                .unwrap()
                .properties["mode"],
            "first"
        );
        app.editor.session.selection = [c.id].into();
        click(&ctx, &mut app, "User action");
        assert!(app.editor.simulation.actions.iter().any(|a| matches!(a, SimulationAction::PluginAction { id, action, argument } if *id==c.id && action=="go" && argument.is_empty())));
        app.editor.simulation.view.plugin_error_count = 1;
        app.editor
            .simulation
            .view
            .plugin_errors
            .insert(c.id, "Missing test plugin".into());
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, "Missing test plugin"));
        assert!(
            i18n::with_language(language, || app.editor.simulation.status()).contains(
                if language == Language::English {
                    "plugin error"
                } else {
                    "插件错误"
                }
            )
        );
        let before = app.project().clone();
        let sources = vec![logisim_core::plugin::PluginSource {
            plugin: manifest.id.clone(),
            version: manifest.version.clone(),
            path: "/plugins/repaired.wasm".into(),
            fallback: String::new(),
        }];
        app.set_plugin_sources(sources.clone());
        assert_eq!(app.project().plugin_sources, sources);
        assert_eq!(app.project().circuits, before.circuits);
        app.editor.undo();
        assert_eq!(app.project(), &before);
        app.editor.redo();
        assert_eq!(app.project().plugin_sources, sources);
    }
}

#[test]
fn plugin_tool_and_orientation_edits_preserve_wires_and_undo() {
    use logisim_core::{Component, Facing};
    let mut manifest = manifest();
    manifest.tools[1].bounds = [[0, -10], [60, 10]];
    manifest.tools[1].ports[0].at = [60, 0];
    manifest.tools[1]
        .properties
        .push(logisim_core::plugin::api::Property {
            id: "extra".into(),
            label: "Extra option".into(),
            default: "true".into(),
            kind: logisim_core::plugin::api::PropertyKind::Boolean,
        });
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        app.set_plugin_catalogue(vec![manifest.clone()]);
        let mut c = Component::from_plugin(manifest.components()[0].clone(), Point::new(200, 200));
        c.label = "User label".into();
        c.plugin
            .as_mut()
            .unwrap()
            .properties
            .insert("mode".into(), "second".into());
        let id = app.editor.session.circuit.add_component(c);
        app.editor
            .session
            .circuit
            .add_wire(Point::new(220, 200), Point::new(240, 200));
        app.editor.session.selection.insert(id);
        let before = app.circuit().clone();
        // Hide the palette so tool names refer to the attribute selector.
        click(&ctx, &mut app, "User library");
        let labels = frame(&ctx, &mut app, vec![]);
        let pos = labels
            .iter()
            .rev()
            .find(|(text, _)| text == "Plugin tool A")
            .unwrap()
            .1
            .center();
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut app,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        click(&ctx, &mut app, "Plugin tool B");
        app.finish_edit();
        let edited = app.circuit().component(id).unwrap();
        assert_eq!(edited.plugin.as_ref().unwrap().tool.id, "second");
        assert_eq!(edited.plugin.as_ref().unwrap().properties["mode"], "second");
        assert_eq!(edited.plugin.as_ref().unwrap().properties["extra"], "true");
        assert_eq!(edited.ports(), vec![Point::new(260, 200)]);
        assert_eq!(edited.label, "User label");
        assert_eq!(app.circuit().wires, before.wires);
        app.editor.undo();
        assert_eq!(app.circuit(), &before);
        app.editor.redo();
        app.editor.session.selection.insert(id);
        let before_rotation = app.circuit().clone();
        let tr = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        click(&ctx, &mut app, &tr("East"));
        click(&ctx, &mut app, &tr("South"));
        app.finish_edit();
        let rotated = app.circuit().component(id).unwrap();
        assert_eq!(rotated.facing, Facing::South);
        assert_eq!(rotated.ports(), vec![Point::new(200, 260)]);
        let bounds = crate::drawing::bounds(rotated);
        assert_eq!(bounds.min, egui::pos2(190.0, 200.0));
        assert_eq!(bounds.max, egui::pos2(210.0, 260.0));
        assert_eq!(app.circuit().wires, before.wires);
        app.editor.undo();
        assert_eq!(app.circuit(), &before_rotation);
        app.editor.redo();
        app.editor.duplicate();
        assert_eq!(app.circuit().components.len(), 2);
        assert_eq!(
            app.circuit().components[0].plugin,
            app.circuit().components[1].plugin
        );
        app.editor.undo();
        assert_eq!(app.circuit().components.len(), 1);
    }
}
