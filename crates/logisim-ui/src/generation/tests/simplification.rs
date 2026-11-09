use crate::generation::*;

use super::support::*;

#[test]
fn cached_forms_survive_output_switches_rename_and_restore_but_not_table_changes() {
    use logisim_core::generation::Cell;
    let mut w = Workspace::default();
    w.generator.analysis = Analysis::default();
    w.generator.analysis.add_input("A".into()).unwrap();
    w.generator.analysis.add_input("B".into()).unwrap();
    w.generator.analysis.add_output("Y".into()).unwrap();
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
fn localized_inline_simplification_edits_the_draft_and_requires_apply() {
    use crate::{Language, i18n, tests::support::widgets::frame};
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
