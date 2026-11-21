//! Logical, term-major programming planes. Core owns the cell layout and resize policy.
use super::{
    edit::{choice, decode},
    schema::{Rule, WIDTH},
};
use crate::{Error, Result};
use logisim_core::{Pla, PlaInput};
use serde_json::{Map, Value, json};

const INPUT_CODES: [(PlaInput, &str); 3] = [
    (PlaInput::Disconnected, "disconnected"),
    (PlaInput::Direct, "direct"),
    (PlaInput::Inverted, "inverted"),
];

#[derive(Clone, Copy)]
pub(super) enum Plane {
    Input,
    Output,
}

impl Plane {
    fn columns(self) -> &'static str {
        match self {
            Self::Input => "inputs",
            Self::Output => "outputs",
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::Input => "input_plane",
            Self::Output => "output_plane",
        }
    }
    pub fn schema(self) -> Value {
        let cell = match self {
            Self::Input => json!({"type":"string", "enum": INPUT_CODES.map(|(_,code)|code)}),
            Self::Output => Rule::Boolean.json(),
        };
        json!({"type":"array","minItems":1,"maxItems":32,"x-rows":"terms",
            "items":{"type":"array","minItems":1,"maxItems":32,"items":cell},"x-columns":self.columns()})
    }
    fn write(self, pla: &mut Pla, v: &Value) -> Result<()> {
        let (inputs, terms, outputs) = pla.dimensions();
        let columns = match self {
            Self::Input => inputs,
            Self::Output => outputs,
        };
        let key = self.key();
        let invalid_shape = || {
            Error::new(
                "invalid_property",
                format!("{key} requires {terms} rows of {columns} cells."),
            )
        };
        let rows = v
            .as_array()
            .filter(|rows| rows.len() == usize::from(terms))
            .ok_or_else(invalid_shape)?;
        for (row, cells) in rows.iter().enumerate() {
            let cells = cells
                .as_array()
                .filter(|cells| cells.len() == usize::from(columns))
                .ok_or_else(invalid_shape)?;
            for (col, value) in cells.iter().enumerate() {
                match self {
                    Self::Input => {
                        pla.set_input(row as u8, col as u8, choice(key, value, INPUT_CODES)?)
                    }
                    Self::Output => pla.set_output(row as u8, col as u8, decode(key, value)?),
                }
            }
        }
        Ok(())
    }
}

pub(super) const FIELDS: [(&str, Rule); 5] = [
    ("inputs", WIDTH),
    ("terms", WIDTH),
    ("outputs", WIDTH),
    ("input_plane", Rule::PlaPlane(Plane::Input)),
    ("output_plane", Rule::PlaPlane(Plane::Output)),
];

pub(super) fn describe(pla: &Pla, data: &mut Value) {
    let (inputs, terms, outputs) = pla.dimensions();
    data["inputs"] = inputs.into();
    data["terms"] = terms.into();
    data["outputs"] = outputs.into();
    data["input_plane"] = (0..terms)
        .map(|row| {
            (0..inputs)
                .map(|col| {
                    let cell = pla.input(row, col);
                    let (_, code) = INPUT_CODES
                        .iter()
                        .find(|(k, _)| *k == cell)
                        .expect("complete PLA input codes");
                    Value::from(*code)
                })
                .collect::<Value>()
        })
        .collect();
    data["output_plane"] = (0..terms)
        .map(|row| {
            (0..outputs)
                .map(|col| Value::from(pla.output(row, col)))
                .collect::<Value>()
        })
        .collect();
}

pub(super) fn update(pla: &mut Pla, props: &Map<String, Value>) -> Result<()> {
    let old = pla.dimensions();
    let dimension = |key, current| props.get(key).map_or(Ok(current), |v| decode::<u8>(key, v));
    let (inputs, terms, outputs) = (
        dimension("inputs", old.0)?,
        dimension("terms", old.1)?,
        dimension("outputs", old.2)?,
    );
    if (inputs, terms, outputs) != old {
        pla.resize(inputs, terms, outputs);
    }
    for plane in [Plane::Input, Plane::Output] {
        if let Some(value) = props.get(plane.key()) {
            plane.write(pla, value)?;
        }
    }
    Ok(())
}
