use super::*;

fn analysis(inputs: &[&str], outputs: &[&str]) -> Analysis {
    let mut result = Analysis::default();
    for name in inputs {
        result.add_input((*name).into()).unwrap();
    }
    for name in outputs {
        result.add_output((*name).into()).unwrap();
    }
    result
}

#[test]
fn nand_basis_uses_only_tied_or_binary_nand_gates_and_keeps_the_specification() {
    use crate::{Gate, Kind};
    let mut a = analysis(&["A", "B", "C"], &["Y"]);
    for (text, gates) in [
        ("~A", 1),
        ("A & B", 2),
        ("A | B", 3),
        ("~(A & B)", 1),
        ("A & B & C", 4),
        ("~(A | B)", 4),
        ("~~A", 0),
        ("A ^ B", 4),
        ("~(A ^ B)", 5),
        ("A", 0),
        ("0", 0),
        ("~0", 0),
    ] {
        a.apply_expression(0, text).unwrap();
        let before = a.clone();
        let circuit = a.generate_with_basis(GateBasis::Nand).unwrap();
        assert_eq!(a, before);
        assert_eq!(
            circuit
                .components
                .iter()
                .filter(|c| matches!(c.kind, Kind::Gate(_)))
                .count(),
            gates,
            "{text}"
        );
        for component in &circuit.components {
            assert!(matches!(
                component.kind,
                Kind::Gate(Gate::Nand) | Kind::Pin | Kind::Constant
            ));
            if component.kind == Kind::Gate(Gate::Nand) {
                assert_eq!(component.input_count(), 2);
                assert_eq!(component.gate_options.negated, 0);
            }
        }
        assert!(circuit.dangling_ends().is_empty(), "{text}");
        assert_eq!(
            a.generate().unwrap(),
            a.generate_with_basis(GateBasis::Standard).unwrap()
        );
    }
}

#[test]
fn nand_expansion_obeys_the_final_geometry_budget_without_changing_the_model() {
    let mut a = analysis(&["A"], &["Y"]);
    let mut expression = "A".to_owned();
    for _ in 0..8 {
        expression = format!("({expression}) | ({expression})");
    }
    a.apply_expression(0, &expression).unwrap();
    let before = a.clone();
    let standard = a.generate().unwrap();
    assert!(a.generate_with_basis(GateBasis::Nand).is_err());
    assert_eq!(a, before);
    assert_eq!(a.generate().unwrap(), standard);
}

#[test]
fn observed_expression_syntax_precedence_and_errors_are_atomic() {
    let mut a = analysis(&["A", "B"], &["Y"]);
    for (text, expected) in [
        ("A ^ B", [false, true, true, false]),
        ("A + B ^ A & B", [false, true, true, true]),
        ("(A + B) ^ (A & B)", [false, true, true, false]),
        ("~A", [true, true, false, false]),
        ("!A", [true, true, false, false]),
        ("A'", [true, true, false, false]),
        ("A B", [false, false, false, true]),
        ("A & (B ^ 1)", [false, false, true, false]),
        ("0", [false; 4]),
        ("1", [true; 4]),
    ] {
        a.apply_expression(0, text).unwrap();
        assert_eq!(a.outputs()[0].values(), expected.map(Cell::from), "{text}");
        let expression = a.expression(0).unwrap();
        let formatted = Expression::parse(&expression.text(a.inputs()), a.inputs()).unwrap();
        for (row, expected) in expected.into_iter().enumerate() {
            assert_eq!(formatted.evaluate(row, 2), expected);
        }
    }
    for text in ["C", "A &", "A * B", "(A", "A)", "", "A ^ ^ B", "2"] {
        let before = a.clone();
        assert!(a.apply_expression(0, text).is_err(), "{text}");
        assert_eq!(a, before);
    }
    for text in [
        format!("{}A{}", "(".repeat(10000), ")".repeat(10000)),
        "~".repeat(200) + "A",
        vec!["A"; 1000].join(" & "),
    ] {
        assert!(a.apply_expression(0, &text).is_err());
    }
}

#[test]
fn every_three_input_ternary_table_has_a_valid_sop_and_pos_cover() {
    let mut a = analysis(&["A", "B", "C"], &["Y"]);
    for mut pattern in 0..3usize.pow(8) {
        for row in 0..8 {
            a.set_cell(0, row, [Cell::Zero, Cell::One, Cell::DontCare][pattern % 3])
                .unwrap();
            pattern /= 3;
        }
        for form in [Form::SumOfProducts, Form::ProductOfSums] {
            let expression = a.minimized(0, form).unwrap();
            for (row, expected) in a.outputs()[0].values().iter().enumerate() {
                if *expected != Cell::DontCare {
                    assert_eq!(Cell::from(expression.evaluate(row, 3)), *expected);
                }
            }
        }
    }
}

#[test]
fn variables_preserve_function_and_columns_through_editing() {
    let mut a = analysis(&["A", "B"], &["Y", "Z"]);
    a.apply_expression(0, "A & ~B").unwrap();
    a.apply_expression(1, "B").unwrap();
    a.swap(true, 0, 1).unwrap();
    assert_eq!(
        a.outputs()[0].values(),
        [Cell::Zero, Cell::One, Cell::Zero, Cell::Zero]
    );
    assert_eq!(a.expression(0).unwrap().text(a.inputs()), "A & ~B");
    a.rename(true, 1, "数据".into()).unwrap();
    assert_eq!(a.expression(0).unwrap().text(a.inputs()), "数据 & ~B");
    a.add_input("C".into()).unwrap();
    assert_eq!(
        a.outputs()[0].values()[..4],
        [Cell::Zero, Cell::Zero, Cell::One, Cell::One]
    );
    a.remove(true, 2).unwrap();
    a.swap(false, 0, 1).unwrap();
    assert_eq!(a.outputs()[0].name(), "Z");
    a.remove(true, 1).unwrap();
    assert_eq!(a.outputs()[0].values(), [Cell::Zero, Cell::One]);
    assert_eq!(a.outputs()[1].values(), [Cell::DontCare, Cell::Zero]);
    let before = a.clone();
    for name in ["B", "Z", "1bad", "", "space name"] {
        assert!(a.add_input(name.into()).is_err());
    }
    assert!(a.swap(true, 0, 100).is_err());
    assert!(a.set_cell(0, 10, Cell::One).is_err());
    assert_eq!(a, before);
}

#[test]
fn limits_constants_and_dont_cares_are_explicit() {
    let mut a = Analysis::default();
    assert!(a.generate().is_err());
    a.add_output("Y".into()).unwrap();
    for text in ["1", "0"] {
        a.apply_expression(0, text).unwrap();
        assert!(a.generate().is_ok());
    }
    for i in 0..MAX_INPUTS {
        a.add_input(format!("I{i}")).unwrap();
    }
    let before = a.clone();
    assert!(a.add_input("tooMany".into()).is_err());
    assert_eq!(a, before);
    for i in 1..MAX_OUTPUTS {
        a.add_output(format!("O{i}")).unwrap();
    }
    assert!(a.add_output("tooMany".into()).is_err());
    for row in 0..a.rows() {
        a.set_cell(0, row, Cell::from(row.count_ones() % 2 == 1))
            .unwrap();
    }
    assert!(a.generate().is_err());
    // A don't-care can eliminate a literal, without being turned into a float.
    let mut a = analysis(&["A", "B"], &["Y"]);
    for (row, value) in [Cell::Zero, Cell::DontCare, Cell::One, Cell::One]
        .into_iter()
        .enumerate()
    {
        a.set_cell(0, row, value).unwrap();
    }
    assert_eq!(a.expression(0).unwrap().text(a.inputs()), "A");
}

#[test]
fn bounded_simplification_and_cancellation_preserve_all_cells() {
    let mut a = Analysis::default();
    for i in 0..12 {
        a.add_input(format!("I{i}")).unwrap();
    }
    a.add_output("Y".into()).unwrap();
    a.apply_expression(0, &a.inputs().join(" ^ ")).unwrap();
    let before = a.clone();
    let calls = std::cell::Cell::new(0);
    let error = a
        .minimized_cancellable(0, Form::SumOfProducts, || {
            calls.set(calls.get() + 1);
            calls.get() == 3
        })
        .unwrap_err();
    assert_eq!(error.message, "Computation cancelled.");
    assert_eq!(calls.get(), 3);
    let optimized = a.minimized(0, Form::SumOfProducts).unwrap();
    for row in 0..a.rows() {
        assert_eq!(optimized.evaluate(row, 12), row.count_ones() % 2 == 1);
    }
    let mut hard = a.clone();
    let mut rng = 12345u32;
    for row in 0..hard.rows() {
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        hard.set_cell(0, row, Cell::from(rng >> 31 != 0)).unwrap();
    }
    let original = hard.clone();
    assert!(hard.minimized(0, Form::SumOfProducts).is_err());
    assert_eq!(hard, original);
    assert_eq!(a, before);
    assert!(
        a.generate_cancellable(GateBasis::Standard, || true)
            .is_err()
    );
    assert!(a.generate().is_ok());
}

#[test]
fn reduced_inputs_constants_and_cached_columns_keep_exact_semantics() {
    let mut a = Analysis::default();
    for i in 0..12 {
        a.add_input(format!("I{i}")).unwrap();
    }
    a.add_output("Y".into()).unwrap();
    let mut cache = Minimizer::default();
    for (case, text) in [(0, "1"), (1, "I3"), (2, "I3 ^ I9")] {
        for row in 0..a.rows() {
            let v = match case {
                0 => true,
                1 => row & (1 << 8) != 0,
                _ => (row & (1 << 8) != 0) ^ (row & (1 << 2) != 0),
            };
            a.set_cell(0, row, Cell::from(v)).unwrap();
        }
        let expected = Expression::parse(text, a.inputs()).unwrap();
        for form in [Form::SumOfProducts, Form::ProductOfSums] {
            let result = cache.minimized(&a, 0, form, || false).unwrap();
            for row in 0..a.rows() {
                assert_eq!(result.evaluate(row, 12), expected.evaluate(row, 12));
            }
        }
    }
    assert!(cache.cached(&a, 0, Form::SumOfProducts).is_some());
    a.rename(true, 3, "Renamed".into()).unwrap();
    assert!(
        cache
            .cached(&a, 0, Form::SumOfProducts)
            .unwrap()
            .unwrap()
            .text(a.inputs())
            .contains("Renamed")
    );
    a.set_cell(0, 0, Cell::One).unwrap();
    assert!(cache.cached(&a, 0, Form::SumOfProducts).is_none());
    a.set_cell(0, 0, Cell::Zero).unwrap();
    assert!(cache.cached(&a, 0, Form::SumOfProducts).is_some());
    assert!(
        cache
            .minimized(&a, 0, Form::SumOfProducts, || true)
            .is_err()
    );
    a.swap(true, 3, 0).unwrap();
    assert!(cache.cached(&a, 0, Form::SumOfProducts).is_none());
}
