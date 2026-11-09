//! Versioned plugin wire contract. All messages are UTF-8 JSON, enum tags snake_case.
//!
//! A Wasm32 module exports `memory`, `logisim_abi: () -> i32` (returns its ABI version),
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
//! the saved document exactly. Ports and bounds are fixed in local coordinates; placement rotation and labels
//! are host document metadata and do not alter the plugin protocol.
//! Plugin strings use only plugin-owned translations via the optional versioned
//! [`presentation`] extension. Simulation protocol evolution uses a new ABI.
//!
//! Coordinates are integer grid units relative to the component origin, with +x right
//! and +y down. Ports lie on multiples of ten. Shapes are painted in vector order,
//! static graphics first, then runtime graphics, clipped to the declared bounds.
//! Text uses a host monospace font, top-left alignment and a size in grid units.
//! Colors are unpremultiplied RGBA. No egui objects, callbacks or external resources
//! cross this interface. Boolean properties are `true`/`false`, integers are decimal,
//! and choices are their declared string values; keys must match the schema exactly.
//!
//! Each runtime instance starts with Reset, even if all inputs are unknown. A property
//! edit or Reset creates a new VM; moves and rewiring retain the VM and explicit state.
//! Hosts may skip Propagate when the sampled inputs have not changed. Do not use the
//! invocation count as a clock. Stateful edge detection belongs in explicit state.
//! Action arguments are UTF-8, at most 1024 bytes; actions without an argument use "".
//! Invalid responses, traps or exhausted fuel fault the instance until Reset: output
//! ports drive Error and runtime graphics are discarded. Document snapshots retain
//! only identity/version, tool metadata and properties, never runtime state or code.
//!
//! V1 host limits: a 4 MiB module, 16 MiB linear memory per VM, one memory/table,
//! at most 10,000 table entries, 10,000,000 Wasm fuel units per call (including buffer
//! allocation), 256 KiB UTF-8 messages, and 64 KiB opaque state. Metadata constraints
//! are enforced by `Manifest::validate` and `Tool::validate_evaluation/validate_update`.
//!
//! ABI 2 adds opt-in stable-state requests; ABI 1 retains only Describe/Evaluate.
//! See [`StableContract`]. The manifest ABI must equal the exported ABI. Existing
//! component schemas and event behavior are unchanged; an additive execution capability
//! alone does not require changing the saved component version or Tool snapshot.
//! ABI 3 adds [`StateTransition`] results for stable sampling. ABI 2 modules only
//! receive the original StableSample request and use conservative cache invalidation.
pub mod frame;
pub mod presentation;
mod stable;
mod validate;
use serde::{Deserialize, Serialize};
pub use stable::*;
use std::collections::BTreeMap;

pub const ABI_VERSION: u32 = 3;
pub const STABLE_ABI_VERSION: u32 = 2;
pub const LEGACY_ABI_VERSION: u32 = 1;
pub fn supports_abi(abi: u32) -> bool {
    matches!(abi, LEGACY_ABI_VERSION | STABLE_ABI_VERSION | ABI_VERSION)
}
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
    StableContract(StableConfig),
    StableOutputs(StableEvaluation),
    StableSample(StableEvaluation),
    StableTransition(StableEvaluation),
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
    StableContract(Option<StableContract>),
    StableOutputs(StableOutputs),
    StableState(Vec<u8>),
    StableTransition(StateTransition),
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
