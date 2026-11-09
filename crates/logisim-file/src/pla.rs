use crate::{Error, Result};
use logisim_core::{Component, Facing, Kind, Pla, PlaInput};
use std::collections::BTreeMap;

pub(crate) fn allowed(key: &str) -> bool {
    matches!(
        key,
        "inputs" | "and" | "outputs" | "Contents" | "Select" | "label" | "labelfont" | "labelcolor"
    )
}
pub(crate) fn read(c: &mut Component, a: &BTreeMap<String, String>) -> Result<()> {
    if c.kind != Kind::PlaRom {
        return Ok(());
    }
    let inputs = crate::attributes::number(a, "inputs", 4, 1..=32)?;
    let terms = crate::attributes::number(a, "and", 4, 1..=32)?;
    let outputs = crate::attributes::number(a, "outputs", 4, 1..=32)?;
    c.pla = Pla::new(inputs, terms, outputs);
    c.pla.select_high = match a.get("Select").map(String::as_str) {
        None | Some("low") => false,
        Some("high") => true,
        _ => return Err(Error("Unsupported PLA select polarity".into())),
    };
    if let Some(text) = a.get("Contents") {
        let invalid = || Error("Invalid PLA connection table or dimensions".into());
        let input_len = usize::from(inputs) * usize::from(terms);
        let total = input_len + usize::from(outputs) * usize::from(terms);
        let mut cells = Vec::with_capacity(total);
        for token in text.split_whitespace() {
            let (value, count) = token.split_once('*').unwrap_or((token, "1"));
            if !value.bytes().all(|v| v.is_ascii_digit())
                || !count.bytes().all(|v| v.is_ascii_digit())
            {
                return Err(invalid());
            }
            let value = value
                .parse::<u8>()
                .ok()
                .filter(|v| *v <= 2)
                .ok_or_else(invalid)?;
            let count = count
                .parse::<usize>()
                .ok()
                .filter(|n| *n > 0 && *n <= total - cells.len())
                .ok_or_else(invalid)?;
            cells.extend(std::iter::repeat_n(value, count));
        }
        if cells.len() != total {
            return Err(invalid());
        }
        for row in 0..terms {
            for col in 0..inputs {
                c.pla.set_input(
                    row,
                    col,
                    match cells[usize::from(row) * usize::from(inputs) + usize::from(col)] {
                        0 => PlaInput::Disconnected,
                        1 => PlaInput::Inverted,
                        2 => PlaInput::Direct,
                        _ => unreachable!(),
                    },
                );
            }
            for col in 0..outputs {
                let value =
                    cells[input_len + usize::from(row) * usize::from(outputs) + usize::from(col)];
                if value > 1 {
                    return Err(invalid());
                }
                c.pla.set_output(row, col, value == 1);
            }
        }
    }
    Ok(())
}
pub(crate) fn write(out: &mut String, c: &Component) -> Result<()> {
    if c.kind != Kind::PlaRom {
        return Ok(());
    }
    if c.facing != Facing::East {
        return Err(Error("Unsupported PLA orientation".into()));
    }
    use crate::xml::attribute as a;
    let (inputs, terms, outputs) = c.pla.dimensions();
    for (name, value) in [("inputs", inputs), ("and", terms), ("outputs", outputs)] {
        a(out, name, value)?;
    }
    if c.pla.select_high {
        a(out, "Select", "high")?;
    }
    let input: Vec<_> = (0..terms)
        .flat_map(|row| {
            (0..inputs).map(move |col| match c.pla.input(row, col) {
                PlaInput::Disconnected => 0,
                PlaInput::Inverted => 1,
                PlaInput::Direct => 2,
            })
        })
        .collect();
    let output: Vec<_> = (0..terms)
        .flat_map(|row| (0..outputs).map(move |col| u8::from(c.pla.output(row, col))))
        .collect();
    if input.iter().chain(&output).any(|v| *v != 0) {
        let mut text = String::new();
        for plane in [&input, &output] {
            let mut start = 0;
            while start < plane.len() {
                let value = plane[start];
                let count = plane[start..].iter().take_while(|v| **v == value).count();
                if count >= 3 {
                    text.push_str(&format!("{value}*{count} "));
                } else {
                    for _ in 0..count {
                        text.push_str(&format!("{value} "));
                    }
                }
                start += count;
            }
        }
        a(out, "Contents", text)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
