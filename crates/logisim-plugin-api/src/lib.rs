//! Version 1 plugin wire contract. All messages are UTF-8 JSON, enum tags snake_case.
//!
//! A Wasm32 module exports `memory`, `logisim_abi: () -> i32` (returns 1),
//! `logisim_buffer: (i32) -> i32`, and `logisim_call: (i32) -> i64`.
//! The host allocates an initialized request buffer, writes exactly the requested bytes,
//! then calls with that length. The returned integer packs response length in the high
//! 32 bits and offset in the low 32 bits. Response storage remains valid until the next
//! call. No imports, WASI, raw host pointers, or shared allocator are permitted.
//!
//! Describe is called on a disposable instance. Each simulated component gets a fresh
//! instance. Evaluate's explicit state is runtime-only, initially empty. Reset must
//! initialize state and outputs; Propagate must be stable with unchanged inputs/state.
//! Input and output arrays contain one entry for EVERY port (input-port output entries
//! are ignored). Output widths must match the declared ports. Inputs are sampled before
//! any component's new drives are committed. Actions are explicit UI requests.
//! A schema change requires a new plugin version; identity/version/definition must match
//! the saved document exactly. V1 ports and bounds are fixed, components face East.
//! Plugin strings are opaque to host translation. Protocol evolution uses a new ABI.
mod validate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const ABI_VERSION: u32 = 1;
pub const MAX_MESSAGE: usize = 256 * 1024;
pub const MAX_STATE: usize = 64 * 1024;
pub type Properties = BTreeMap<String, String>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub abi: u32,
    pub id: String,
    pub version: String,
    pub name: String,
    pub tools: Vec<Tool>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    pub id: String,
    pub name: String,
    pub bounds: [[i32; 2]; 2],
    pub ports: Vec<Port>,
    pub graphics: Vec<Shape>,
    pub properties: Vec<Property>,
    pub actions: Vec<Action>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Port {
    pub id: String,
    pub label: String,
    pub at: [i32; 2],
    pub width: u8,
    pub output: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Property {
    pub id: String,
    pub label: String,
    pub default: String,
    pub kind: PropertyKind,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PropertyKind {
    Boolean,
    Integer { min: i64, max: i64 },
    Choice { values: Vec<String> },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub id: String,
    pub label: String,
    pub argument: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Shape {
    Rect {
        min: [i32; 2],
        max: [i32; 2],
        fill: [u8; 4],
        stroke: [u8; 4],
    },
    Line {
        a: [i32; 2],
        b: [i32; 2],
        color: [u8; 4],
    },
    Ellipse {
        min: [i32; 2],
        max: [i32; 2],
        fill: [u8; 4],
        stroke: [u8; 4],
    },
    Text {
        at: [i32; 2],
        text: String,
        size: u8,
        color: [u8; 4],
    },
}
/// Least-significant bit first masks, 1–32 bits. Masks are disjoint and width-bounded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signal {
    pub width: u8,
    pub value: u32,
    pub unknown: u32,
    pub error: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    pub plugin: String,
    pub version: String,
    pub tool: Tool,
    pub properties: Properties,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    Reset,
    Propagate,
    Action { id: String, argument: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evaluation {
    pub tool: String,
    pub properties: Properties,
    pub inputs: Vec<Signal>,
    pub state: Vec<u8>,
    pub event: Event,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Update {
    pub outputs: Vec<Signal>,
    pub state: Vec<u8>,
    pub graphics: Vec<Shape>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Request {
    Describe,
    Evaluate(Evaluation),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Response {
    Manifest(Manifest),
    Update(Update),
    Error(String),
}

impl Manifest {
    pub fn components(&self) -> Vec<Component> {
        self.tools
            .iter()
            .map(|tool| Component {
                plugin: self.id.clone(),
                version: self.version.clone(),
                tool: tool.clone(),
                properties: tool
                    .properties
                    .iter()
                    .map(|p| (p.id.clone(), p.default.clone()))
                    .collect(),
            })
            .collect()
    }
}
