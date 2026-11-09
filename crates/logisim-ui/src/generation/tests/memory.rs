use super::support::finish_jobs;
use crate::generation::*;
use logisim_core::{Gate, Kind, generation::Cell};

fn build(w: &mut Workspace, analysis: Analysis, name: &str) {
    w.generator.load_analysis(analysis);
    w.generator.name = name.into();
    w.open_generator();
    w.generator.request_build();
    finish_jobs(w);
    assert!(!w.generator.open, "{:?}", w.generator.error);
}

#[test]
fn generated_expressions_restore_all_outputs_per_circuit_without_a_worker_or_edit() {
    let mut w = Workspace::default();
    let mut original = Generator::default().analysis;
    original.apply_expression(0, "A ^ B").unwrap();
    original.add_output("Z".into()).unwrap();
    original.apply_expression(1, "(A | B) & (~A | ~B)").unwrap();
    original.swap(true, 0, 1).unwrap();
    original.swap(false, 0, 1).unwrap();
    build(&mut w, original.clone(), "first");
    let first = w.active_circuit();
    let mut other = Generator::default().analysis;
    other.apply_expression(0, "A | B").unwrap();
    build(&mut w, other.clone(), "second");
    let second = w.active_circuit();
    let document = w.project().clone();
    for (id, expected, tables) in [
        (first, &original, false),
        (second, &other, true),
        (first, &original, true),
    ] {
        w.editor.switch_circuit(id);
        w.simulation_mut().ticks = 17;
        w.set_truth_table_generation(tables);
        w.request_analysis();
        assert!(w.take_analysis_request().is_none());
        assert_eq!(&w.generator.analysis, expected);
        assert_eq!(w.generator.tab, Tab::Expression);
        assert!(!w.generator.computing);
        assert_eq!(w.generator.target.as_ref().unwrap().id(), id);
        assert!(w.generator.replace_target);
        for output in 0..expected.outputs().len() {
            w.generator.output = output;
            w.generator.refresh();
            assert_eq!(
                w.generator.draft,
                expected.expression(output).unwrap().text(expected.inputs())
            );
        }
        assert_eq!(w.project(), &document);
        assert_eq!(w.simulation_mut().ticks, 17);
        assert!(w.take_file_actions().is_empty());
        assert!(w.take_simulation_actions().is_empty());
        w.generator.open = false;
    }
    // Analysis did not introduce an undo entry: undo still removes the second build.
    w.editor.undo();
    assert_eq!(w.project().circuits.len(), 2);
    assert!(w.project().circuit(second).is_none());
}

#[test]
fn edits_invalidate_memory_undo_restores_it_and_old_analysis_results_are_rejected() {
    let mut w = Workspace::default();
    let mut original = Generator::default().analysis;
    original.apply_expression(0, "A ^ B").unwrap();
    build(&mut w, original.clone(), "xor");
    let before = w.editor.snapshot();
    w.editor
        .session
        .circuit
        .components
        .iter_mut()
        .find(|c| c.kind == Kind::Gate(Gate::Xor))
        .unwrap()
        .kind = Kind::Gate(Gate::Or);
    w.editor.record(before);
    w.request_analysis();
    let (old, _) = w.take_analysis_request().unwrap();
    w.editor.undo();
    w.request_analysis();
    assert!(w.take_analysis_request().is_none());
    assert_eq!(w.generator.analysis, original);
    w.receive_analysis(old, Ok(Analysis::default()));
    assert_eq!(w.generator.analysis, original);
    w.editor.redo();
    w.request_analysis();
    assert!(w.take_analysis_request().is_some());
    w.editor.undo();
    w.editor
        .session
        .circuit
        .components
        .iter_mut()
        .find(|c| c.kind == Kind::Pin)
        .unwrap()
        .label = "renamed".into();
    w.request_analysis();
    assert!(w.take_analysis_request().is_some());
}

#[test]
fn only_successfully_installed_builds_replace_the_last_generated_expressions() {
    let mut w = Workspace::default();
    let mut original = Generator::default().analysis;
    original.apply_expression(0, "A ^ B").unwrap();
    build(&mut w, original.clone(), "xor");
    w.request_analysis();
    w.generator.analysis.apply_expression(0, "A & B").unwrap();
    w.generator.refresh();
    w.generator.request_build();
    let stale = w.take_generation_request().unwrap();
    w.generator.cancel_computation();
    w.receive_generation(stale.serial, stale.execute(|| false));
    w.request_analysis();
    assert_eq!(w.generator.analysis, original);
    w.generator.analysis.apply_expression(0, "A & B").unwrap();
    w.generator.refresh();
    w.generator.request_build();
    let cancelled = w.take_generation_request().unwrap();
    w.receive_generation(cancelled.serial, cancelled.execute(|| true));
    assert!(w.generator.error.is_some());
    w.request_analysis();
    assert_eq!(w.generator.analysis, original);
    w.generator.analysis.apply_expression(0, "A & B").unwrap();
    w.generator.refresh();
    w.generator.replace_target = false;
    w.generator.name = "main".into(); // installation fails after generation succeeds
    w.generator.request_build();
    finish_jobs(&mut w);
    assert!(w.generator.error.is_some());
    w.request_analysis();
    assert_eq!(w.generator.analysis, original);
    w.generator.analysis.apply_expression(0, "A | B").unwrap();
    w.generator.refresh();
    w.generator.request_build();
    finish_jobs(&mut w);
    let rebuilt = w.project().clone();
    assert_eq!(rebuilt.circuits.len(), 2);
    w.request_analysis();
    assert!(w.take_analysis_request().is_none());
    assert_eq!(w.generator.applied, "A | B");
    w.editor.undo();
    w.request_analysis();
    assert!(w.take_analysis_request().is_some()); // only the latest successful build is retained
    w.editor.redo();
    w.request_analysis();
    assert!(w.take_analysis_request().is_none());
    assert_eq!(w.generator.applied, "A | B");
    assert_eq!(w.project(), &rebuilt);
}

#[test]
fn expression_memory_supports_32_inputs_but_does_not_cross_project_or_session_boundaries() {
    let mut w = Workspace::default();
    let mut analysis = Analysis::expressions();
    for i in 0..32 {
        analysis.add_input(format!("I{i}")).unwrap();
    }
    analysis.add_output("Y".into()).unwrap();
    analysis
        .apply_expression(0, &analysis.inputs().join(" ^ "))
        .unwrap();
    build(&mut w, analysis.clone(), "wide");
    let id = w.active_circuit();
    w.request_analysis();
    assert!(w.take_analysis_request().is_none());
    assert_eq!(w.generator.analysis, analysis);
    w.document.partial = true;
    w.request_analysis();
    assert!(w.analysis.open());
    assert!(w.take_analysis_request().is_none());
    w.document.partial = false;
    let project = w.project().clone();
    let mut reopened = Workspace::default();
    reopened.replace_project(project.clone());
    reopened.editor.switch_circuit(id);
    reopened.request_analysis();
    assert!(reopened.take_analysis_request().is_some());
    w.replace_project(project);
    w.editor.switch_circuit(id);
    w.request_analysis();
    assert!(w.take_analysis_request().is_some());
}

#[test]
fn table_builds_remember_the_resolved_function_of_every_output() {
    let mut w = Workspace::default();
    let mut table = Generator::default().analysis.truth_table(|| false).unwrap();
    table.add_output("Z".into()).unwrap();
    table.set_cell(0, 3, Cell::One).unwrap();
    table.set_cell(0, 0, Cell::DontCare).unwrap();
    table.set_cell(1, 1, Cell::One).unwrap();
    table.set_cell(1, 2, Cell::Zero).unwrap();
    let expected = logisim_core::generation::Minimizer::default()
        .expressions(&table, || false)
        .unwrap();
    build(&mut w, table, "from_table");
    w.request_analysis();
    assert!(w.take_analysis_request().is_none());
    assert_eq!(w.generator.analysis, expected);
    assert!(!w.generator.computing);
}
