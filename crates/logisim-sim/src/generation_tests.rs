//! Verify generated geometry through the real net resolver, not an AST evaluator.
use crate::Simulator;
use logisim_core::{
    Circuit, Kind,
    generation::{Analysis, Cell, Form},
};

fn verify(analysis: &Analysis, circuit: &Circuit) {
    let input_ids: Vec<_> = analysis
        .inputs()
        .iter()
        .map(|name| {
            circuit
                .components
                .iter()
                .find(|c| c.kind == Kind::Pin && !c.output && &c.label == name)
                .unwrap()
                .id
        })
        .collect();
    let output_ids: Vec<_> = analysis
        .outputs()
        .iter()
        .map(|output| {
            circuit
                .components
                .iter()
                .find(|c| c.kind == Kind::Pin && c.output && c.label == output.name())
                .unwrap()
                .id
        })
        .collect();
    let mut sim = Simulator::default();
    sim.synchronize(circuit);
    // Reverse traversal exercises both rising and falling inputs after initial settle.
    for row in (0..analysis.rows()).rev() {
        for (i, id) in input_ids.iter().enumerate() {
            sim.set_input(*id, u32::from(analysis.input_value(row, i).unwrap()));
        }
        sim.advance(1024);
        assert!(
            sim.diagnostics().is_empty(),
            "row {row}: {:?}",
            sim.diagnostics()
        );
        for (output, id) in analysis.outputs().iter().zip(&output_ids) {
            let actual = sim
                .port_value(*id, 0)
                .unwrap()
                .number()
                .expect("generated output must be binary");
            let expected = output.values()[row];
            if expected != Cell::DontCare {
                assert_eq!(
                    Cell::from(actual == 1),
                    expected,
                    "{} row {row}",
                    output.name()
                );
            }
        }
    }
}

#[test]
fn generated_expressions_handle_fanout_crossings_constants_and_uneven_trees() {
    let mut a = Analysis::default();
    for name in ["A", "B", "Cin", "Unused"] {
        a.add_input(name.into()).unwrap();
    }
    for (name, expression) in [
        ("Sum", "A ^ B ^ Cin"),
        ("Carry", "A B | Cin (A ^ B)"),
        ("Tree", "~(~A | (B & (Cin ^ A))) ^ (B | (A & Cin))"),
        ("Direct", "Cin"),
        ("Zero", "0"),
        ("One", "1"),
    ] {
        a.add_output(name.into()).unwrap();
        a.apply_expression(a.outputs().len() - 1, expression)
            .unwrap();
    }
    let c = a.generate().unwrap();
    assert_eq!(c, a.generate().unwrap());
    verify(&a, &c);
}

#[test]
fn generated_tables_and_both_simplification_forms_drive_the_specified_outputs() {
    let mut a = Analysis::default();
    for name in ["A", "B", "C"] {
        a.add_input(name.into()).unwrap();
    }
    a.add_output("Y".into()).unwrap();
    // All binary three-input functions plus deterministic don't-care variants.
    for pattern in 0..256 {
        for row in 0..8 {
            a.set_cell(0, row, Cell::from(pattern & (1 << row) != 0))
                .unwrap();
        }
        if pattern % 3 == 0 {
            a.set_cell(0, pattern % 8, Cell::DontCare).unwrap();
        }
        let c = a.generate().unwrap();
        verify(&a, &c);
        let mut pos = a.clone();
        let text = a
            .minimized(0, Form::ProductOfSums)
            .unwrap()
            .text(a.inputs());
        pos.apply_expression(0, &text).unwrap();
        verify(&a, &pos.generate().unwrap());
    }
}
