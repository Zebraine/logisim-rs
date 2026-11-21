//! Candidate wire representations for version one. The plugin ABI's JSON tag
//! is local to that ABI. A binary file uses ordinary externally tagged enums.

use super::*;
use logisim_core::plugin::api;
use serde::{Deserializer, Serializer};

macro_rules! wire_enum {
    ($name:ident { $($variant:ident $( { $($field:ident: $ty:ty),+ } )?),+ }) => {
        #[derive(Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        enum $name { $($variant $( { $($field: $ty),+ } )?),+ }
        impl From<api::$name> for $name {
            fn from(value: api::$name) -> Self {
                match value { $(api::$name::$variant $( { $($field),+ } )? =>
                    Self::$variant $( { $($field),+ } )?),+ }
            }
        }
        impl From<$name> for api::$name {
            fn from(value: $name) -> Self {
                match value { $($name::$variant $( { $($field),+ } )? =>
                    Self::$variant $( { $($field),+ } )?),+ }
            }
        }
    }
}

wire_enum!(Shape {
    Rect { min: [i32; 2], max: [i32; 2], fill: [u8; 4], stroke: [u8; 4] },
    Line { a: [i32; 2], b: [i32; 2], color: [u8; 4] },
    Ellipse { min: [i32; 2], max: [i32; 2], fill: [u8; 4], stroke: [u8; 4] },
    Text { at: [i32; 2], text: String, size: u8, color: [u8; 4] }
});
wire_enum!(PropertyKind { Boolean, Integer { min: i64, max: i64 }, Choice { values: Vec<String> } });

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginV1 {
    plugin: String,
    version: String,
    tool: String,
    name: String,
    bounds: [[i32; 2]; 2],
    ports: Vec<api::Port>,
    graphics: Vec<Shape>,
    properties: Vec<(String, String, String, PropertyKind)>,
    actions: Vec<api::Action>,
    values: api::Properties,
}

impl From<api::Component> for PluginV1 {
    fn from(c: api::Component) -> Self {
        let t = c.tool;
        Self {
            plugin: c.plugin,
            version: c.version,
            tool: t.id,
            name: t.name,
            bounds: t.bounds,
            ports: t.ports,
            graphics: t.graphics.into_iter().map(Into::into).collect(),
            properties: t
                .properties
                .into_iter()
                .map(|p| (p.id, p.label, p.default, p.kind.into()))
                .collect(),
            actions: t.actions,
            values: c.properties,
        }
    }
}
impl From<PluginV1> for api::Component {
    fn from(p: PluginV1) -> Self {
        Self {
            plugin: p.plugin,
            version: p.version,
            properties: p.values,
            tool: api::Tool {
                id: p.tool,
                name: p.name,
                bounds: p.bounds,
                ports: p.ports,
                graphics: p.graphics.into_iter().map(Into::into).collect(),
                properties: p
                    .properties
                    .into_iter()
                    .map(|(id, label, default, kind)| api::Property {
                        id,
                        label,
                        default,
                        kind: kind.into(),
                    })
                    .collect(),
                actions: p.actions,
            },
        }
    }
}

pub(super) mod plugin_field {
    use super::*;
    pub(in crate::native) fn serialize<S: Serializer>(
        v: &Option<api::Component>,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        if s.is_human_readable() {
            v.serialize(s)
        } else {
            v.clone().map(PluginV1::from).serialize(s)
        }
    }
    pub(in crate::native) fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<Option<api::Component>, D::Error> {
        if d.is_human_readable() {
            Option::deserialize(d)
        } else {
            Ok(Option::<PluginV1>::deserialize(d)?.map(Into::into))
        }
    }
}
