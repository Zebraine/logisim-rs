use super::*;
use logisim_core::{Component, Kind, Point, Project};

#[test]
fn localized_dialog_applies_expressions_and_blocks_stale_generation() {
    use crate::{
        Language, i18n,
        i18n_tests::{frame, frame_at_size},
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
                "Format",
                "Sum of products",
                "Simplify",
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

#[test]
fn circuit_analysis_success_loads_table_and_builds_as_one_edit() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut w = Workspace::default();
    let mut analysis = Generator::default().analysis;
    analysis.apply_expression(0, "A ^ B").unwrap();
    w.replace_circuit(analysis.generate().unwrap());
    let before = w.project().clone();
    w.request_analysis();
    let serial = w.take_analysis_request().unwrap();
    w.receive_analysis(serial, Ok(analysis.clone()));
    assert!(w.generator.open);
    assert_eq!(w.generator.tab, Tab::Table);
    assert_eq!(w.generator.analysis, analysis);
    assert_eq!(w.project(), &before);
    assert!(!w.editor.can_undo());
    click(&ctx, &mut w, "Use NAND gates only");
    click(&ctx, &mut w, "Build Circuit");
    assert_eq!(w.circuit_name(), "main");
    assert_eq!(w.project().circuits.len(), 1);
    w.editor.undo();
    assert_eq!(w.project().circuits, before.circuits);
    w.editor.redo();
    assert_eq!(w.circuit_name(), "main");
}

#[test]
fn circuit_analysis_cancel_error_stale_results_and_partial_import_preserve_draft() {
    use crate::{Language, i18n, i18n_tests::frame_at_size};
    for language in Language::ALL {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut w = Workspace::default();
        w.set_language(language);
        w.generator.draft = "unfinished &".into();
        let before = w.project().clone();
        w.request_analysis();
        let serial = w.take_analysis_request().unwrap();
        let label = |key: &str| i18n::with_language(language, || tr(key).to_owned());
        for _ in 0..3 {
            frame_at_size(&ctx, &mut w, vec![], egui::vec2(720.0, 480.0));
        }
        click(&ctx, &mut w, &label("Cancel"));
        w.receive_analysis(serial, Ok(Analysis::default()));
        assert_eq!(w.generator.draft, "unfinished &");
        assert!(!w.generator.open);
        w.request_analysis();
        let serial = w.take_analysis_request().unwrap();
        w.receive_analysis(
            serial,
            Err(Error {
                message: "Circuit analysis does not support timing or stateful components:",
                detail: "child / register".into(),
            }),
        );
        let mut labels = vec![];
        for _ in 0..3 {
            labels = frame_at_size(&ctx, &mut w, vec![], egui::vec2(720.0, 480.0));
        }
        let rect = labels
            .iter()
            .find(|(text, _)| *text == label("Close"))
            .unwrap()
            .1;
        assert!(rect.bottom() <= 480.0);
        assert!(labels.iter().any(|(text, _)| *text == "child / register"));
        click(&ctx, &mut w, &label("Close"));
        assert_eq!(w.generator.draft, "unfinished &");
        assert_eq!(w.project(), &before);
        assert!(!w.editor.can_undo());
        w.request_analysis();
        let old = w.take_analysis_request().unwrap();
        w.replace_project(Project::default());
        w.request_analysis();
        let new = w.take_analysis_request().unwrap();
        assert_ne!(old, new);
        w.receive_analysis(old, Ok(Analysis::default()));
        assert!(w.analysis_pending(new));
        assert!(!w.generator.open);
        click(&ctx, &mut w, &label("Cancel"));
        w.document.partial = true;
        w.request_analysis();
        assert!(w.take_analysis_request().is_none());
    }
}

pub(super) fn finish_jobs(w: &mut Workspace) {
    while let Some(request) = w.take_generation_request() {
        w.receive_generation(request.serial, request.execute(|| false));
    }
}
pub(super) fn click(ctx: &egui::Context, w: &mut Workspace, label: &str) {
    finish_jobs(w);
    crate::i18n_tests::click(ctx, w, label);
    finish_jobs(w);
}

#[test]
fn cancelled_and_replaced_generation_results_never_change_the_draft_or_document() {
    let mut w = Workspace::default();
    w.open_generator();
    w.generator.request_minimize();
    let old = w.take_generation_request().unwrap();
    w.generator.analysis.apply_expression(0, "A ^ B").unwrap();
    w.generator.refresh();
    w.receive_generation(old.serial, old.execute(|| false));
    assert_eq!(w.generator.applied, "A ^ B");
    finish_jobs(&mut w);
    w.generator.request_build();
    let old = w.take_generation_request().unwrap();
    let before = w.project().clone();
    w.generator.cancel_computation();
    w.receive_generation(old.serial, old.execute(|| false));
    assert_eq!(w.project(), &before);
    w.generator.request_build();
    let old = w.take_generation_request().unwrap();
    w.replace_project(Project::default());
    w.open_generator();
    w.generator.request_minimize();
    let new = w.take_generation_request().unwrap();
    w.receive_generation(old.serial, old.execute(|| false));
    assert!(w.generation_pending(new.serial));
    assert_eq!(w.project().circuits.len(), 1);
}

#[test]
fn work_budget_failure_keeps_the_table_and_expression_generation_available() {
    use logisim_core::generation::{Cell, MAX_TABLE_INPUTS};
    let mut a = Analysis::default();
    for i in 0..MAX_TABLE_INPUTS {
        a.add_input(format!("I{i}")).unwrap();
    }
    a.add_output("Y".into()).unwrap();
    let expression = a.inputs().join(" ^ ");
    a.apply_expression(0, &expression).unwrap();
    let mut rng = 12345u32;
    for row in 0..a.rows() {
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        a.set_cell(0, row, Cell::from(rng >> 31 != 0)).unwrap();
    }
    let mut w = Workspace::default();
    w.generator.load_analysis(a.clone());
    w.open_generator();
    finish_jobs(&mut w);
    assert!(
        w.generator
            .error
            .as_ref()
            .unwrap()
            .message
            .contains("limit")
    );
    assert_eq!(w.generator.analysis, a);
    w.generator
        .analysis
        .apply_expression(0, &expression)
        .unwrap();
    w.generator.refresh();
    finish_jobs(&mut w);
    assert!(!w.generator.pending());
    w.generator.request_build();
    finish_jobs(&mut w);
    assert_eq!(w.project().circuits.len(), 2);
}

#[test]
fn cached_forms_survive_output_switches_rename_and_restore_but_not_table_changes() {
    use logisim_core::generation::Cell;
    let mut w = Workspace::default();
    w.open_generator();
    w.generator.analysis.set_cell(0, 3, Cell::One).unwrap();
    w.generator.refresh();
    finish_jobs(&mut w);
    w.generator.form = Form::ProductOfSums;
    w.generator.request_simplify();
    finish_jobs(&mut w);
    for form in [Form::SumOfProducts, Form::ProductOfSums] {
        assert!(
            w.generator
                .cache
                .cached(&w.generator.analysis, 0, form)
                .is_some()
        );
        w.generator.form = form;
        w.generator.request_simplify();
        assert!(!w.generator.computing);
        assert!(w.take_generation_request().is_none());
    }
    w.generator.analysis.rename(true, 0, "NewA".into()).unwrap();
    w.generator.refresh();
    assert!(!w.generator.computing);
    w.generator.analysis.set_cell(0, 0, Cell::Zero).unwrap();
    w.generator.refresh();
    assert!(w.generator.computing);
    finish_jobs(&mut w);
    w.generator.analysis.set_cell(0, 0, Cell::DontCare).unwrap();
    w.generator.refresh();
    assert!(!w.generator.computing);
}

#[test]
fn localized_expression_mode_builds_without_tables_and_materializes_on_request() {
    use crate::{Language, i18n, i18n_tests::frame_at_size};
    for language in Language::ALL {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut w = Workspace::default();
        w.set_language(language);
        w.open_generator();
        let label = |key: &str| i18n::with_language(language, || tr(key).to_owned());
        click(&ctx, &mut w, &label("Expression mode"));
        assert!(w.generator.analysis.is_expression_mode());
        w.generator.draft = "A ^ B".into();
        click(&ctx, &mut w, &label("Apply"));
        assert_eq!(w.generator.analysis.rows(), 0);
        click(&ctx, &mut w, &label("Truth Table"));
        click(&ctx, &mut w, &label("Create Truth Table"));
        assert_eq!(w.generator.analysis.rows(), 4);
        assert_eq!(w.generator.applied, "A ^ B");
        click(&ctx, &mut w, &label("Expression mode"));
        for i in 2..32 {
            w.generator.analysis.add_input(format!("I{i}")).unwrap();
        }
        w.generator
            .analysis
            .apply_expression(0, &w.generator.analysis.inputs().join(" ^ "))
            .unwrap();
        w.generator.refresh();
        assert!(!w.generator.computing);
        w.generator.tab = Tab::Table;
        let mut labels = vec![];
        for _ in 0..3 {
            labels = frame_at_size(&ctx, &mut w, vec![], egui::vec2(720.0, 480.0));
        }
        assert!(labels.iter().any(|(s, _)| *s
            == label(
                "Complete truth tables support at most 12 inputs. The expressions are preserved."
            )));
        assert!(
            !labels
                .iter()
                .any(|(s, _)| *s == label("Create Truth Table"))
        );
        w.generator.request_mode(false);
        finish_jobs(&mut w);
        assert!(w.generator.analysis.is_expression_mode());
        assert_eq!(w.generator.analysis.rows(), 0);
        assert!(w.generator.error.is_some());
        click(&ctx, &mut w, &label("Build Circuit"));
        assert_eq!(w.project().circuits.len(), 2);
        assert_eq!(
            w.circuit()
                .components
                .iter()
                .filter(|c| c.kind == Kind::Gate(logisim_core::Gate::Xor))
                .count(),
            31
        );
    }
}

#[test]
fn variable_add_controls_follow_table_expression_and_output_limits() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut w = Workspace::default();
    w.open_generator();
    for i in 2..12 {
        w.generator.inputs.name = format!("I{i}");
        click(&ctx, &mut w, "Add");
    }
    assert_eq!(w.generator.analysis.inputs().len(), 12);
    w.generator.inputs.name = "I12".into();
    click(&ctx, &mut w, "Add");
    assert_eq!(w.generator.analysis.inputs().len(), 12);
    click(&ctx, &mut w, "Expression mode");
    click(&ctx, &mut w, "Input variables");
    for i in 12..32 {
        w.generator.inputs.name = format!("I{i}");
        click(&ctx, &mut w, "Add");
    }
    assert_eq!(w.generator.analysis.inputs().len(), 32);
    click(&ctx, &mut w, "Output variables");
    for i in 1..32 {
        w.generator.outputs.name = format!("O{i}");
        click(&ctx, &mut w, "Add");
    }
    assert_eq!(w.generator.analysis.outputs().len(), 32);
    for _ in 0..3 {
        crate::i18n_tests::frame_at_size(&ctx, &mut w, vec![], egui::vec2(720.0, 480.0));
    }
    let labels = crate::i18n_tests::frame_at_size(&ctx, &mut w, vec![], egui::vec2(720.0, 480.0));
    assert!(
        labels
            .iter()
            .any(|(label, rect)| label == "Add" && rect.bottom() <= 480.0)
    );
}

#[test]
fn localized_inline_simplification_edits_the_draft_and_requires_apply() {
    use crate::{Language, i18n, i18n_tests::frame};
    for language in Language::ALL {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut w = Workspace::default();
        w.set_language(language);
        w.generator
            .analysis
            .apply_expression(0, "A & B | A & ~B")
            .unwrap();
        w.generator.refresh();
        w.open_generator();
        w.generator.tab = Tab::Expression;
        let label = |key: &str| i18n::with_language(language, || tr(key).to_owned());
        let before = w.generator.analysis.clone();
        let original = w.generator.applied.clone();
        assert!(!w.generator.computing);
        let labels = frame(&ctx, &mut w, vec![]);
        assert_eq!(
            labels
                .iter()
                .filter(|(text, _)| *text == label("Simplify"))
                .count(),
            1
        );
        assert!(!labels.iter().any(|(text, _)| text == "Minimized"));
        click(&ctx, &mut w, &label("Sum of products"));
        click(&ctx, &mut w, &label("Product of sums"));
        assert_eq!(w.generator.form, Form::ProductOfSums);
        assert!(!w.generator.computing);
        assert_eq!(w.generator.draft, original);
        click(&ctx, &mut w, &label("Simplify"));
        assert_eq!(w.generator.draft, "A");
        assert_eq!(w.generator.applied, original);
        assert_eq!(w.generator.analysis, before);
        click(&ctx, &mut w, &label("Build Circuit"));
        assert_eq!(w.project().circuits.len(), 1);
        click(&ctx, &mut w, &label("Revert"));
        assert_eq!(w.generator.draft, original);
        // The button simplifies the edit, not the previously applied function.
        w.generator.draft = "B & A | B & ~A".into();
        click(&ctx, &mut w, &label("Simplify"));
        assert_eq!(w.generator.draft, "B");
        assert_eq!(w.generator.analysis, before);
        click(&ctx, &mut w, &label("Apply"));
        assert_eq!(w.generator.applied, "B");
        assert_eq!(
            w.generator
                .analysis
                .expression(0)
                .unwrap()
                .text(w.generator.analysis.inputs()),
            "B"
        );
        w.generator.draft = "A &".into();
        let before = w.generator.analysis.clone();
        click(&ctx, &mut w, &label("Simplify"));
        assert!(w.generator.error.is_some());
        assert_eq!(w.generator.draft, "A &");
        assert_eq!(w.generator.applied, "B");
        assert_eq!(w.generator.analysis, before);
    }
}

#[test]
fn inline_simplification_preserves_mode_cancels_and_rejects_oversized_tables() {
    let mut w = Workspace::default();
    w.open_generator();
    w.generator.request_mode(true);
    finish_jobs(&mut w);
    w.generator.draft = "A & B | A & ~B".into();
    w.generator.request_simplify();
    let request = w.take_generation_request().unwrap();
    let original = w.generator.analysis.clone();
    w.generator.cancel_computation();
    w.receive_generation(request.serial, request.execute(|| false));
    assert_eq!(w.generator.draft, "A & B | A & ~B");
    assert_eq!(w.generator.analysis, original);
    w.generator.request_simplify();
    let request = w.take_generation_request().unwrap();
    w.receive_generation(request.serial, request.execute(|| true));
    assert_eq!(w.generator.draft, "A & B | A & ~B");
    assert!(w.generator.error.is_some());
    w.generator.request_simplify();
    finish_jobs(&mut w);
    assert_eq!(w.generator.draft, "A");
    assert_eq!(w.generator.analysis, original);
    assert!(w.generator.analysis.is_expression_mode());
    assert_eq!(w.generator.analysis.rows(), 0);
    for i in 2..32 {
        w.generator.analysis.add_input(format!("I{i}")).unwrap();
    }
    w.generator.refresh();
    w.generator.draft = "A | B".into();
    let original = w.generator.analysis.clone();
    w.generator.request_simplify();
    finish_jobs(&mut w);
    assert!(
        w.generator
            .error
            .as_ref()
            .unwrap()
            .message
            .contains("12 inputs")
    );
    assert_eq!(w.generator.draft, "A | B");
    assert_eq!(w.generator.analysis, original);
}
