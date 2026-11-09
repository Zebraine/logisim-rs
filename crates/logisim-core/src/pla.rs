//! Editable PLA connection planes. Columns run from most to least significant bit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlaInput {
    #[default]
    Disconnected,
    Inverted,
    Direct,
}
impl PlaInput {
    pub fn next(self) -> Self {
        match self {
            Self::Disconnected => Self::Direct,
            Self::Direct => Self::Inverted,
            Self::Inverted => Self::Disconnected,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pla {
    inputs: u8,
    terms: u8,
    outputs: u8,
    input_plane: Vec<PlaInput>,
    output_plane: Vec<bool>,
    pub select_high: bool,
}
impl Default for Pla {
    fn default() -> Self {
        Self::new(4, 4, 4)
    }
}
impl Pla {
    pub fn new(inputs: u8, terms: u8, outputs: u8) -> Self {
        let (inputs, terms, outputs) = (
            inputs.clamp(1, 32),
            terms.clamp(1, 32),
            outputs.clamp(1, 32),
        );
        Self {
            inputs,
            terms,
            outputs,
            input_plane: vec![PlaInput::Disconnected; usize::from(inputs) * usize::from(terms)],
            output_plane: vec![false; usize::from(outputs) * usize::from(terms)],
            select_high: false,
        }
    }
    pub fn dimensions(&self) -> (u8, u8, u8) {
        (self.inputs, self.terms, self.outputs)
    }
    pub fn input(&self, row: u8, column: u8) -> PlaInput {
        assert!(row < self.terms && column < self.inputs);
        self.input_plane[usize::from(row) * usize::from(self.inputs) + usize::from(column)]
    }
    pub fn output(&self, row: u8, column: u8) -> bool {
        assert!(row < self.terms && column < self.outputs);
        self.output_plane[usize::from(row) * usize::from(self.outputs) + usize::from(column)]
    }
    pub fn set_input(&mut self, row: u8, column: u8, value: PlaInput) {
        assert!(row < self.terms && column < self.inputs);
        self.input_plane[usize::from(row) * usize::from(self.inputs) + usize::from(column)] = value;
    }
    pub fn set_output(&mut self, row: u8, column: u8, connected: bool) {
        assert!(row < self.terms && column < self.outputs);
        self.output_plane[usize::from(row) * usize::from(self.outputs) + usize::from(column)] =
            connected;
    }
    pub fn resize(&mut self, inputs: u8, terms: u8, outputs: u8) {
        let mut next = Self::new(inputs, terms, outputs);
        for row in 0..self.terms.min(next.terms) {
            for col in 0..self.inputs.min(next.inputs) {
                next.set_input(row, col, self.input(row, col));
            }
            for col in 0..self.outputs.min(next.outputs) {
                next.set_output(row, col, self.output(row, col));
            }
        }
        next.select_high = self.select_high;
        *self = next;
    }
    pub fn clear(&mut self) {
        self.input_plane.fill(PlaInput::Disconnected);
        self.output_plane.fill(false);
    }
}

#[cfg(test)]
mod tests;
