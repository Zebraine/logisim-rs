use crate::generation::*;
use logisim_core::Kind;

use super::support::*;

#[test]
fn localized_dialog_applies_expressions_and_blocks_stale_generation() {
    use crate::{
        Language, i18n,
        tests::support::widgets::{frame, frame_at_size},
    };
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
                "Use two-input gates only",
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
        assert!(!w.generator.two_input_only);
        click(&ctx, &mut w, &label("Use two-input gates only"));
        assert!(w.generator.two_input_only);
        assert!(!w.generator.nand_only);
        click(&ctx, &mut w, &label("Use NAND gates only"));
        assert!(w.generator.nand_only);
        assert_eq!(w.generator.analysis, before);
        w.generator.draft = "A ^ B".into();
        click(&ctx, &mut w, &label("Build Circuit"));
        assert_eq!(w.project().circuits.len(), 1);
        click(&ctx, &mut w, &label("Apply"));
        assert!(w.generator.analysis.is_expression_mode());
        assert_eq!(w.generator.analysis.rows(), 0);
        assert_eq!(w.generator.applied, "A ^ B");
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
fn optional_truth_table_controls_preserve_the_table_and_unapplied_draft() {
    use crate::{Language, i18n, tests::support::widgets::frame_at_size};
    for language in Language::ALL {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut w = Workspace::default();
        w.set_language(language);
        w.open_generator();
        assert!(!w.truth_table_generation());
        assert!(w.generator.analysis.is_expression_mode());
        let label = |key: &str| i18n::with_language(language, || tr(key).to_owned());
        for size in [egui::vec2(1200.0, 820.0), egui::vec2(720.0, 480.0)] {
            let labels = frame_at_size(&ctx, &mut w, vec![], size);
            for key in ["Truth Table", "Create Truth Table", "Expression mode"] {
                assert!(!labels.iter().any(|(text, _)| *text == label(key)));
            }
        }
        w.set_truth_table_generation(true);
        click(&ctx, &mut w, &label("Truth Table"));
        click(&ctx, &mut w, &label("Create Truth Table"));
        assert_eq!(w.generator.analysis.rows(), 4);
        w.generator.draft = "A &".into();
        let analysis = w.generator.analysis.clone();
        let applied = w.generator.applied.clone();
        let serial = w.generator.serial;
        w.set_truth_table_generation(false);
        assert_eq!(w.generator.tab, Tab::Expression);
        let labels = frame_at_size(&ctx, &mut w, vec![], egui::vec2(720.0, 480.0));
        for key in ["Truth Table", "Create Truth Table", "Expression mode"] {
            assert!(!labels.iter().any(|(text, _)| *text == label(key)));
        }
        w.set_truth_table_generation(true);
        assert_eq!(w.generator.analysis, analysis);
        assert_eq!(w.generator.draft, "A &");
        assert_eq!(w.generator.applied, applied);
        assert_eq!(w.generator.serial, serial);
        assert!(!w.editor.can_undo());
        click(&ctx, &mut w, &label("Revert"));
        click(&ctx, &mut w, &label("Truth Table"));
        assert_eq!(w.generator.tab, Tab::Table);
        assert_eq!(w.generator.analysis, analysis);
    }
}

#[test]
fn analysis_and_inflight_table_conversion_use_the_current_visibility_preference() {
    let mut analysis = Generator::default().analysis;
    analysis.apply_expression(0, "A ^ B").unwrap();
    let analysis = analysis.truth_table(|| false).unwrap();
    for enabled in [false, true] {
        let mut w = Workspace::default();
        w.set_truth_table_generation(enabled);
        w.replace_circuit(analysis.generate().unwrap());
        w.request_analysis();
        let (serial, _) = w.take_analysis_request().unwrap();
        w.receive_analysis(serial, Ok(analysis.clone()));
        assert_eq!(w.generator.analysis, analysis);
        assert_eq!(
            w.generator.tab,
            if enabled { Tab::Table } else { Tab::Expression }
        );
    }
    let mut w = Workspace::default();
    w.set_truth_table_generation(true);
    w.open_generator();
    w.generator.request_mode(false);
    let request = w.take_generation_request().unwrap();
    w.set_truth_table_generation(false);
    assert!(w.generation_pending(request.serial));
    w.receive_generation(request.serial, request.execute(|| false));
    assert_eq!(w.generator.analysis.rows(), 4);
    assert_eq!(w.generator.tab, Tab::Expression);
    assert!(!w.editor.can_undo());
}

#[test]
fn localized_expression_mode_builds_without_tables_and_materializes_on_request() {
    use crate::{Language, i18n, tests::support::widgets::frame_at_size};
    for language in Language::ALL {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut w = Workspace::default();
        w.set_language(language);
        w.set_truth_table_generation(true);
        w.open_generator();
        let label = |key: &str| i18n::with_language(language, || tr(key).to_owned());
        click(&ctx, &mut w, &label("Expression"));
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
            5
        );
    }
}

#[test]
fn variable_add_controls_follow_table_expression_and_output_limits() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut w = Workspace::default();
    w.set_truth_table_generation(true);
    w.generator.analysis = w.generator.analysis.truth_table(|| false).unwrap();
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
        crate::tests::support::widgets::frame_at_size(
            &ctx,
            &mut w,
            vec![],
            egui::vec2(720.0, 480.0),
        );
    }
    let labels = crate::tests::support::widgets::frame_at_size(
        &ctx,
        &mut w,
        vec![],
        egui::vec2(720.0, 480.0),
    );
    assert!(
        labels
            .iter()
            .any(|(label, rect)| label == "Add" && rect.bottom() <= 480.0)
    );
}
