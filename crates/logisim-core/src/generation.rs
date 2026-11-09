//! Bounded, binary combinational specifications and editable circuit synthesis.
//! This model has no simulator, GUI or filesystem dependency. A don't-care is a
//! synthesis freedom, not a floating simulation value.
mod expression;
mod layout;
mod minimize;
#[cfg(test)]
mod tests;

use crate::Circuit;
pub use expression::Expression;

pub const MAX_INPUTS: usize = 8;
pub const MAX_OUTPUTS: usize = 8;
pub const MAX_EXPRESSION_BYTES: usize = 16_384;
pub const MAX_GENERATED_ITEMS: usize = crate::MAX_CIRCUIT_ITEMS;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cell {
    Zero,
    One,
    #[default]
    DontCare,
}
impl Cell {
    pub fn next(self) -> Self {
        match self {
            Self::Zero => Self::One,
            Self::One => Self::DontCare,
            Self::DontCare => Self::Zero,
        }
    }
    pub fn text(self) -> &'static str {
        match self {
            Self::Zero => "0",
            Self::One => "1",
            Self::DontCare => "x",
        }
    }
}
impl From<bool> for Cell {
    fn from(value: bool) -> Self {
        if value { Self::One } else { Self::Zero }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Form {
    #[default]
    SumOfProducts,
    ProductOfSums,
}

/// Logic gates used by synthesis. Pins and constants are retained in either basis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GateBasis {
    #[default]
    Standard,
    Nand,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    /// Application-owned message, separate from user names for localization.
    pub message: &'static str,
    pub detail: String,
}
impl Error {
    fn new(message: &'static str) -> Self {
        Self {
            message,
            detail: String::new(),
        }
    }
    fn detail(message: &'static str, detail: impl ToString) -> Self {
        Self {
            message,
            detail: detail.to_string(),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message)?;
        if !self.detail.is_empty() {
            write!(f, " {}", self.detail)?;
        }
        Ok(())
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    name: String,
    values: Vec<Cell>,
    expression: Option<Expression>,
}
impl Output {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn values(&self) -> &[Cell] {
        &self.values
    }
}

/// Input order determines the table, with the first input as the most significant bit.
/// Mutations preserve the shape of every output column and are atomic on failure.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Analysis {
    inputs: Vec<String>,
    outputs: Vec<Output>,
}
impl Analysis {
    pub fn inputs(&self) -> &[String] {
        &self.inputs
    }
    pub fn outputs(&self) -> &[Output] {
        &self.outputs
    }
    pub fn rows(&self) -> usize {
        1 << self.inputs.len()
    }
    pub fn input_value(&self, row: usize, input: usize) -> Option<bool> {
        (row < self.rows() && input < self.inputs.len())
            .then(|| row & (1 << (self.inputs.len() - input - 1)) != 0)
    }
    fn validate_name(&self, name: &str, except: Option<(bool, usize)>) -> Result<()> {
        let mut chars = name.chars();
        if name.chars().count() > 32
            || !chars.next().is_some_and(char::is_alphabetic)
            || !chars.all(|c| c.is_alphanumeric() || c == '_')
        {
            return Err(Error::new(
                "Use a letter followed by letters, digits or underscores (up to 32 characters).",
            ));
        }
        if self
            .inputs
            .iter()
            .enumerate()
            .any(|(i, n)| n == name && except != Some((true, i)))
            || self
                .outputs
                .iter()
                .enumerate()
                .any(|(i, o)| o.name == name && except != Some((false, i)))
        {
            return Err(Error::detail("Duplicate variable name:", name));
        }
        Ok(())
    }
    pub fn add_input(&mut self, name: String) -> Result<()> {
        self.validate_name(&name, None)?;
        if self.inputs.len() == MAX_INPUTS {
            return Err(Error::new("At most 8 inputs are supported."));
        }
        self.inputs.push(name);
        for output in &mut self.outputs {
            output.values = output.values.iter().flat_map(|v| [*v, *v]).collect();
        }
        Ok(())
    }
    pub fn add_output(&mut self, name: String) -> Result<()> {
        self.validate_name(&name, None)?;
        if self.outputs.len() == MAX_OUTPUTS {
            return Err(Error::new("At most 8 outputs are supported."));
        }
        self.outputs.push(Output {
            name,
            values: vec![Cell::DontCare; self.rows()],
            expression: None,
        });
        Ok(())
    }
    pub fn rename(&mut self, input: bool, index: usize, name: String) -> Result<()> {
        self.check_index(input, index)?;
        self.validate_name(&name, Some((input, index)))?;
        if input {
            self.inputs[index] = name;
        } else {
            self.outputs[index].name = name;
        }
        Ok(())
    }
    pub fn remove(&mut self, input: bool, index: usize) -> Result<()> {
        self.check_index(input, index)?;
        if input {
            self.inputs.remove(index);
            let bit = 1 << (self.inputs.len() - index);
            let rows = self.rows();
            for output in &mut self.outputs {
                output.values = (0..rows)
                    .map(|row| {
                        let old = ((row & !(bit - 1)) << 1) | (row & (bit - 1));
                        let a = output.values[old];
                        let b = output.values[old | bit];
                        if a == b { a } else { Cell::DontCare }
                    })
                    .collect();
                output.expression = None;
            }
        } else {
            self.outputs.remove(index);
        }
        Ok(())
    }
    pub fn swap(&mut self, input: bool, a: usize, b: usize) -> Result<()> {
        self.check_index(input, a)?;
        self.check_index(input, b)?;
        if input {
            self.inputs.swap(a, b);
            let x = 1 << (self.inputs.len() - a - 1);
            let y = 1 << (self.inputs.len() - b - 1);
            for output in &mut self.outputs {
                output.values = (0..output.values.len())
                    .map(|row| {
                        let old = if (row & x != 0) == (row & y != 0) {
                            row
                        } else {
                            row ^ x ^ y
                        };
                        output.values[old]
                    })
                    .collect();
                if let Some(expression) = &mut output.expression {
                    expression.swap_inputs(a, b);
                }
            }
        } else {
            self.outputs.swap(a, b);
        }
        Ok(())
    }
    fn check_index(&self, input: bool, index: usize) -> Result<()> {
        if index
            < if input {
                self.inputs.len()
            } else {
                self.outputs.len()
            }
        {
            Ok(())
        } else {
            Err(Error::new("Variable index is out of range."))
        }
    }
    pub fn set_cell(&mut self, output: usize, row: usize, cell: Cell) -> Result<()> {
        self.check_index(false, output)?;
        let column = &mut self.outputs[output];
        let value = column
            .values
            .get_mut(row)
            .ok_or_else(|| Error::new("Table row is out of range."))?;
        if *value != cell {
            *value = cell;
            column.expression = None;
        }
        Ok(())
    }
    pub fn apply_expression(&mut self, output: usize, text: &str) -> Result<()> {
        self.check_index(false, output)?;
        let expression = Expression::parse(text, &self.inputs)?;
        let values = (0..self.rows())
            .map(|row| Cell::from(expression.evaluate(row, self.inputs.len())))
            .collect();
        self.outputs[output].values = values;
        self.outputs[output].expression = Some(expression);
        Ok(())
    }
    pub fn minimized(&self, output: usize, form: Form) -> Result<Expression> {
        self.check_index(false, output)?;
        Ok(minimize::minimize(
            &self.outputs[output].values,
            self.inputs.len(),
            form,
        ))
    }
    pub fn expression(&self, output: usize) -> Result<Expression> {
        self.check_index(false, output)?;
        if let Some(expression) = &self.outputs[output].expression {
            Ok(expression.clone())
        } else {
            self.minimized(output, Form::SumOfProducts)
        }
    }
    pub fn generate(&self) -> Result<Circuit> {
        self.generate_with_basis(GateBasis::Standard)
    }
    /// Generate an independent document without changing the table or expressions.
    /// Equivalence is binary and settled; gate delays and four-state behavior can differ.
    pub fn generate_with_basis(&self, basis: GateBasis) -> Result<Circuit> {
        if self.outputs.is_empty() {
            return Err(Error::new(
                "Add at least one output before building a circuit.",
            ));
        }
        layout::generate(self, basis)
    }
}
