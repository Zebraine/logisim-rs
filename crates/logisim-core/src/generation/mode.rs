use super::{Analysis, Cell, Error, MAX_TABLE_INPUTS, Minimizer, Result, work::Work};

impl Minimizer {
    /// Resolve don't-cares once, then discard dense tables. Failure leaves the
    /// original specification unchanged; existing expressions keep their structure.
    pub fn expressions(
        &mut self,
        analysis: &Analysis,
        cancelled: impl Fn() -> bool,
    ) -> Result<Analysis> {
        let mut work = Work::new(&cancelled);
        work.step()?;
        if analysis.expression_mode {
            return Ok(analysis.clone());
        }
        let mut result = analysis.clone();
        for (i, output) in result.outputs.iter_mut().enumerate() {
            output.expression = Some(self.expression_with(analysis, i, &mut work)?);
            output.values = Vec::new();
        }
        result.expression_mode = true;
        Ok(result)
    }
}
impl Analysis {
    /// Materialize a bounded complete table on demand, preserving supplied ASTs.
    pub fn truth_table(&self, cancelled: impl Fn() -> bool) -> Result<Self> {
        let mut work = Work::new(&cancelled);
        work.step()?;
        if self.inputs.len() > MAX_TABLE_INPUTS {
            return Err(Error::new(
                "Complete truth tables support at most 12 inputs. The expressions are preserved.",
            ));
        }
        if !self.expression_mode {
            return Ok(self.clone());
        }
        let mut result = self.clone();
        for output in &mut result.outputs {
            let expression = output
                .expression
                .as_ref()
                .expect("expression mode has an AST for every output");
            let cost = expression.node_count();
            let mut values = Vec::with_capacity(1 << self.inputs.len());
            for row in 0..1 << self.inputs.len() {
                work.spend(cost).map_err(|e| {
                    if e.message == Work::exhausted().message {
                        Error::new("Truth table generation exceeded its work limit. The expressions are preserved.")
                    } else { e }
                })?;
                values.push(Cell::from(expression.evaluate(row, self.inputs.len())));
            }
            output.values = values;
        }
        result.expression_mode = false;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Gate, Kind,
        generation::{Form, MAX_INPUTS, MAX_OUTPUTS},
    };

    #[test]
    fn thirty_two_inputs_never_materialize_a_table_and_preserve_xor_structure() {
        let mut a = Analysis::expressions();
        for i in 0..MAX_INPUTS {
            a.add_input(format!("I{i}")).unwrap();
        }
        for i in 0..MAX_OUTPUTS {
            a.add_output(format!("O{i}")).unwrap();
        }
        a.apply_expression(0, &a.inputs().join(" ^ ")).unwrap();
        assert_eq!(a.rows(), 0);
        assert!(a.outputs().iter().all(|o| o.values().is_empty()));
        let before = a.clone();
        assert!(a.add_input("Extra".into()).is_err());
        assert!(a.add_output("Extra".into()).is_err());
        assert!(a.apply_expression(0, "I32").is_err());
        assert!(a.set_cell(0, 0, Cell::One).is_err());
        assert!(a.truth_table(|| false).is_err());
        assert!(a.minimized(0, Form::SumOfProducts).is_err());
        assert_eq!(a, before);
        let circuit = a.generate().unwrap();
        assert_eq!(
            circuit
                .components
                .iter()
                .filter(|c| c.kind == Kind::Gate(Gate::Xor))
                .count(),
            31
        );
        assert!(
            circuit
                .components
                .iter()
                .all(|c| !matches!(c.kind, Kind::Gate(Gate::And | Gate::Or)))
        );
    }

    #[test]
    fn modes_roundtrip_atomically_and_variable_edits_keep_bindings() {
        let mut a = Analysis::expressions();
        for name in ["Unused", "A", "B"] {
            a.add_input(name.into()).unwrap();
        }
        a.add_output("Y".into()).unwrap();
        a.apply_expression(0, "A ^ B").unwrap();
        let before = a.clone();
        assert!(a.remove(true, 1).is_err());
        assert_eq!(a, before);
        a.remove(true, 0).unwrap();
        a.rename(true, 0, "Renamed".into()).unwrap();
        a.swap(true, 0, 1).unwrap();
        assert_eq!(a.expression(0).unwrap().text(a.inputs()), "Renamed ^ B");
        assert!(a.truth_table(|| true).is_err());
        let table = a.truth_table(|| false).unwrap();
        assert_eq!(table.rows(), 4);
        assert_eq!(
            table.outputs()[0].values(),
            [Cell::Zero, Cell::One, Cell::One, Cell::Zero]
        );
        assert_eq!(
            Minimizer::default().expressions(&table, || false).unwrap(),
            a
        );
        let mut table = table;
        table.set_cell(0, 0, Cell::DontCare).unwrap();
        let expression = Minimizer::default().expressions(&table, || false).unwrap();
        let restored = expression.truth_table(|| false).unwrap();
        for (old, new) in table.outputs()[0]
            .values()
            .iter()
            .zip(restored.outputs()[0].values())
        {
            if *old != Cell::DontCare {
                assert_eq!(old, new);
            }
        }
    }

    #[test]
    fn table_materialization_has_a_shared_budget_and_preserves_all_expressions_on_failure() {
        let mut a = Analysis::expressions();
        for i in 0..12 {
            a.add_input(format!("I{i}")).unwrap();
        }
        let mut text = "I0".to_owned();
        for _ in 0..7 {
            text = format!("({text}) ^ ({text})");
        }
        for i in 0..32 {
            a.add_output(format!("O{i}")).unwrap();
            a.apply_expression(i, &text).unwrap();
        }
        let before = a.clone();
        assert!(a.truth_table(|| false).is_err());
        assert_eq!(a, before);
        assert!(a.outputs().iter().all(|o| o.values().is_empty()));
    }
}
