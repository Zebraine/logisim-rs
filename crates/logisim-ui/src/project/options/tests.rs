use crate::{
    DocumentView, FileAction, Language, Workspace,
    i18n::{tr, with_language},
    tests::support::widgets::{click, frame_at_size, has},
};
use egui::{Context, Event, Modifiers, PointerButton, Pos2, Rect, Vec2, vec2};
use logisim_core::{
    Component, Kind, Point, Project,
    plugin::{PluginSource, api::Manifest},
};

fn overview(ctx: &Context, app: &mut Workspace, size: Vec2) -> Vec<(String, Rect)> {
    for _ in 0..2 {
        frame_at_size(ctx, app, vec![], size);
    }
    let labels = frame_at_size(ctx, app, vec![], size);
    let title = with_language(app.language(), || tr("Project Options"));
    let start = labels
        .iter()
        .rposition(|(text, _)| text == title)
        .expect("project overview");
    labels.into_iter().skip(start).collect()
}

fn rect(labels: &[(String, Rect)], text: &str) -> Rect {
    labels
        .iter()
        .find(|(s, _)| s == text)
        .unwrap_or_else(|| panic!("missing {text}"))
        .1
}

fn click_at(ctx: &Context, app: &mut Workspace, pos: Pos2, size: Vec2) {
    for pressed in [true, false] {
        frame_at_size(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    pressed,
                    button: PointerButton::Primary,
                    modifiers: Modifiers::NONE,
                },
            ],
            size,
        );
    }
}

fn scroll_body(ctx: &Context, app: &mut Workspace, size: Vec2, pointer: Pos2) {
    for _ in 0..6 {
        frame_at_size(
            ctx,
            app,
            vec![
                Event::PointerMoved(pointer),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: vec2(0., -3.),
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                    modifiers: Modifiers::NONE,
                },
            ],
            size,
        );
    }
}

#[test]
fn browsing_blocks_canvas_edits_and_opening_a_definition_keeps_history_empty() {
    let ctx = Context::default();
    let size = vec2(1440., 1600.);
    let mut app = Workspace::default();
    let mut project = Project::default();
    let child = project.add("part".into()).unwrap();
    let pin = project
        .main_circuit_mut()
        .circuit
        .add_component(Component::new(Kind::Pin, Point::ZERO));
    app.replace_project(project.clone());
    frame_at_size(&ctx, &mut app, vec![], size);
    app.editor.session.selection.insert(pin);
    app.canvas.camera.pan(vec2(45., 55.));
    let camera = app.canvas.camera;
    app.simulation_mut().ticks = 83;
    click(&ctx, &mut app, "Project");
    click(&ctx, &mut app, "Project Options…");
    assert!(app.project_options_open);
    frame_at_size(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(egui::pos2(1350., 900.)),
            Event::Key {
                key: egui::Key::Delete,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
        ],
        size,
    );
    assert_eq!(app.project(), &project);
    assert_eq!(app.editor.session.selection, [pin].into());
    assert_eq!(app.canvas.camera.offset, camera.offset);
    assert_eq!(app.canvas.camera.zoom, camera.zoom);
    assert_eq!(app.simulation_mut().ticks, 83);
    assert!(!app.editor.can_undo());
    let labels = overview(&ctx, &mut app, size);
    let pos = labels
        .iter()
        .rev()
        .find(|(s, _)| s == "Open")
        .unwrap()
        .1
        .center();
    click_at(&ctx, &mut app, pos, size);
    assert!(!app.project_options_open && !app.editor.can_undo());
    assert_eq!(app.active_circuit(), child);
    assert_eq!(app.project(), &project);
}

#[test]
fn missing_dependency_opens_file_workflow_without_editing_the_project() {
    let ctx = Context::default();
    let mut project = Project::default();
    project.plugin_sources.push(PluginSource {
        plugin: "test.module".into(),
        version: "1".into(),
        path: "previous/module.wasm".into(),
        fallback: String::new(),
    });
    let mut app = Workspace::default();
    app.replace_project(project.clone());
    app.project_options_open = true;
    click(&ctx, &mut app, "Resolve missing…");
    assert!(!app.project_options_open);
    assert_eq!(app.take_file_actions(), [FileAction::ResolvePlugins]);
    assert_eq!(app.project(), &project);
    assert!(!app.editor.can_undo());

    let source = &project.plugin_sources[0];
    app.set_plugin_catalogue(vec![
        Manifest {
            abi: 1,
            id: source.plugin.clone(),
            version: source.version.clone(),
            name: "Test module".into(),
            tools: Vec::new(),
        }
        .into(),
    ]);
    app.project_options_open = true;
    let labels = overview(&ctx, &mut app, vec2(1200., 820.));
    assert!(has(&labels, "Loaded"));
    assert!(!has(&labels, "Resolve missing…"));
    click(&ctx, &mut app, "Load Plugin…");
    assert_eq!(app.take_file_actions(), [FileAction::LoadPlugin]);
    assert!(!app.project_options_open);
}

#[test]
fn translated_repair_and_load_actions_fit_both_themes_and_viewports() {
    for language in Language::ALL {
        for theme in [crate::ThemeMode::Light, crate::ThemeMode::Dark] {
            for size in [vec2(720., 480.), vec2(1200., 820.)] {
                let ctx = Context::default();
                crate::configure(&ctx);
                let mut app = Workspace::default();
                app.apply_settings(crate::Settings {
                    theme,
                    language,
                    ..Default::default()
                });
                app.editor
                    .session
                    .project
                    .plugin_sources
                    .push(PluginSource {
                        plugin: "test.module".into(),
                        version: "1".into(),
                        path: format!("{}tool.wasm", "long/path/".repeat(40)),
                        fallback: "/backup/tool.wasm".into(),
                    });
                app.project_options_open = true;
                overview(&ctx, &mut app, size);
                scroll_body(&ctx, &mut app, size, size.to_pos2() / 2.);
                let labels = overview(&ctx, &mut app, size);
                let at = |key| rect(&labels, with_language(language, || tr(key)));
                let load = at("Load Plugin…");
                let repair = at("Resolve missing…");
                let close = at("Close");
                let bounds = ctx
                    .memory(|m| m.area_rect(egui::Id::unique("project-options")))
                    .unwrap();
                assert!(bounds.contains_rect(load) && bounds.contains_rect(repair));
                assert!(load.right() < repair.left() && repair.bottom() < close.top());
                click_at(&ctx, &mut app, repair.center(), size);
                assert_eq!(app.take_file_actions(), [FileAction::ResolvePlugins]);
                assert!(!app.project_options_open);
            }
        }
    }
}

#[test]
fn overview_counts_definitions_and_reports_exact_manifest_matches_and_live_library_groups() {
    let ctx = Context::default();
    let mut app = Workspace::default();
    let mut project = Project::default();
    let child = project.add("Child".into()).unwrap();
    project
        .circuit_mut(child)
        .unwrap()
        .circuit
        .add_component(Component::new(Kind::Pin, Point::ZERO));
    project
        .main_circuit_mut()
        .circuit
        .add_wire(Point::ZERO, Point::new(50, 0));
    let dependency = |id: &str, version: &str| PluginSource {
        plugin: id.into(),
        version: version.into(),
        path: format!("/project/{id}.wasm"),
        fallback: String::new(),
    };
    let mut other = dependency("test.other", "1");
    other.fallback = "/archive/other.wasm".into();
    project.plugin_sources = vec![dependency("test.present", "1"), other];
    app.replace_project(project);
    app.editor.switch_circuit(child);
    let import = app
        .project()
        .with_library(&Project::default(), "Library {}")
        .unwrap();
    app.apply_library_import(import);
    app.set_plugin_catalogue(
        ["test.present", "test.other"]
            .map(|id| {
                Manifest {
                    abi: 1,
                    id: id.into(),
                    version: if id == "test.present" { "1" } else { "2" }.into(),
                    name: "Live module".into(),
                    tools: Vec::new(),
                }
                .into()
            })
            .to_vec(),
    );
    app.set_document(DocumentView {
        path: Some("/project/example.circ".into()),
        dirty: true,
        ..Default::default()
    });
    app.project_options_open = true;
    let size = vec2(1200., 820.);
    let labels = overview(&ctx, &mut app, size);
    for (header, count) in [
        ("Circuits", "3"),
        ("Components", "1"),
        ("Wires", "1"),
        ("Plugins", "2"),
        ("Libraries", "1"),
    ] {
        let heading = rect(&labels, header);
        let (value, _) = labels
            .iter()
            .filter(|(_, r)| r.top() >= heading.bottom() && (r.left() - heading.left()).abs() < 1.)
            .min_by(|a, b| a.1.top().total_cmp(&b.1.top()))
            .unwrap();
        assert_eq!(value, count, "{header}");
    }
    assert!(has(&labels, "/project/example.circ") && has(&labels, "Unsaved changes"));
    assert!(has(&labels, "Child") && has(&labels, "main"));
    assert!(has(&labels, "Active") && has(&labels, "Main"));
    for (id, status) in [("test.present", "Loaded"), ("test.other", "Missing")] {
        assert!((rect(&labels, id).center().y - rect(&labels, status).center().y).abs() < 2.);
    }
    let pointer = rect(&labels, "test.other").center();
    frame_at_size(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(pointer),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0., -180.),
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Unknown,
                modifiers: Modifiers::NONE,
            },
        ],
        size,
    );
    assert!(has(&overview(&ctx, &mut app, size), "/archive/other.wasm"));
    assert_eq!(
        app.imported_library_counts().collect::<Vec<_>>(),
        [("Library {}", 1)]
    );
    app.editor.undo();
    assert_eq!(app.imported_library_counts().count(), 0);
    app.editor.redo();
    assert_eq!(
        app.imported_library_counts().collect::<Vec<_>>(),
        [("Library {}", 1)]
    );
    app.set_plugin_catalogue(vec![]);
    let labels = overview(&ctx, &mut app, size);
    assert_eq!(labels.iter().filter(|(s, _)| s == "Missing").count(), 2);
}

#[test]
fn bilingual_overview_bounds_truncation_empty_states_and_disabled_single_main() {
    for lang in Language::ALL {
        let ctx = Context::default();
        crate::configure(&ctx);
        let mut app = Workspace {
            project_options_open: true,
            ..Default::default()
        };
        app.set_language(lang);
        let translate = |label| with_language(lang, || tr(label));
        let project_path = format!("/work/{}/example.circ", "long-subdirectory/".repeat(24));
        let name = "Circuit {0} · ".repeat(32);
        app.editor.session.project.main_circuit_mut().name = name.clone();
        for size in [vec2(1200., 820.), vec2(720., 480.)] {
            app.set_document(DocumentView::default());
            let empty = overview(&ctx, &mut app, size);
            for key in ["Untitled", "No changes"] {
                assert!(has(&empty, translate(key)), "{lang:?} {size}: {key}");
            }
            app.set_document(DocumentView {
                path: Some(project_path.clone().into()),
                ..Default::default()
            });
            let labels = overview(&ctx, &mut app, size);
            let bounds = ctx
                .memory(|m| m.area_rect(egui::Id::unique("project-options")))
                .unwrap();
            assert!(Rect::from_min_size(Pos2::ZERO, size).contains_rect(bounds));
            assert!(bounds.width() <= size.x * 0.9 + 1.);
            let path_rect = rect(&labels, &project_path);
            assert!(
                bounds.contains_rect(path_rect)
                    && path_rect.right() <= rect(&labels, translate("No changes")).left()
            );
            for (_, r) in labels.iter().filter(|(s, _)| s == &name) {
                assert!(bounds.contains_rect(*r));
            }
            let open = rect(&labels, translate("Open"));
            let main = rect(&labels, translate("Set as Main"));
            let close = rect(&labels, translate("Close"));
            assert!(
                open.right() < main.left()
                    && bounds.contains_rect(main)
                    && bounds.contains_rect(close)
            );
            assert!(main.bottom() < close.top());
            let pos = main.center();
            click_at(&ctx, &mut app, pos, size);
            assert!(app.project_options_open && !app.editor.can_undo());
            scroll_body(&ctx, &mut app, size, bounds.center());
            let bottom = overview(&ctx, &mut app, size);
            for key in [
                "No plugin dependencies.",
                "No imported libraries.",
                "Load Plugin…",
            ] {
                assert!(has(&bottom, translate(key)), "{lang:?} {size}: {key}");
            }
            assert!(!has(&bottom, translate("Resolve missing…")));
            assert_eq!(rect(&bottom, translate("Close")), close);
        }
    }
}
