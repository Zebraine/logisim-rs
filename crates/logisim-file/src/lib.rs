//! The supported .circ subset, independently specified from runtime-created files.
//! No filesystem or simulator dependency; unsupported content is never silently saved.
mod appearance;
mod arithmetic;
mod attributes;
mod bus;
#[cfg(test)]
mod compatibility_tests;
mod component;
mod diagnostics;
mod fonts;
mod gates;
mod hex;
mod io;
mod latches;
#[cfg(test)]
mod limits_tests;
mod memory;
mod parity;
mod pla;
mod plexers;
mod plugin;
mod read;
mod sequential;
mod terminal;
#[cfg(test)]
mod tests;
mod transistors;
mod ttl;
mod write;
mod xml;

pub use hex::{HexError, parse_hex};
pub use logisim_core::Project as Document;
pub use read::parse;
pub use write::serialize;

pub const MAX_BYTES: usize = 8 * 1024 * 1024;
/// Project-wide budget, independent of the per-circuit topology budget.
pub use logisim_core::{MAX_CIRCUIT_ITEMS, MAX_PROJECT_ITEMS as MAX_ITEMS};
const MAX_XML_NODES: u32 = 1_000_000;

#[derive(Debug)]
pub struct Import {
    pub document: Document,
    /// Any omitted or approximated content requires saving to a different file.
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
