//! Bounded session cache, independent of variable names and document state.
use super::{
    Analysis, Cell, Circuit, Expression, Form, GenerationOptions, Result, layout, work::Work,
};
use std::{collections::VecDeque, sync::Arc};

struct Entry {
    inputs: usize,
    values: Vec<Cell>,
    form: Form,
    result: Result<Expression>,
}
#[derive(Clone, Default)]
pub struct Minimizer {
    entries: VecDeque<Arc<Entry>>,
}
impl Minimizer {
    pub fn cached(
        &self,
        analysis: &Analysis,
        output: usize,
        form: Form,
    ) -> Option<Result<Expression>> {
        if analysis.is_expression_mode() {
            return None;
        }
        let column = analysis.outputs.get(output)?;
        self.entries
            .iter()
            .rev()
            .find(|e| {
                e.inputs == analysis.inputs.len() && e.form == form && e.values == column.values
            })
            .map(|e| e.result.clone())
    }
    fn insert(
        &mut self,
        analysis: &Analysis,
        output: usize,
        form: Form,
        result: Result<Expression>,
    ) {
        if self.entries.len() == 32 {
            self.entries.pop_front();
        }
        self.entries.push_back(Arc::new(Entry {
            inputs: analysis.inputs.len(),
            values: analysis.outputs[output].values.clone(),
            form,
            result,
        }));
    }
    pub fn minimized(
        &mut self,
        analysis: &Analysis,
        output: usize,
        form: Form,
        cancelled: impl Fn() -> bool,
    ) -> Result<Expression> {
        let mut work = Work::new(&cancelled);
        work.step()?;
        analysis.check_index(false, output)?;
        if let Some(result) = self.cached(analysis, output, form) {
            return result;
        }
        let result = analysis.minimize_with(output, form, &mut work);
        if result
            .as_ref()
            .err()
            .is_none_or(|e| e.message == Work::exhausted().message)
        {
            self.insert(analysis, output, form, result.clone());
        }
        result
    }
    pub(super) fn expression_with(
        &mut self,
        analysis: &Analysis,
        output: usize,
        work: &mut Work<'_>,
    ) -> Result<Expression> {
        work.step()?;
        analysis.check_index(false, output)?;
        if let Some(expression) = analysis.supplied_expression(output) {
            return Ok(expression.clone());
        }
        if let Some(result) = self.cached(analysis, output, Form::SumOfProducts) {
            return result;
        }
        let expression = analysis.minimize_with(output, Form::SumOfProducts, work)?;
        self.insert(
            analysis,
            output,
            Form::SumOfProducts,
            Ok(expression.clone()),
        );
        Ok(expression)
    }
    pub fn generate(
        &mut self,
        analysis: &Analysis,
        options: impl Into<GenerationOptions>,
        cancelled: impl Fn() -> bool,
    ) -> Result<Circuit> {
        self.generate_with_expressions(analysis, options, cancelled)
            .map(|(circuit, _)| circuit)
    }
    /// Return the exact expressions used by this build, before layout balancing
    /// and gate conversion. Resolve tables only once under the same work budget.
    pub fn generate_with_expressions(
        &mut self,
        analysis: &Analysis,
        options: impl Into<GenerationOptions>,
        cancelled: impl Fn() -> bool,
    ) -> Result<(Circuit, Analysis)> {
        if analysis.outputs.is_empty() {
            return Err(super::Error::new(
                "Add at least one output before building a circuit.",
            ));
        }
        layout::generate(analysis, options.into(), &mut Work::new(&cancelled), self)
    }
}
