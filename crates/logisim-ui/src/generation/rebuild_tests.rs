use super::{
    tests::{click, finish_jobs},
    *,
};
use crate::{Language, i18n, i18n_tests::frame_at_size};
use logisim_core::{Facing, Id, Point, Project};

fn workspace(language: Language) -> (Workspace, Id, Id) {
    let mut analysis = Generator::default().analysis;
    analysis.apply_expression(0, "A ^ B").unwrap();
    let mut project = Project::single("child", analysis.generate().unwrap());
    let child = project.main;
    project.main_circuit_mut().circuit.appearance =
        Some(project.main_circuit().editable_appearance());
    let parent = project.add("parent".into()).unwrap();
    project.main = parent;
    let mut component = project.instance(child, Point::new(300, 200)).unwrap();
    component.facing = Facing::North;
    let instance = project
        .circuit_mut(parent)
        .unwrap()
        .circuit
        .add_component(component);
    let mut w = Workspace::default();
    w.set_language(language);
    w.replace_project(project);
    w.editor.enter_instance(instance);
    w.request_analysis();
    let serial = w.take_analysis_request().unwrap();
    w.receive_analysis(serial, Ok(analysis));
    finish_jobs(&mut w);
    (w, child, instance)
}

#[test]
fn localized_build_replaces_the_analysis_target_and_restores_context_with_one_undo() {
    for language in Language::ALL {
        let (mut w, child, instance) = workspace(language);
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let label = |key| i18n::with_language(language, || tr(key).to_owned());
        let replace = i18n::with_language(language, || tf!("Replace circuit: {}", "child"));
        assert!(w.generator.replace_target);
        let before = w.editor.snapshot();
        for size in [egui::vec2(1200.0, 820.0), egui::vec2(720.0, 480.0)] {
            let mut labels = vec![];
            for _ in 0..3 {
                labels = frame_at_size(&ctx, &mut w, vec![], size);
            }
            let rect = labels.iter().find(|(s, _)| s == &replace).unwrap().1;
            assert!(rect.left() >= 0.0 && rect.right() <= size.x && rect.bottom() <= size.y);
            assert!(!labels.iter().any(|(s, _)| s == &label("New circuit name")));
        }
        click(&ctx, &mut w, &label("Use NAND gates only"));
        click(&ctx, &mut w, &label("Build Circuit"));
        assert!(!w.generator.open);
        assert_eq!(w.active_circuit(), child);
        assert_eq!(w.editor.path, [instance]);
        assert_eq!(w.editor.root, before.root);
        assert_eq!(w.project().circuits.len(), 2);
        assert_eq!(
            w.project().circuit(before.root),
            before.project.circuit(before.root)
        );
        assert_eq!(w.circuit().appearance, before.circuit.appearance);
        let built = w.editor.snapshot();
        assert_ne!(built.circuit, before.circuit);
        w.editor.undo();
        assert_eq!(w.project(), &before.project);
        assert_eq!(w.editor.path, before.path);
        w.editor.redo();
        assert_eq!(w.project(), &built.project);
        assert_eq!(w.editor.path, built.path);
    }
}

#[test]
fn opting_out_creates_a_copy_without_replacing_the_source() {
    let (mut w, child, _) = workspace(Language::English);
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let before = w.project().clone();
    click(&ctx, &mut w, "Replace circuit: child");
    assert!(!w.generator.replace_target);
    click(&ctx, &mut w, "Build Circuit");
    assert_eq!(w.project().circuits.len(), 3);
    assert_eq!(w.circuit_name(), "analyzed");
    assert_eq!(w.project().circuit(child), before.circuit(child));
    w.editor.undo();
    assert_eq!(w.project(), &before);
}

#[test]
fn changed_or_removed_targets_fail_atomically_instead_of_overwriting_another_circuit() {
    let (mut w, child, _) = workspace(Language::English);
    w.editor.circuit.label.text = "changed after analysis".into();
    w.editor.sync_project();
    let before = w.editor.snapshot();
    w.generator.request_build();
    finish_jobs(&mut w);
    assert!(w.generator.open);
    assert!(
        w.generator
            .error
            .as_ref()
            .unwrap()
            .message
            .contains("changed")
    );
    assert_eq!(w.project(), &before.project);
    assert_eq!(w.editor.circuit, before.circuit);
    assert!(!w.editor.can_undo());
    let parent = w.editor.root;
    w.editor.switch_circuit(parent);
    w.editor.circuit.components.clear();
    w.editor.sync_project();
    w.editor.project.remove(child).unwrap();
    let before = w.editor.snapshot();
    w.generator.request_build();
    finish_jobs(&mut w);
    assert!(w.generator.error.is_some());
    assert_eq!(w.project(), &before.project);
    assert_eq!(w.active_circuit(), parent);
    assert!(!w.editor.can_undo());
}
