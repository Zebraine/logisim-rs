//! The supported .circ subset, independently specified from runtime-created files.
//! No filesystem or simulator dependency; unsupported content is never silently saved.
mod appearance;
mod arithmetic;
mod bus;
mod io;
mod memory;
mod plexers;
mod read;
mod sequential;
#[cfg(test)]
mod tests;
mod transistors;
mod write;

pub use logisim_core::Project as Document;
pub use read::parse;
pub use write::serialize;

pub const MAX_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_ITEMS: usize = 4096;

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
