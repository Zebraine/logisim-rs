use super::*;
use logisim_core::{Component, Kind, Point, Project};

#[test]
fn localized_dialog_applies_expressions_and_blocks_stale_generation() {
    use crate::{
        Language, i18n,
        i18n_tests::{click, frame, frame_at_size},
    };
    use logisim_core::generation::Cell;
    for language in Language::ALL {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut w = Workspace::default();
        w.set_language(language);
        w.open_generator();
        w.generator.tab = Tab::Expression;
        let label = |key: &str| i18n::with_language(language, || tr(key).to_owned());
        for size in [egui::vec2(1200.0, 820.0), egui::vec2(720.0, 480.0)] {
            let mut labels = vec![];
            for _ in 0..3 {
                labels = frame_at_size(&ctx, &mut w, vec![], size);
            }
            for key in [
                "Combinational Circuit Generator",
                "Input variables",
                "Output variables",
                "Clear",
                "Use NAND gates only",
                "Build Circuit",
                "Close",
            ] {
                let rect = labels
                    .iter()
                    .find(|(text, _)| *text == label(key))
                    .unwrap_or_else(|| panic!("Missing {key} at {size:?}: {labels:?}"))
                    .1;
                assert!(
                    rect.left() >= 0.0
                        && rect.right() <= size.x
                        && rect.top() >= 0.0
                        && rect.bottom() <= size.y,
                    "{key} {rect:?}"
                );
            }
        }
        let before = w.generator.analysis.clone();
        assert!(!w.generator.nand_only);
        click(&ctx, &mut w, &label("Use NAND gates only"));
        assert!(w.generator.nand_only);
        assert_eq!(w.generator.analysis, before);
        w.generator.draft = "A ^ B".into();
        click(&ctx, &mut w, &label("Build Circuit"));
        assert_eq!(w.project().circuits.len(), 1);
        click(&ctx, &mut w, &label("Apply"));
        assert_eq!(
            w.generator.analysis.outputs()[0].values(),
            [Cell::Zero, Cell::One, Cell::One, Cell::Zero]
        );
        w.generator.draft = "A &".into();
        click(&ctx, &mut w, &label("Apply"));
        assert!(w.generator.error.is_some());
        click(&ctx, &mut w, &label("Build Circuit"));
        assert_eq!(w.project().circuits.len(), 1);
        frame(
            &ctx,
            &mut w,
            vec![egui::Event::Key {
                key: egui::Key::Z,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }],
        );
        assert_eq!(w.project().circuits.len(), 1);
        click(&ctx, &mut w, &label("Revert"));
        click(&ctx, &mut w, &label("Build Circuit"));
        assert!(!w.generator.open);
        assert_eq!(w.circuit_name(), "generated");
        assert_eq!(w.project().circuits.len(), 2);
        assert_eq!(
            w.circuit()
                .components
                .iter()
                .filter(|c| c.kind == Kind::Gate(logisim_core::Gate::Nand))
                .count(),
            4
        );
        let built = w.circuit().clone();
        w.editor.undo();
        assert_eq!(w.project().circuits.len(), 1);
        w.editor.redo();
        assert_eq!(w.circuit(), &built);
    }
}

#[test]
fn nand_budget_failure_is_atomic_and_can_be_retried_with_standard_gates() {
    use crate::i18n_tests::click;
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut w = Workspace::default();
    let mut expression = "A".to_owned();
    for _ in 0..8 {
        expression = format!("({expression}) | ({expression})");
    }
    w.generator
        .analysis
        .apply_expression(0, &expression)
        .unwrap();
    w.generator.refresh();
    w.open_generator();
    let before = w.project().clone();
    click(&ctx, &mut w, "Use NAND gates only");
    click(&ctx, &mut w, "Build Circuit");
    assert!(w.generator.open);
    assert!(w.generator.error.is_some());
    assert_eq!(w.project(), &before);
    assert!(!w.editor.can_undo());
    click(&ctx, &mut w, "Use NAND gates only");
    click(&ctx, &mut w, "Build Circuit");
    assert!(!w.generator.open);
    assert_eq!(w.project().circuits.len(), 2);
    assert!(
        w.circuit()
            .components
            .iter()
            .any(|c| c.kind == Kind::Gate(logisim_core::Gate::Or))
    );
    w.replace_project(Project::default());
    assert!(!w.generator.nand_only);
}

#[test]
fn build_is_one_edit_and_failure_does_not_change_project_or_history() {
    let mut w = Workspace::default();
    w.editor.place(Point::new(100, 100));
    w.finish_edit();
    let before = w.editor.snapshot();
    let mut generator = Generator::default();
    generator.analysis.apply_expression(0, "A ^ B").unwrap();
    let circuit = generator.analysis.generate().unwrap();
    assert!(
        w.editor
            .install_generated("main".into(), circuit.clone())
            .is_err()
    );
    assert_eq!(w.project(), &before.project);
    w.editor.install_generated("xor".into(), circuit).unwrap();
    let id = w.active_circuit();
    let built = w.circuit().clone();
    assert_ne!(id, before.active);
    assert_eq!(w.project().main, before.project.main);
    w.editor.undo();
    assert_eq!(w.active_circuit(), before.active);
    assert_eq!(w.project().circuits, before.project.circuits);
    w.editor.redo();
    assert_eq!(w.active_circuit(), id);
    assert_eq!(w.circuit(), &built);
    // The result remains an ordinary editable subcircuit, including its Pin interface.
    w.editor.switch_circuit(before.active);
    let instance = w.project().instance(id, Point::new(400, 400)).unwrap();
    assert_eq!(instance.interface.ports.len(), 3);
    w.editor.circuit.add_component(instance);
    w.finish_edit();
    w.project().validate().unwrap();
}

#[test]
fn project_budget_failure_preserves_selection_and_draft_can_be_retried() {
    let mut w = Workspace::default();
    let mut project = Project::default();
    for i in 0..16 {
        let id = if i == 0 {
            project.main
        } else {
            project.add(format!("c{i}")).unwrap()
        };
        for j in 0..4096 {
            project
                .circuit_mut(id)
                .unwrap()
                .circuit
                .add_component(Component::new(Kind::Pin, Point::new(j * 10, 100)));
        }
    }
    w.replace_project(project);
    let before = w.project().clone();
    let result = w.editor.install_generated(
        "generated".into(),
        Generator::default().analysis.generate().unwrap(),
    );
    assert!(result.is_err());
    assert_eq!(w.project(), &before);
    assert!(!w.editor.can_undo());
    w.open_generator();
    w.generator.draft = "A &".into();
    assert!(w.generator.pending());
    w.generator.open = false;
    w.open_generator();
    assert_eq!(w.generator.draft, "A &");
    w.replace_project(Project::default());
    assert!(!w.generator.open);
    assert!(!w.generator.pending());
}
