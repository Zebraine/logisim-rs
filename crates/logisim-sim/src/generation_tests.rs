//! Verify generated geometry through the real net resolver, not an AST evaluator.
use crate::Simulator;
use logisim_core::{
    Circuit, Gate, Kind,
    generation::{Analysis, Cell, Form, GateBasis},
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
    for compiled in [false, true] {
        let mut sim = Simulator::default();
        sim.synchronize(circuit);
        // Reverse traversal exercises both rising and falling inputs after initial settle.
        for row in (0..analysis.rows()).rev() {
            for (i, id) in input_ids.iter().enumerate() {
                sim.set_input(*id, u32::from(analysis.input_value(row, i).unwrap()));
            }
            if compiled {
                sim.settle_compiled().unwrap();
            } else {
                sim.advance(1024);
            }
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
    for basis in [GateBasis::Standard, GateBasis::Nand] {
        let c = a.generate_with_basis(basis).unwrap();
        assert_eq!(c, a.generate_with_basis(basis).unwrap());
        verify(&a, &c);
    }
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
        let mut pos = a.clone();
        let text = a
            .minimized(0, Form::ProductOfSums)
            .unwrap()
            .text(a.inputs());
        pos.apply_expression(0, &text).unwrap();
        for basis in [GateBasis::Standard, GateBasis::Nand] {
            verify(&a, &a.generate_with_basis(basis).unwrap());
            verify(&a, &pos.generate_with_basis(basis).unwrap());
        }
    }
}

#[test]
fn nested_nand_xor_reuses_subtrees_and_preserves_all_eight_input_rows() {
    let mut a = Analysis::default();
    for name in ["A", "B", "C", "D", "E", "F", "G", "H"] {
        a.add_input(name.into()).unwrap();
    }
    a.add_output("Parity".into()).unwrap();
    a.apply_expression(0, "A ^ B ^ C ^ D ^ E ^ F ^ G ^ H")
        .unwrap();
    let circuit = a.generate_with_basis(GateBasis::Nand).unwrap();
    assert_eq!(
        circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::Gate(Gate::Nand))
            .count(),
        4 * 7
    );
    verify(&a, &circuit);
    a.add_output("Inverted".into()).unwrap();
    a.apply_expression(1, "~((A ^ B) ^ (C ^ (D ^ (E ^ F)))) | ~(G & ~H)")
        .unwrap();
    verify(&a, &a.generate_with_basis(GateBasis::Nand).unwrap());
}

#[test]
fn expression_only_thirty_two_input_xor_simulates_with_standard_and_nand_gates() {
    let mut a = Analysis::expressions();
    for i in 0..32 {
        a.add_input(format!("I{i}")).unwrap();
    }
    a.add_output("Y".into()).unwrap();
    a.apply_expression(0, &a.inputs().join(" ^ ")).unwrap();
    for basis in [GateBasis::Standard, GateBasis::Nand] {
        let circuit = a.generate_with_basis(basis).unwrap();
        let inputs: Vec<_> = a
            .inputs()
            .iter()
            .map(|name| {
                circuit
                    .components
                    .iter()
                    .find(|c| &c.label == name)
                    .unwrap()
                    .id
            })
            .collect();
        let output = circuit
            .components
            .iter()
            .find(|c| c.label == "Y")
            .unwrap()
            .id;
        let mut sim = Simulator::default();
        sim.synchronize(&circuit);
        let mut patterns = vec![0u32, u32::MAX, 0x12345678, 0xAAAAAAAA, 0x55555555];
        for bit in 0..32 {
            patterns.extend([1 << bit, !(1 << bit)]);
        }
        for row in patterns {
            for (i, id) in inputs.iter().enumerate() {
                sim.set_input(*id, (row >> (31 - i)) & 1);
            }
            sim.settle_compiled().unwrap();
            assert!(sim.diagnostics().is_empty());
            assert_eq!(
                sim.port_value(output, 0).unwrap().number(),
                Some(row.count_ones() % 2)
            );
        }
    }
}

#[test]
fn balanced_and_generation_preserves_grouping_fanout_and_inversions() {
    let mut a = Analysis::default();
    for name in ["A", "B", "C", "D", "E", "F", "G", "H"] {
        a.add_input(name.into()).unwrap();
    }
    for (name, text) in [
        ("Left", "A & B & C & D & E & F & G & H"),
        ("Right", "A & (B & (C & (D & (E & (F & (G & H))))))"),
        ("Odd", "A & B & C & D & E"),
        ("Nested", "A & (B | C) & ~(D & E & F & G & H)"),
        ("Xor", "(A & B & C & D) ^ (E & F & G & H)"),
        ("Repeated", "A & B & A & ~C & 1 & D & E & F"),
    ] {
        a.add_output(name.into()).unwrap();
        a.apply_expression(a.outputs().len() - 1, text).unwrap();
    }
    for basis in [GateBasis::Standard, GateBasis::Nand] {
        verify(&a, &a.generate_with_basis(basis).unwrap());
    }
}
