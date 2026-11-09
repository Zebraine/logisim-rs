//! Display evaluation and instance-local persistence measured from the reference runtime.
use crate::{Bit, Simulator, Value};
use logisim_core::{Circuit, Component, Id, Kind, MatrixInput};
use std::collections::BTreeMap;

pub(crate) struct MatrixState {
    layout: (u8, u8, MatrixInput),
    until: Vec<u64>,
}
fn layout(c: &Component) -> (u8, u8, MatrixInput) {
    (
        c.io.matrix_columns.clamp(1, 32),
        c.io.matrix_rows.clamp(1, 32),
        c.io.matrix_input,
    )
}
impl Simulator {
    pub(crate) fn synchronize_displays(&mut self, circuit: &Circuit) {
        let mut previous = std::mem::take(&mut self.matrices);
        self.matrices = circuit
            .components
            .iter()
            .filter(|c| c.kind == Kind::DotMatrix)
            .map(|c| {
                let layout = layout(c);
                let state = previous
                    .remove(&c.id)
                    .filter(|s| s.layout == layout)
                    .unwrap_or_else(|| MatrixState {
                        layout,
                        until: vec![0; usize::from(layout.0) * usize::from(layout.1)],
                    });
                (c.id, state)
            })
            .collect::<BTreeMap<_, _>>();
    }
    pub(crate) fn reset_displays(&mut self) {
        self.display_ticks = 0;
        for state in self.matrices.values_mut() {
            state.until.fill(0);
        }
    }
    pub(crate) fn update_displays(&mut self, changed: Option<&crate::work::WorkSet>) {
        let samples: Vec<_> = self
            .plan
            .matrices
            .iter()
            .filter(|entry| entry.affected(changed))
            .map(|entry| {
                let c = &self.circuit.components[entry.component];
                (
                    c.id,
                    c.io.matrix_persistence,
                    matrix(c, &entry.sample(&self.values)),
                )
            })
            .collect();
        for (id, persistence, bits) in samples {
            let state = self.matrices.get_mut(&id).expect("compiled matrix state");
            for (until, bit) in state.until.iter_mut().zip(bits) {
                if persistence == 0 {
                    *until = 0;
                } else if bit == Bit::One {
                    *until = self.display_ticks.saturating_add(u64::from(persistence));
                }
            }
        }
    }
    fn display_ports(&self, c: &Component) -> Vec<Value> {
        (0..c.ports().len())
            .map(|i| self.port_value(c.id, i).expect("compiled display port"))
            .collect()
    }
    pub fn rgb_color(&self, id: Id) -> Option<[u8; 3]> {
        let c = self
            .circuit
            .component(id)
            .filter(|c| c.kind == Kind::RgbLed)?;
        Some(rgb(c, &self.display_ports(c)))
    }
    /// Row-major cells, from the top-left; Unknown and Error both render red.
    pub fn matrix_cells(&self, id: Id) -> Option<Vec<Bit>> {
        let c = self
            .circuit
            .component(id)
            .filter(|c| c.kind == Kind::DotMatrix)?;
        let mut cells = matrix(c, &self.display_ports(c));
        if c.io.matrix_persistence > 0
            && let Some(state) = self.matrices.get(&id)
        {
            for (cell, until) in cells.iter_mut().zip(&state.until) {
                if *cell == Bit::Zero && self.display_ticks < *until {
                    *cell = Bit::One;
                }
            }
        }
        Some(cells)
    }
}
fn rgb(c: &Component, ports: &[Value]) -> [u8; 3] {
    std::array::from_fn(|i| {
        ports[i].number().map_or(0, |n| {
            if c.io.rgb_multibit {
                n as u8
            } else if n == 1 {
                255
            } else {
                0
            }
        })
    })
}
fn matrix(c: &Component, ports: &[Value]) -> Vec<Bit> {
    let (cols, rows, mode) = layout(c);
    (0..rows)
        .flat_map(|r| {
            (0..cols).map(move |col| match mode {
                MatrixInput::Columns => ports[usize::from(col)].bit(rows - 1 - r),
                MatrixInput::Rows => ports[usize::from(r)].bit(cols - 1 - col),
                MatrixInput::Select => match ports[1].bit(rows - 1 - r) {
                    Bit::Zero => Bit::Zero,
                    Bit::One => ports[0].bit(col),
                    bit => bit,
                },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
