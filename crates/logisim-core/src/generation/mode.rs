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
mod tests;
