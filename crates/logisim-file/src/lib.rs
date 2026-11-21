//! Native v1 CBOR projects and the supported legacy .circ subset.
//! No filesystem or simulator dependency; unsupported content is never silently saved.
mod agent_ids;
mod appearance;
mod arithmetic;
mod attributes;
mod bus;
mod component;
mod diagnostics;
mod fonts;
mod gates;
mod hex;
mod io;
mod latches;
#[cfg(test)]
mod tests;

mod format;
pub mod native;
pub use format::{ProjectFormat, rebase_plugin_sources};
pub use native::{parse_native, serialize_native};
mod memory;
mod parity;
mod pla;
mod plexers;
mod plugin;
mod read;
mod sequential;
mod terminal;

mod transistors;
mod write;
mod xml;

pub use hex::{HexError, parse_hex};
pub use logisim_core::Project as Document;
pub use read::parse;
pub use write::serialize;

/// Check one prospective component edit against the same attribute codec used at save time.
pub fn validate_component(component: &logisim_core::Component, document: &Document) -> Result<()> {
    component::write(&mut String::new(), component, document, "")
}

pub const MAX_BYTES: usize = 8 * 1024 * 1024;
/// Project-wide budget, independent of the per-circuit topology budget.
pub use logisim_core::{MAX_CIRCUIT_ITEMS, MAX_PROJECT_ITEMS as MAX_ITEMS};
const MAX_XML_NODES: u32 = 1_000_000;

#[derive(Debug)]
pub struct Import {
    pub document: Document,
    /// Content was omitted or approximated. Identity-only diagnostics do not set this flag.
    pub partial: bool,
    /// Includes recoverable problems with optional agent-id attributes.
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

fn valid_point(p: logisim_core::Point) -> bool {
    (-1_000_000..=1_000_000).contains(&p.x) && (-1_000_000..=1_000_000).contains(&p.y)
}
