//! Public field names describe the editable subset of the existing component codecs.
use logisim_core::{Component, Gate, Kind, PinPull, Radix};
use serde_json::{Value, json};

pub(super) fn fields(kind: Kind) -> Vec<&'static str> {
    let mut keys = Vec::new();
    if kind.rotatable() {
        keys.push("facing");
    }
    match kind {
        Kind::Pin => keys.extend(["bits", "output", "three_state", "pull", "label_facing"]),
        Kind::Gate(g) => {
            keys.push("bits");
            if !g.unary() {
                keys.extend(["inputs", "negation_mask"]);
            }
            if g != Gate::Buffer {
                keys.push("size");
            }
            if matches!(g, Gate::Xor | Gate::Xnor) {
                keys.push("xor_odd");
            }
        }
        Kind::Parity(_) => keys.extend(["bits", "size", "inputs", "output", "negation_mask"]),
        Kind::Probe => keys.extend(["radix", "label_facing"]),
        Kind::Tunnel => keys.push("bits"),
        Kind::Splitter => keys.extend(["input_bits", "fanout", "mapping"]),
        _ => {
            let extra = super::builtins::fields(kind);
            if extra.is_empty() {
                return vec![];
            }
            keys.extend(extra.iter().map(|f| f.name));
        }
    }
    if kind.has_label() {
        if kind != Kind::Text {
            keys.push("label");
        }
        keys.push("font");
        if kind != Kind::Tunnel {
            keys.push("color");
        }
    }
    keys
}

pub(super) const PULL: [(PinPull, &str); 3] = [
    (PinPull::None, "none"),
    (PinPull::Up, "up"),
    (PinPull::Down, "down"),
];
pub(super) const RADIX: [(Radix, &str); 5] = [
    (Radix::Binary, "binary"),
    (Radix::Octal, "octal"),
    (Radix::Signed, "signed"),
    (Radix::Unsigned, "unsigned"),
    (Radix::Hex, "hex"),
];
fn code<T: PartialEq>(table: &[(T, &'static str)], value: T) -> &'static str {
    table
        .iter()
        .find(|(item, _)| *item == value)
        .expect("complete field vocabulary")
        .1
}

pub(super) fn describe(c: &Component, data: &mut Value) {
    for field in super::builtins::fields(c.kind) {
        data[field.name] = (field.read)(c);
    }
    match c.kind {
        Kind::Pin => {
            data["output"] = c.output.into();
            data["three_state"] = c.pin.three_state.into();
            data["pull"] = code(&PULL, c.pin.pull).into();
            data["label_facing"] = c.label_facing.name().into();
        }
        Kind::Gate(_) => {
            data["inputs"] = c.input_count().into();
            data["size"] = c.gate_options.size.name().into();
            data["negation_mask"] = c.gate_options.negated.into();
            data["xor_odd"] = c.gate_options.xor_odd.into();
        }
        Kind::Parity(_) => {
            data["inputs"] = c.parity.inputs.into();
            data["size"] = c.parity.size.name().into();
            data["output"] = c.parity.output.name().into();
            data["negation_mask"] = c.parity.negated.into();
        }
        Kind::Probe => {
            data["radix"] = code(&RADIX, c.radix).into();
            data["label_facing"] = c.label_facing.name().into();
        }
        Kind::Splitter => {
            data["input_bits"] = c.bits.into();
            data["fanout"] = c.splitter.fanout.into();
            data["mapping"] = json!(c.splitter.mapping[..usize::from(c.bits)]);
        }
        Kind::Constant => data["constant"] = c.constant.into(),
        Kind::Subcircuit(id) => data["circuit_id"] = id.into(),
        _ => {}
    }
    let writable = fields(c.kind);
    let mut types = serde_json::Map::new();
    for &key in &writable {
        let hint = match &data[key] {
            Value::Bool(_) => "boolean",
            Value::Number(_) => "integer",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
            Value::Null => unreachable!("missing snapshot value for {key}"),
        };
        types.insert(key.into(), hint.into());
    }
    data["property_types"] = types.into();
    data["patchable_properties"] = json!(writable);
}
