//! ROM images are change points, including transitions back to zero inside the address space.
use super::edit::decode;
use crate::{Error, Result};
use logisim_core::{Component, MemoryContents};
use serde_json::{Value, json};

pub(super) const MAX_RUNS: usize = 65_536;

fn boundaries(c: &Component) -> impl Iterator<Item = (u32, u32)> + '_ {
    let capacity = c.memory_capacity();
    c.memory
        .contents
        .boundaries()
        .take_while(move |&(a, _)| a < capacity)
}

fn check_budget(c: &Component) -> Result<()> {
    if boundaries(c).nth(MAX_RUNS).is_some() {
        return Err(Error::new(
            "too_large",
            format!("ROM {} exceeds {MAX_RUNS} run boundaries.", c.id),
        ));
    }
    Ok(())
}

pub(super) fn snapshot(c: &Component) -> Result<Value> {
    check_budget(c)?;
    Ok(boundaries(c)
        .map(|(address, word)| json!([address, word]))
        .collect())
}

pub(super) fn update(
    c: &mut Component,
    replacement: Option<&Value>,
    old_dimensions: (u8, u8),
) -> Result<()> {
    if let Some(value) = replacement {
        let capacity = c.memory_capacity();
        let mask = u32::MAX >> (32 - c.bits);
        let runs = value.as_array().expect("validated ROM run list");
        let mut contents = MemoryContents::default();
        let (mut start, mut word) = (0, 0);
        for (index, run) in runs.iter().enumerate() {
            let [address, next] = decode::<[u32; 2]>("contents", run)?;
            if (index > 0 && address <= start) || address >= capacity || next > mask {
                return Err(Error::new(
                    "invalid_property",
                    format!(
                        "Invalid contents[{index}]. Addresses must increase and fit address_bits; words must fit bits."
                    ),
                ));
            }
            if word != 0 {
                contents.fill(start, address, word);
            }
            (start, word) = (address, next);
        }
        if word != 0 {
            contents.fill(start, capacity, word);
        }
        c.memory.contents = contents;
    } else if old_dimensions != (c.memory.address_bits, c.bits) {
        c.memory.contents.resize(c.memory.address_bits, c.bits);
        check_budget(c)?;
    }
    Ok(())
}
