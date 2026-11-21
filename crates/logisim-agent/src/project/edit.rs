//! Apply named property edits to a core component before handing it back to the circuit.
use super::properties::{PULL, RADIX, fields};
use crate::{Error, Result};
use logisim_core::{Component, Facing, GateSize, Kind, ParityOutput};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

pub(super) fn require_supported(kind: Kind) -> Result<()> {
    if fields(kind).is_empty() {
        return Err(unsupported(kind.name()));
    }
    Ok(())
}
pub(super) fn unsupported(name: &str) -> Error {
    Error::new(
        "unsupported_component",
        format!("Project Patch cannot edit {name}."),
    )
}

fn invalid(message: impl ToString) -> Error {
    Error::new("invalid_property", message)
}
pub(super) fn decode<T: DeserializeOwned>(key: &str, v: &Value) -> Result<T> {
    T::deserialize(v).map_err(|e| invalid(format!("{key}: {e}")))
}
pub(super) fn choice<T: Copy>(
    key: &str,
    v: &Value,
    choices: impl IntoIterator<Item = (T, &'static str)>,
) -> Result<T> {
    let text = v
        .as_str()
        .ok_or_else(|| invalid(format!("{key} must be a string.")))?;
    choices
        .into_iter()
        .find(|(_, s)| s.eq_ignore_ascii_case(text))
        .map(|(value, _)| value)
        .ok_or_else(|| invalid(format!("Invalid {key}: {text}.")))
}
fn facing(key: &str, v: &Value) -> Result<Facing> {
    choice(key, v, Facing::ALL.map(|f| (f, f.name())))
}

pub(super) fn apply(c: &mut Component, properties: &Map<String, Value>) -> Result<()> {
    require_supported(c.kind)?;
    let writable = fields(c.kind);
    let old_memory = (c.memory.address_bits, c.bits);
    let old_split = (c.bits, c.splitter.fanout);
    for (k, v) in properties {
        let key = k.as_str();
        let Some((_, rule)) = writable.iter().find(|(name, _)| *name == key) else {
            return Err(invalid(format!(
                "{} has no writable {key} property.",
                c.kind.name()
            )));
        };
        rule.validate(key, v)?;
        if c.kind == Kind::PlaRom && super::pla::FIELDS.iter().any(|(name, _)| *name == key) {
            continue;
        }
        if let Some(field) = super::builtins::fields(c.kind)
            .iter()
            .find(|f| f.name == key)
        {
            (field.write)(c, v)?;
            continue;
        }
        match key {
            "position" => c.position = super::patch::point(decode(key, v)?)?,
            "facing" => c.facing = facing(key, v)?,
            "bits" | "input_bits" => c.bits = decode(key, v)?,
            "label" => c.label = decode(key, v)?,
            "font" => {
                let font = v
                    .as_object()
                    .ok_or_else(|| invalid("font must be an object."))?;
                for (key, v) in font {
                    match key.as_str() {
                        "family" => c.font.family = decode(key, v)?,
                        "size" => c.font.size = decode(key, v)?,
                        "bold" => c.font.bold = decode(key, v)?,
                        "italic" => c.font.italic = decode(key, v)?,
                        _ => return Err(invalid(format!("Unknown font field {key}."))),
                    }
                }
            }
            "color" => {
                let [r, g, b, a]: [u8; 4] = decode(key, v)?;
                c.color = [r, g, b];
                c.color_alpha = a;
            }
            "label_facing" => c.label_facing = facing(key, v)?,
            "output" if c.kind == Kind::Pin => c.output = decode(key, v)?,
            "output" => c.parity.output = choice(key, v, ParityOutput::ALL.map(|p| (p, p.name())))?,
            "three_state" => c.pin.three_state = decode(key, v)?,
            "pull" => c.pin.pull = choice(key, v, PULL)?,
            "radix" => c.radix = choice(key, v, RADIX)?,
            "inputs" if matches!(c.kind, Kind::Gate(_)) => c.gate_options.inputs = decode(key, v)?,
            "inputs" => c.parity.inputs = decode(key, v)?,
            "size" => {
                let size = choice(
                    key,
                    v,
                    [GateSize::Narrow, GateSize::Medium, GateSize::Wide].map(|s| (s, s.name())),
                )?;
                if matches!(c.kind, Kind::Parity(_)) {
                    c.parity.size = size;
                } else {
                    c.gate_options.size = size;
                }
            }
            "xor_odd" => c.gate_options.xor_odd = decode(key, v)?,
            "fanout" => c.splitter.fanout = decode(key, v)?,
            // Both depend on final dimensions, independent of JSON key order.
            "negation_mask" | "mapping" | "contents" => {}
            _ => unreachable!("writable property without an editor"),
        }
    }
    if matches!(c.kind, Kind::Gate(_) | Kind::Parity(_)) {
        let active = u32::MAX >> (32 - c.input_count());
        let mask = if let Some(v) = properties.get("negation_mask") {
            let mask: u32 = decode("negation_mask", v)?;
            if mask & !active != 0 {
                return Err(invalid("negation_mask refers to an inactive input."));
            }
            mask
        } else if matches!(c.kind, Kind::Parity(_)) {
            c.parity.negated & active
        } else {
            u32::from(c.gate_options.negated) & active
        };
        if matches!(c.kind, Kind::Parity(_)) {
            c.parity.negated = mask;
        } else {
            c.gate_options.negated = mask as u8;
        }
    }
    if c.kind == Kind::PlaRom {
        super::pla::update(&mut c.pla, properties)?;
    }
    if c.kind == Kind::Rom {
        super::memory::update(c, properties.get("contents"), old_memory)?;
    }
    if c.kind == Kind::Splitter {
        if let Some(v) = properties.get("mapping") {
            let map: Vec<Option<u8>> = decode("mapping", v)?;
            if map.len() != usize::from(c.bits)
                || map.iter().flatten().any(|&n| n >= c.splitter.fanout)
            {
                return Err(invalid(
                    "Provide one null or valid zero-based branch per incoming bit.",
                ));
            }
            c.splitter.mapping.fill(None);
            c.splitter.mapping[..map.len()].copy_from_slice(&map);
        } else if old_split != (c.bits, c.splitter.fanout) {
            c.splitter.distribute(c.bits, c.splitter.descending);
        }
    }
    Ok(())
}
