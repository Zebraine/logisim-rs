use super::super::*;
use crate::{tests::support::Temp, tests::support::plugins::fixture};
use logisim_sim::ProjectSimulator as Simulator;
use std::fs;
#[test]
fn missing_and_invalid_confirm_keep_static_definitions_and_valid_plugins_running() {
    let temp = Temp::new();
    let (mut project, a) = fixture("test.a", 1);
    let (b, bytes_b) = fixture("test.b", 0);
    let mut second = b.main_circuit().circuit.components[0].clone();
    second.position.x += 100;
    project.main_circuit_mut().circuit.add_component(second);
    project.plugin_sources[0].path = "a.wasm".into();
    project.plugin_sources.extend(b.plugin_sources);
    fs::write(temp.path("a.wasm"), a).unwrap();
    fs::write(temp.path("b.wasm"), &bytes_b).unwrap();
    let path = temp.path("circuit.circ");
    let original = logisim_file::serialize(&project).unwrap();
    fs::write(&path, &original).unwrap();
    for replacement in ["", "still-missing.wasm", "a.wasm", "b.wasm"] {
        let mut files = Files::default();
        let mut workspace = Workspace::default();
        let mut sim = Simulator::default();
        let ctx = egui::Context::default();
        // A previously loaded module must not hide a missing dependency in this file.
        files.plugins.load(&bytes_b).unwrap();
        files.load(&path, &mut workspace, &mut sim);
        if !replacement.is_empty() {
            let dialog = files.dialog.take().unwrap();
            files.respond(dialog, Response::Cancel, &ctx, &mut workspace, &mut sim);
            let active = files.plugins.clone();
            files.request(FileAction::ResolvePlugins, &ctx, &mut workspace, &mut sim);
            assert!(active.same(&files.plugins));
        }
        let Dialog::Plugins(mut resolution) =
            files.dialog.take().expect("missing dependency dialog")
        else {
            panic!("wrong dialog")
        };
        assert!(resolution.rows[0].loaded.is_some());
        assert!(resolution.rows[1].loaded.is_none());
        let before = workspace.project().clone();
        resolution.rows[1].location = replacement.into();
        files.respond(
            Dialog::Plugins(resolution),
            Response::ConfirmPlugins,
            &ctx,
            &mut workspace,
            &mut sim,
        );
        assert!(!files.busy());
        sim.synchronize(workspace.project(), workspace.project().main);
        sim.advance(32);
        let c = &workspace.project().main_circuit().circuit;
        let view = sim.view(&[]).unwrap();
        assert_eq!(
            view.port_value(c.components[0].id, 0).unwrap().number(),
            Some(1)
        );
        if replacement == "b.wasm" {
            assert!(sim.plugin_failures().is_empty());
            assert_eq!(
                view.port_value(c.components[1].id, 0).unwrap().number(),
                Some(0)
            );
            assert!(files.storage.dirty(workspace.project()));
        } else {
            assert_eq!(sim.plugin_failures().len(), 1);
            assert_eq!(view.plugin_graphics(c.components[1].id).unwrap(), &[]);
            assert_eq!(
                view.port_value(c.components[1].id, 0).unwrap().bit(0),
                logisim_sim::Bit::Error
            );
            assert_eq!(workspace.project(), &before);
            assert!(!files.storage.dirty(workspace.project()));
            let presented = crate::simulation::present_project(&c.clone(), &sim, &[]);
            assert!(presented.plugin_errors.contains_key(&c.components[1].id));
        }
        assert_eq!(workspace.project().circuits, before.circuits);
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
    }
}
#[test]
fn browse_cancel_keeps_resolution_and_new_clears_old_catalogue() {
    let temp = Temp::new();
    let (project, _) = fixture("test.a", 1);
    let path = temp.path("circuit.circ");
    fs::write(&path, logisim_file::serialize(&project).unwrap()).unwrap();
    let mut files = Files::default();
    let mut workspace = Workspace::default();
    let mut sim = Simulator::default();
    let ctx = egui::Context::default();
    files.load(&path, &mut workspace, &mut sim);
    let dialog = files.dialog.take().unwrap();
    files.respond(
        dialog,
        Response::BrowsePlugin(0),
        &ctx,
        &mut workspace,
        &mut sim,
    );
    let dialog = files.dialog.take().unwrap();
    assert!(matches!(dialog, Dialog::PluginPicker { .. }));
    files.respond(dialog, Response::Cancel, &ctx, &mut workspace, &mut sim);
    let dialog = files.dialog.take().unwrap();
    assert!(matches!(dialog, Dialog::Plugins(_)));
    files.respond(
        dialog,
        Response::ConfirmPlugins,
        &ctx,
        &mut workspace,
        &mut sim,
    );
    assert!(!files.busy());
    files.request(FileAction::New, &ctx, &mut workspace, &mut sim);
    assert_eq!(workspace.project(), &logisim_core::Project::default());
    assert_eq!(files.plugins.manifests().count(), 0);
}

#[test]
fn project_options_repairs_only_on_confirm_and_records_one_undoable_source_edit() {
    let temp = Temp::new();
    let (project, wasm) = fixture("test.repair", 1);
    let path = temp.path("project.circ");
    let original = logisim_file::serialize(&project).unwrap();
    fs::write(&path, &original).unwrap();
    let candidate = temp.path("candidate.wasm");
    fs::write(&candidate, wasm).unwrap();
    let mut h = super::Harness::default();
    assert!(h.files.load(&path, &mut h.ws, &mut h.sim));
    h.click("Cancel");
    let before = h.ws.project().clone();
    let registry = h.files.plugins.clone();
    let repair = |h: &mut super::Harness| {
        h.click("Project");
        h.click("Project Options…");
        h.click("Resolve missing…");
        assert!(matches!(h.files.dialog, Some(Dialog::Plugins(_))));
        assert!(!h.frame(vec![]).iter().any(|(s, _)| s == "Project Options"));
        h.click("Browse…");
        assert!(matches!(h.files.dialog, Some(Dialog::PluginPicker { .. })));
        h.click("candidate.wasm");
        h.click("Open");
        h.click("Check");
        assert!(h.frame(vec![]).iter().any(|(s, _)| s == "Plugin ready"));
    };
    repair(&mut h);
    assert!(registry.same(&h.files.plugins));
    h.click("Cancel");
    assert!(!h.files.busy() && registry.same(&h.files.plugins));
    assert_eq!(h.ws.project(), &before);
    assert!(!h.ws.plugin_dependencies().next().unwrap().1);
    assert!(!h.files.present(&h.ws).dirty);
    assert_eq!(h.sim.plugin_failures().len(), 1);
    assert_eq!(fs::read_to_string(&path).unwrap(), original);

    repair(&mut h);
    h.click("Confirm");
    h.frame(vec![]);
    assert!(!h.files.busy() && h.files.present(&h.ws).dirty);
    assert_eq!(h.ws.project().circuits, before.circuits);
    let (source, loaded) = h.ws.plugin_dependencies().next().unwrap();
    assert!(loaded);
    assert_eq!(Path::new(&source.path), candidate.canonicalize().unwrap());
    assert_eq!(h.files.plugins.manifests().count(), 1);
    assert!(h.sim.plugin_failures().is_empty());
    let output = before.main_circuit().circuit.components[0].id;
    assert_eq!(
        h.sim
            .view(&[])
            .unwrap()
            .port_value(output, 0)
            .unwrap()
            .number(),
        Some(1)
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), original);

    let active = h.files.plugins.clone();
    h.files
        .request(FileAction::ResolvePlugins, &h.ctx, &mut h.ws, &mut h.sim);
    assert!(!h.files.busy() && active.same(&h.files.plugins));
    h.click_at(egui::pos2(1050., 730.));
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Z,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    }]);
    assert_eq!(h.ws.project(), &before);
    assert!(!h.files.present(&h.ws).dirty);
}

#[test]
fn reappearing_recorded_module_resolves_without_an_empty_dialog_or_a_document_edit() {
    let temp = Temp::new();
    let (project, wasm) = fixture("test.reappeared", 0);
    let path = temp.path("project.circ");
    fs::write(&path, logisim_file::serialize(&project).unwrap()).unwrap();
    let mut h = super::Harness::default();
    assert!(h.files.load(&path, &mut h.ws, &mut h.sim));
    h.click("Cancel");
    let before = h.ws.project().clone();
    fs::write(temp.path("missing.wasm"), wasm).unwrap();
    h.files
        .request(FileAction::ResolvePlugins, &h.ctx, &mut h.ws, &mut h.sim);
    assert!(!h.files.busy());
    assert!(h.ws.plugin_dependencies().next().unwrap().1);
    assert_eq!(h.ws.project(), &before);
    assert!(!h.files.present(&h.ws).dirty);
}
