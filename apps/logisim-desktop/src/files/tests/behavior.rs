use super::super::*;
use logisim_core::{Circuit, Component, Kind, Point, Project};
use logisim_sim::ProjectSimulator as Simulator;

#[test]
fn project_options_navigates_and_records_main_against_the_file_baseline() {
    let mut h = super::Harness::default();
    let temp = crate::tests::support::Temp::new();
    let path = temp.path("overview.circ");
    let mut project = Project::default();
    let child = project.add("second".into()).unwrap();
    let original = project.main;
    let pin = project
        .main_circuit_mut()
        .circuit
        .add_component(Component::new(Kind::Pin, Point::ZERO));
    h.files
        .storage
        .save(&project, &path, SaveMode::Current, false)
        .unwrap();
    h.ws.replace_project(project);
    let document = h.files.present(&h.ws);
    assert_eq!(document.path.as_deref(), Some(path.as_path()));
    assert!(!document.dirty);
    h.ws.set_document(document);

    let open_overview = |h: &mut super::Harness| {
        h.click("Project");
        h.click("Project Options…");
        assert!(h.frame(vec![]).iter().any(|(s, _)| s == "Project Options"));
    };
    let last_action = |h: &mut super::Harness, label| {
        let rows = h.frame(vec![]);
        let position = rows.iter().rev().find(|(s, _)| s == label).unwrap().1;
        h.click_at(position);
    };
    open_overview(&mut h);
    last_action(&mut h, "Open");
    assert!(!h.frame(vec![]).iter().any(|(s, _)| s == "Project Options"));
    assert_eq!(h.ws.active_circuit(), child);
    assert_eq!(h.ws.project().main, original);
    assert!(!h.files.present(&h.ws).dirty);
    open_overview(&mut h);
    // The first row is already main; its disabled action has no effect.
    h.click("Set as Main");
    assert!(!h.files.present(&h.ws).dirty);
    // Open the original again, then promote the other row without navigating to it.
    h.click("Open");
    h.frame(vec![]);
    crate::simulation::apply_project(
        &mut h.sim,
        &[],
        logisim_ui::SimulationAction::SetInput { id: pin, value: 1 },
    );
    open_overview(&mut h);
    last_action(&mut h, "Set as Main");
    assert_eq!(h.ws.active_circuit(), original);
    assert_eq!(h.ws.project().main, child);
    assert!(h.files.present(&h.ws).dirty);
    h.ws.set_document(h.files.present(&h.ws));
    assert!(h.frame(vec![]).iter().any(|(s, _)| s == "Unsaved changes"));
    assert_eq!(h.sim.input(pin).unwrap().number(), Some(1));
    h.click("Close");
    h.click_at(egui::pos2(1050., 730.));
    for (modifiers, main, dirty) in [
        (egui::Modifiers::COMMAND, original, false),
        (
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            child,
            true,
        ),
    ] {
        h.frame(vec![egui::Event::Key {
            key: egui::Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }]);
        assert_eq!(h.ws.project().main, main);
        assert_eq!(h.ws.active_circuit(), original);
        assert_eq!(h.files.present(&h.ws).dirty, dirty);
        h.frame(vec![]);
        assert_eq!(h.sim.input(pin).unwrap().number(), Some(1));
    }
    open_overview(&mut h);
    h.click("Load Plugin…");
    assert!(
        matches!(h.files.dialog, Some(Dialog::Picker(ref picker)) if picker.mode == Mode::Plugin)
    );
    assert!(!h.frame(vec![]).iter().any(|(s, _)| s == "Project Options"));
}

#[test]
fn label_bearing_placements_reopen_with_serif_and_joystick_keeps_its_native_font() {
    let dir = crate::tests::support::Temp::new();
    for (name, kind) in [
        ("Joystick", Kind::Joystick),
        ("PLA ROM", Kind::PlaRom),
        ("Pin", Kind::Pin),
    ] {
        let path = dir.path(&format!("{name}.circ"));
        let mut h = super::Harness::default();
        logisim_ui::configure(&h.ctx);
        h.ws.apply_settings(logisim_ui::Settings {
            default_font_family: logisim_ui::FontPreset::Serif,
            default_font_bold: true,
            default_font_italic: true,
            default_font_size: Some(24),
            ..h.ws.preferences()
        });
        h.click("Components");
        h.click("Search · Ctrl+F");
        h.frame(vec![egui::Event::Text(name[..name.len() - 1].into())]);
        h.click(name);
        h.click_at(egui::pos2(650., 300.));
        let placed = &h.ws.circuit().components;
        assert_eq!(placed.len(), 1, "{name}");
        assert_eq!(placed[0].kind, kind);
        let font = &placed[0].font;
        if kind.has_label() {
            assert_eq!(
                (font.family.as_str(), font.size, font.bold, font.italic),
                ("Serif", 24, true, true)
            );
        } else {
            assert_eq!(font.size, 12);
        }
        h.files
            .storage
            .save(h.ws.project(), &path, SaveMode::Current, false)
            .unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        if !kind.has_label() {
            assert!(!saved.contains("name=\"labelfont\"") && !saved.contains("name=\"font\""));
        }
        let (loaded, warnings) = Storage::new().load(&path).unwrap();
        assert!(warnings.is_empty());
        assert_eq!(&loaded.main_circuit().circuit.components, placed);
    }
}

#[test]
fn placing_and_reopening_unary_gates_preserves_their_sizes() {
    use egui::pos2;
    use logisim_core::{Gate, GateSize};
    let dir = crate::tests::support::Temp::new();
    let path = dir.path("unary.circ");
    let mut h = super::Harness::default();
    logisim_ui::configure(&h.ctx);
    h.click("Components");
    h.click("Gates");
    for (gate, default, x) in [
        (Gate::Not, GateSize::Medium, 650.0),
        (Gate::Buffer, GateSize::Wide, 750.0),
        (Gate::Not, GateSize::Wide, 850.0),
    ] {
        h.ws.apply_settings(logisim_ui::Settings {
            default_gate_size: default,
            ..h.ws.preferences()
        });
        h.click(gate.name());
        h.click_at(pos2(x, 300.0));
    }
    let components = &h.ws.circuit().components;
    let expected = [
        (Kind::Gate(Gate::Not), GateSize::Narrow),
        (Kind::Gate(Gate::Buffer), GateSize::Narrow),
        (Kind::Gate(Gate::Not), GateSize::Wide),
    ];
    assert!(
        components
            .iter()
            .map(|c| (c.kind, c.gate_options.size))
            .eq(expected)
    );
    h.files
        .storage
        .save(h.ws.project(), &path, SaveMode::Current, false)
        .unwrap();
    let (loaded, warnings) = Storage::new().load(&path).unwrap();
    assert!(warnings.is_empty());
    assert_eq!(&loaded.main_circuit().circuit.components, components);
}

#[test]
fn library_picker_is_additive_and_failure_or_cancel_keeps_document_baseline() {
    let temp = crate::tests::support::Temp::new();
    let current = temp.path("current.circ");
    let library = temp.path("File.circ");
    let source = logisim_file::serialize(&Project::default()).unwrap();
    std::fs::write(&library, &source).unwrap();
    let mut files = Files::default();
    let mut workspace = Workspace::default();
    let mut sim = Simulator::default();
    let ctx = egui::Context::default();
    files
        .storage
        .save(workspace.project(), &current, SaveMode::Current, false)
        .unwrap();
    let bytes = std::fs::read(&current).unwrap();
    for cancel in [true, false] {
        files.request(FileAction::ImportLibrary, &ctx, &mut workspace, &mut sim);
        let dialog = files.dialog.take().unwrap();
        assert!(matches!(&dialog, Dialog::Picker(p) if p.mode == Mode::Library));
        let response = if cancel {
            Response::Cancel
        } else {
            Response::Pick(temp.path("missing.circ"))
        };
        files.respond(dialog, response, &ctx, &mut workspace, &mut sim);
        assert!(!files.storage.dirty(workspace.project()));
        files.dialog = None;
    }
    files.request(FileAction::ImportLibrary, &ctx, &mut workspace, &mut sim);
    let dialog = files.dialog.take().unwrap();
    files.respond(
        dialog,
        Response::Pick(library.clone()),
        &ctx,
        &mut workspace,
        &mut sim,
    );
    assert!(!files.busy());
    assert!(files.storage.dirty(workspace.project()));
    assert_eq!(files.storage.path, Some(current.clone()));
    assert_eq!(workspace.project().circuits.len(), 2);
    assert_eq!(std::fs::read(&current).unwrap(), bytes);
    assert_eq!(std::fs::read_to_string(library).unwrap(), source);
}

#[test]
fn cancelling_save_before_new_preserves_document_and_clears_deferred_action() {
    let mut files = Files::default();
    let mut workspace = Workspace::default();
    let mut circuit = Circuit::default();
    circuit.add_component(Component::new(Kind::Pin, Point::new(100, 100)));
    let project = Project::single("unsaved", circuit);
    workspace.replace_project(project.clone());
    let mut sim = Simulator::default();
    let ctx = egui::Context::default();
    files.request(FileAction::New, &ctx, &mut workspace, &mut sim);
    let dialog = files.dialog.take().unwrap();
    assert!(matches!(dialog, Dialog::Unsaved(NextDocument::New)));
    files.respond(dialog, Response::Save, &ctx, &mut workspace, &mut sim);
    let dialog = files.dialog.take().unwrap();
    assert!(matches!(dialog, Dialog::Picker(_)));
    assert!(matches!(files.after_save, Some(NextDocument::New)));
    files.respond(dialog, Response::Cancel, &ctx, &mut workspace, &mut sim);
    assert!(!files.busy());
    assert!(files.after_save.is_none());
    assert_eq!(workspace.project(), &project);
    files.request(FileAction::New, &ctx, &mut workspace, &mut sim);
    let dialog = files.dialog.take().unwrap();
    files.respond(dialog, Response::Discard, &ctx, &mut workspace, &mut sim);
    assert_eq!(workspace.project(), &Project::default());
    assert!(!files.storage.dirty(workspace.project()));
}

#[test]
fn recent_navigation_keeps_the_chosen_target_through_dirty_save_as() {
    let temp = crate::tests::support::Temp::new();
    let target = temp.path("target.circ");
    let copy = temp.path("copy.circ");
    let project = Project::single("remembered", Circuit::default());
    std::fs::write(&target, logisim_file::serialize(&project).unwrap()).unwrap();
    let ctx = egui::Context::default();
    let mut ws = Workspace::default();
    let mut sim = Simulator::default();
    let mut files = Files::default();
    ws.set_recent(vec![target.to_str().unwrap().into()]);
    ws.replace_project(Project::single("unsaved", Circuit::default()));
    for answer in [Response::Cancel, Response::Save] {
        files.request(FileAction::OpenRecent(0), &ctx, &mut ws, &mut sim);
        let dialog = files.dialog.take().unwrap();
        assert!(matches!(dialog, Dialog::Unsaved(NextDocument::Recent(_))));
        files.respond(dialog, answer, &ctx, &mut ws, &mut sim);
        assert_eq!(ws.circuit_name(), "unsaved");
    }
    let dialog = files.dialog.take().unwrap();
    files.respond(
        dialog,
        Response::Pick(copy.clone()),
        &ctx,
        &mut ws,
        &mut sim,
    );
    assert_eq!(ws.circuit_name(), "remembered");
    assert_eq!(
        ws.recent(),
        &[target.to_str().unwrap(), copy.to_str().unwrap()]
    );
    files.request(FileAction::ClearRecent, &ctx, &mut ws, &mut sim);
    assert!(ws.recent().is_empty());
    assert!(!files.storage.dirty(ws.project()));
}

#[test]
fn startup_honors_cli_and_reopen_and_removes_only_the_failed_entry() {
    let temp = crate::tests::support::Temp::new();
    let good = temp.path("document.circ");
    let bad = temp.path("missing.circ");
    std::fs::write(
        &good,
        logisim_file::serialize(&Project::single("opened", Circuit::default())).unwrap(),
    )
    .unwrap();
    let mut ws = Workspace::default();
    let mut sim = Simulator::default();
    let mut files = Files::default();
    ws.set_recent(vec![
        bad.to_str().unwrap().into(),
        good.to_str().unwrap().into(),
    ]);
    files.startup(None, &mut ws, &mut sim);
    assert!(!files.busy() && files.storage.path.is_none());
    let mut values = ws.preferences();
    values.workspace.reopen_last = true;
    ws.apply_settings(values);
    files.startup(None, &mut ws, &mut sim);
    assert!(matches!(files.dialog, Some(Dialog::Message { .. })));
    assert!(files.storage.path.is_none());
    assert_eq!(ws.recent(), &[good.to_str().unwrap()]);
    files.dialog = None;
    files.startup(None, &mut ws, &mut sim);
    assert_eq!(ws.circuit_name(), "opened");
    ws.set_recent(vec![bad.to_str().unwrap().into()]);
    files.startup(Some(good.clone()), &mut ws, &mut sim);
    assert_eq!(
        ws.recent(),
        &[good.to_str().unwrap(), bad.to_str().unwrap()]
    );
    ws.set_recent(vec![
        bad.to_str().unwrap().into(),
        good.to_str().unwrap().into(),
    ]);
    files.request(
        FileAction::Save,
        &egui::Context::default(),
        &mut ws,
        &mut sim,
    );
    assert_eq!(ws.recent()[0], bad.to_str().unwrap());
}

#[test]
fn dark_artwork_preference_preserves_saved_project_bytes_on_reopen() {
    use logisim_core::{Geometry, Paint};
    let temp = crate::tests::support::Temp::new();
    let (mut project, wasm) = crate::tests::support::plugins::fixture("test.contrast", 1);
    let module = temp.path("ink.wasm");
    std::fs::write(&module, wasm).unwrap();
    let module = module.canonicalize().unwrap().to_str().unwrap().to_owned();
    project.plugin_sources[0].path = module.clone();
    project.plugin_sources[0].fallback = module;
    let child = project.add("Artwork".into()).unwrap();
    let def = project.circuit_mut(child).unwrap();
    let mut artwork = def.editable_appearance();
    artwork
        .add(
            Geometry::Poly {
                points: vec![Point::ZERO, Point::new(10, 30)],
                closed: false,
            },
            Paint::default(),
        )
        .unwrap();
    def.circuit.appearance = Some(artwork);
    let instance = project.instance(child, Point::new(220, 160)).unwrap();
    project.main_circuit_mut().circuit.add_component(instance);

    let path = temp.path("artwork.circ");
    let mut h = super::Harness::default();
    h.files
        .storage
        .save(&project, &path, SaveMode::Current, false)
        .unwrap();
    let original = std::fs::read(&path).unwrap();
    assert!(h.files.load(&path, &mut h.ws, &mut h.sim));
    let loaded = h.ws.project().clone();
    for enabled in [false, true] {
        h.ws.apply_settings(logisim_ui::Settings {
            theme: logisim_ui::ThemeMode::Dark,
            dark_artwork_contrast: enabled,
            ..h.ws.preferences()
        });
        h.frame(vec![]);
        assert!(!h.files.present(&h.ws).dirty);
        h.files
            .request(FileAction::Save, &h.ctx, &mut h.ws, &mut h.sim);
        assert!(!h.files.busy());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(h.files.load(&path, &mut h.ws, &mut h.sim));
        assert_eq!(h.ws.project(), &loaded);
    }
}
