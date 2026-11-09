use super::support::{click, finish_jobs};
use crate::generation::*;
use crate::{Language, i18n, tests::support::widgets::frame};
use egui::{Event, Modifiers, PointerButton, Pos2};
use logisim_core::{Component, Gate, Id, Kind, Point, Project};

fn right_click(ctx: &egui::Context, w: &mut Workspace, pos: Pos2) {
    for pressed in [true, false] {
        frame(
            ctx,
            w,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Secondary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
}

fn workspace() -> (Workspace, Analysis, Id, Id, Id) {
    let mut a = Generator::default().analysis;
    a.apply_expression(0, "A ^ B").unwrap();
    let mut project = Project::single("child", a.generate().unwrap());
    let child = project.main;
    project.main_circuit_mut().circuit.appearance =
        Some(project.main_circuit().editable_appearance());
    let parent = project.add("parent".into()).unwrap();
    let other = project.add("other".into()).unwrap();
    project.circuit_mut(other).unwrap().circuit = a.generate().unwrap();
    project.main = parent;
    for (id, point) in [(child, Point::new(200, 200)), (other, Point::new(400, 200))] {
        let c = project.instance(id, point).unwrap();
        project
            .circuit_mut(parent)
            .unwrap()
            .circuit
            .add_component(c);
    }
    project
        .circuit_mut(parent)
        .unwrap()
        .circuit
        .add_component(Component::new(Kind::Register, Point::new(600, 200)));
    let mut w = Workspace::default();
    w.replace_project(project);
    (w, a, child, parent, other)
}

#[test]
fn single_project_entry_can_build_into_an_empty_circuit_and_undo() {
    for language in Language::ALL {
        let mut w = Workspace::default();
        w.set_language(language);
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let t = |key| i18n::with_language(language, || tr(key).to_owned());
        click(&ctx, &mut w, &t("Project"));
        let labels = frame(&ctx, &mut w, vec![]);
        assert!(
            !labels
                .iter()
                .any(|(text, _)| text == "Generate Circuit…" || text == "生成电路…")
        );
        click(&ctx, &mut w, &t("Analyze Circuit…"));
        assert!(w.take_analysis_request().is_none());
        assert!(w.generator.analysis.is_expression_mode());
        assert!(w.generator.analysis.inputs().is_empty());
        assert!(w.generator.analysis.outputs().is_empty());
        assert_eq!(w.generator.tab, Tab::Inputs);
        assert_eq!(
            w.generator.target.as_ref().unwrap().id(),
            w.active_circuit()
        );
        let before = w.project().clone();
        for name in ["A", "B"] {
            w.generator.inputs.name = name.into();
            click(&ctx, &mut w, &t("Add"));
        }
        click(&ctx, &mut w, &t("Output variables"));
        w.generator.outputs.name = "Y".into();
        click(&ctx, &mut w, &t("Add"));
        click(&ctx, &mut w, &t("Expression"));
        w.generator.draft = "A ^ B".into();
        click(&ctx, &mut w, &t("Apply"));
        click(&ctx, &mut w, &t("Build Circuit"));
        assert_eq!(w.project().circuits.len(), 1);
        assert!(
            w.circuit()
                .components
                .iter()
                .any(|c| c.kind == Kind::Gate(Gate::Xor))
        );
        let built = w.project().clone();
        w.editor.undo();
        assert_eq!(w.project(), &before);
        w.editor.redo();
        assert_eq!(w.project(), &built);
    }
}

#[test]
fn sidebar_analysis_binds_the_clicked_definition_through_build_memory_and_history() {
    for language in Language::ALL {
        let (mut w, a, child, parent, other) = workspace();
        w.set_language(language);
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let t = |key| i18n::with_language(language, || tr(key).to_owned());
        for _ in 0..3 {
            frame(&ctx, &mut w, vec![]);
        }
        let labels = frame(&ctx, &mut w, vec![]);
        let pos = labels
            .iter()
            .find(|(s, _)| s == "child")
            .unwrap()
            .1
            .center();
        right_click(&ctx, &mut w, pos);
        click(&ctx, &mut w, &t("Analyze Circuit…"));
        let (serial, target) = w.take_analysis_request().unwrap();
        assert_eq!(target, child);
        assert_eq!(w.active_circuit(), parent);
        // Navigation while the worker runs must not rebind its result.
        w.editor.switch_circuit(other);
        w.receive_analysis(serial, Ok(a));
        assert_eq!(w.generator.target.as_ref().unwrap().id(), child);
        assert_eq!(w.active_circuit(), other);
        let before = w.editor.snapshot();
        click(&ctx, &mut w, &t("Use NAND gates only"));
        click(&ctx, &mut w, &t("Build Circuit"));
        assert!(!w.generator.open);
        assert_eq!(w.active_circuit(), child);
        frame(&ctx, &mut w, vec![]);
        for c in &w.circuit().components {
            assert!(w.canvas.rect.contains(w.canvas.screen(c.position)));
        }
        assert_eq!(w.project().circuit(parent), before.project.circuit(parent));
        assert_eq!(w.project().circuit(other), before.project.circuit(other));
        assert_ne!(w.project().circuit(child), before.project.circuit(child));
        let built = w.project().clone();
        w.editor.undo();
        assert_eq!(w.project(), &before.project);
        assert_eq!(w.active_circuit(), other);
        w.editor.redo();
        assert_eq!(w.project(), &built);
        w.editor.switch_circuit(parent);
        w.request_circuit_analysis(child);
        assert!(w.take_analysis_request().is_none());
        assert_eq!(w.generator.applied, "A ^ B");
        assert_eq!(w.generator.target.as_ref().unwrap().id(), child);
        assert_eq!(w.active_circuit(), parent);
        // A copy also uses the selected child's function without touching the parent.
        w.generator.replace_target = false;
        w.generator.request_build();
        finish_jobs(&mut w);
        assert_eq!(w.project().circuits.len(), 4);
        assert_eq!(w.project().circuit(parent), built.circuit(parent));
        assert_eq!(w.project().circuit(child), built.circuit(child));
    }
}

#[test]
fn canvas_analysis_keeps_the_right_clicked_instance_as_the_pointer_moves() {
    for language in Language::ALL {
        let (mut w, _, child, parent, other) = workspace();
        w.set_language(language);
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        for _ in 0..3 {
            frame(&ctx, &mut w, vec![]);
        }
        let instances: Vec<_> = w
            .circuit()
            .components
            .iter()
            .filter(|c| matches!(c.kind, Kind::Subcircuit(_)))
            .cloned()
            .collect();
        let t = |key| i18n::with_language(language, || tr(key).to_owned());
        for (i, target) in [(0, child), (1, other)] {
            w.editor.selection.clear();
            w.editor.selection.insert(instances[1 - i].id);
            let pos = w.canvas.screen(instances[i].position);
            right_click(&ctx, &mut w, pos);
            let other_pos = w.canvas.screen(instances[1 - i].position);
            frame(&ctx, &mut w, vec![Event::PointerMoved(other_pos)]);
            click(&ctx, &mut w, &t("Analyze Circuit…"));
            assert_eq!(w.take_analysis_request().unwrap().1, target);
            assert_eq!(w.active_circuit(), parent);
            w.analysis.cancel();
        }
    }
}

#[test]
fn missing_or_removed_analysis_targets_preserve_the_document_and_draft() {
    let mut w = Workspace::default();
    let a = Generator::default().analysis;
    let mut project = Project::default();
    let child = project.add("child".into()).unwrap();
    project.circuit_mut(child).unwrap().circuit = a.generate().unwrap();
    w.replace_project(project);
    w.generator.draft = "unfinished &".into();
    w.request_circuit_analysis(child);
    let (serial, target) = w.take_analysis_request().unwrap();
    assert_eq!(target, child);
    w.editor.project.remove(child).unwrap();
    let before = w.project().clone();
    w.receive_analysis(serial, Ok(a));
    assert_eq!(w.generator.draft, "unfinished &");
    assert!(!w.generator.open);
    assert_eq!(w.project(), &before);
    w.request_circuit_analysis(child);
    assert!(w.take_analysis_request().is_none());
    assert!(w.analysis.open());
    assert_eq!(w.generator.draft, "unfinished &");
}
