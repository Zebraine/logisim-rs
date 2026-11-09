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
