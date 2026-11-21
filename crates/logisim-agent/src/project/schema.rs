//! Kind-level field constraints. The codec additionally validates the completed component.
use crate::{Error, Result};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub(super) enum Rule {
    Boolean,
    Integer(i64, i64),
    Enum(fn() -> Vec<&'static str>),
    Label,
    Font,
    FontFamily,
    Rgba { opaque: bool },
    Position,
    Mapping,
    RomRuns,
}

pub(super) const WIDTH: Rule = Rule::Integer(1, 32);
pub(super) const U32: Rule = Rule::Integer(0, u32::MAX as i64);
pub(super) const COORD_LIMIT: i32 = 1_000_000;
const COORD: Rule = Rule::Integer(-(COORD_LIMIT as i64), COORD_LIMIT as i64);
const BYTE: Rule = Rule::Integer(0, 255);
const LABEL_BYTES: usize = 65_536;
const FAMILY_BYTES: usize = 256;
const FONT_FIELDS: [(&str, Rule); 4] = [
    ("family", Rule::FontFamily),
    ("size", Rule::Integer(1, 255)),
    ("bold", Rule::Boolean),
    ("italic", Rule::Boolean),
];

impl Rule {
    pub fn type_name(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Integer(..) => "integer",
            Self::Enum(_) | Self::Label | Self::FontFamily => "string",
            Self::Font => "object",
            Self::Rgba { .. } | Self::Position | Self::Mapping | Self::RomRuns => "array",
        }
    }

    pub fn json(self) -> Value {
        let ty = self.type_name();
        match self {
            Self::Boolean => json!({"type":ty}),
            Self::Integer(min, max) => json!({"type":ty,"minimum":min,"maximum":max}),
            Self::Enum(codes) => json!({"type":ty,"enum":codes()}),
            Self::Label => json!({"type":ty,"maxLength":LABEL_BYTES,"x-maxUtf8Bytes":LABEL_BYTES,
                "pattern":r"^[^\u0000-\u0008\u000B\u000C\u000E-\u001F\uFFFE\uFFFF]*$"}),
            Self::FontFamily => json!({"type":ty,"minLength":1,"maxLength":FAMILY_BYTES,
                "x-maxUtf8Bytes":FAMILY_BYTES,"pattern":r"^(?=[\s\S]*\S)[^\u0000-\u001F\u007F-\u009F\uFFFE\uFFFF]+$"}),
            Self::Font => json!({"type":ty,"additionalProperties":false,
                "properties":object(FONT_FIELDS)}),
            Self::Rgba { opaque } => {
                let mut schema = json!({"type":ty,"minItems":4,"maxItems":4,"items":BYTE.json()});
                if opaque {
                    schema["prefixItems"] =
                        json!([BYTE.json(),BYTE.json(),BYTE.json(),{"type":"integer","const":255}]);
                }
                schema
            }
            Self::Position => json!({"type":ty,"minItems":2,"maxItems":2,"items":COORD.json()}),
            Self::RomRuns => json!({
                "type":ty,"minItems":0,"maxItems":super::memory::MAX_RUNS,
                "items":{"type":"array","minItems":2,"maxItems":2,"items":U32.json(),
                    "prefixItems":[Rule::Integer(0,(1<<24)-1).json(),U32.json()]},
                "x-addressBits":"address_bits","x-wordBits":"bits","x-strictlyIncreasingIndex":0,
                "description":"Each [address, word] starts a run lasting until the next boundary or capacity. Initial words are zero. Addresses must increase and fit address_bits; words must fit bits."
            }),
            Self::Mapping => json!({"type":ty,"minItems":1,"maxItems":32,
                "items":{"type":["integer","null"],"minimum":0,"maximum":31,"nullable":true},
                "description":"One entry per input_bits. A non-null branch index must be less than fanout."}),
        }
    }

    pub fn validate(self, key: &str, value: &Value) -> Result<()> {
        let valid = match self {
            Self::Boolean => value.is_boolean(),
            Self::Integer(min, max) => value.as_i64().is_some_and(|n| (min..=max).contains(&n)),
            Self::Enum(codes) => value
                .as_str()
                .is_some_and(|s| codes().iter().any(|code| code.eq_ignore_ascii_case(s))),
            // XML character validity is checked by the file writer.
            Self::Label => value.as_str().is_some_and(|s| s.len() <= LABEL_BYTES),
            Self::FontFamily => value.as_str().is_some_and(|s| {
                s.len() <= FAMILY_BYTES && !s.trim().is_empty() && !s.chars().any(char::is_control)
            }),
            Self::Font => {
                if let Some(font) = value.as_object() {
                    for (name, v) in font {
                        let (_, rule) =
                            FONT_FIELDS.iter().find(|(k, _)| k == name).ok_or_else(|| {
                                Error::new(
                                    "invalid_property",
                                    format!("Unknown font field {name}."),
                                )
                            })?;
                        rule.validate(name, v)?;
                    }
                    true
                } else {
                    false
                }
            }
            Self::Rgba { opaque } => value.as_array().is_some_and(|a| {
                a.len() == 4
                    && a.iter().all(|v| v.as_u64().is_some_and(|n| n <= 255))
                    && (!opaque || a[3] == 255)
            }),
            Self::Position => value
                .as_array()
                .is_some_and(|a| a.len() == 2 && a.iter().all(|v| COORD.validate(key, v).is_ok())),
            // Individual pairs are decoded against the final dimensions in memory::update.
            Self::RomRuns => value
                .as_array()
                .is_some_and(|v| v.len() <= super::memory::MAX_RUNS),
            Self::Mapping => value.as_array().is_some_and(|a| {
                (1..=32).contains(&a.len())
                    && a.iter()
                        .all(|v| v.is_null() || v.as_u64().is_some_and(|n| n < 32))
            }),
        };
        if valid {
            Ok(())
        } else {
            Err(Error::new(
                "invalid_property",
                format!("Invalid {key}. Expected {}.", self.json()),
            ))
        }
    }
}

pub(super) fn object(fields: impl IntoIterator<Item = (&'static str, Rule)>) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(key, rule)| (key.to_owned(), rule.json()))
            .collect(),
    )
}
