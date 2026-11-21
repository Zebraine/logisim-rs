//! Public field names describe the editable subset of the existing component codecs.
use super::schema::{self, Rule, U32, WIDTH};
use logisim_core::{Component, Facing, Gate, GateSize, Kind, ParityOutput, PinPull, Radix};
use serde_json::{Value, json};

const DIRECTION: Rule = Rule::Enum(|| Facing::ALL.map(|f| f.name()).to_vec());
const SIZES: [GateSize; 3] = [GateSize::Narrow, GateSize::Medium, GateSize::Wide];
const SIZE: Rule = Rule::Enum(|| SIZES.map(|s| s.name()).to_vec());

pub(super) fn fields(kind: Kind) -> Vec<(&'static str, Rule)> {
    if matches!(kind, Kind::Subcircuit(_)) {
        return vec![
            ("label", Rule::Label),
            ("facing", DIRECTION),
            ("font", Rule::Font),
            ("position", Rule::Position),
        ];
    }
    let mut fields = Vec::new();
    if kind.rotatable() {
        fields.push(("facing", DIRECTION));
    }
    match kind {
        Kind::Pin => fields.extend([
            ("bits", WIDTH),
            ("output", Rule::Boolean),
            ("three_state", Rule::Boolean),
            ("pull", Rule::Enum(|| PULL.map(|(_, s)| s).to_vec())),
            ("label_facing", DIRECTION),
        ]),
        Kind::Gate(g) => {
            fields.push(("bits", WIDTH));
            if !g.unary() {
                fields.extend([
                    ("inputs", Rule::Integer(2, Gate::MAX_INPUTS as i64)),
                    ("negation_mask", Rule::Integer(0, u8::MAX as i64)),
                ]);
            }
            if g == Gate::Not {
                fields.push((
                    "size",
                    Rule::Enum(|| {
                        [GateSize::Narrow, GateSize::Wide]
                            .map(|s| s.name())
                            .to_vec()
                    }),
                ));
            } else if g != Gate::Buffer {
                fields.push(("size", SIZE));
            }
            if matches!(g, Gate::Xor | Gate::Xnor) {
                fields.push(("xor_odd", Rule::Boolean));
            }
        }
        Kind::Parity(_) => fields.extend([
            ("bits", WIDTH),
            ("size", SIZE),
            ("inputs", Rule::Integer(2, 32)),
            (
                "output",
                Rule::Enum(|| ParityOutput::ALL.map(|p| p.name()).to_vec()),
            ),
            ("negation_mask", U32),
        ]),
        Kind::Probe => fields.extend([
            ("radix", Rule::Enum(|| RADIX.map(|(_, s)| s).to_vec())),
            ("label_facing", DIRECTION),
        ]),
        Kind::Tunnel => fields.push(("bits", WIDTH)),
        Kind::Splitter => fields.extend([
            ("input_bits", WIDTH),
            ("fanout", WIDTH),
            ("mapping", Rule::Mapping),
        ]),
        _ => {
            let extra = super::builtins::fields(kind);
            if extra.is_empty() {
                return vec![];
            }
            fields.extend(extra.iter().map(|f| (f.name, f.rule)));
        }
    }
    if kind.has_label() {
        if kind != Kind::Text {
            fields.push(("label", Rule::Label));
        }
        fields.push(("font", Rule::Font));
        if kind != Kind::Tunnel {
            fields.push((
                "color",
                Rule::Rgba {
                    opaque: kind != Kind::Text,
                },
            ));
        }
    }
    fields
}

// The schema describes a kind independently of any current instance or partial patch.
// These constraints also depend on sibling properties of the final component.
pub(super) fn property_schema(kind: Kind, fields: &[(&'static str, Rule)]) -> Value {
    let mut schema = schema::object(fields.iter().copied());
    match kind {
        Kind::Counter => {
            schema["counter_max"]["description"] = "Must fit the final bits width.".into()
        }
        Kind::Constant => {
            schema["value"]["description"] =
                "Unsigned document constant. Must fit the final bits width.".into()
        }
        Kind::Gate(g) if !g.unary() => {
            schema["negation_mask"]["description"] = "Only the low inputs bits may be set.".into()
        }
        Kind::Parity(_) => {
            schema["negation_mask"]["description"] = "Only the low inputs bits may be set.".into()
        }
        Kind::FlipFlop(_) => {
            schema["new_layout"]["description"] =
                "A false value requires controls to be legacy.".into()
        }
        _ => {}
    }
    schema
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
        Kind::Subcircuit(id) => {
            data["circuit_id"] = id.into();
            data["circuit_name"] = json!(c.interface.name);
            data["interface_ports"] = super::interface_ports(&c.interface);
        }
        _ => {}
    }
    let writable = fields(c.kind);
    data["property_schema"] = property_schema(c.kind, &writable);
    data["property_types"] = Value::Object(
        writable
            .iter()
            .map(|(key, rule)| ((*key).to_owned(), rule.type_name().into()))
            .collect(),
    );
    data["patchable_properties"] = writable.iter().map(|(key, _)| Value::from(*key)).collect();
}
