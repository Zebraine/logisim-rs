//! Native course component pack. State is owned by the caller, one value per instance.
//! No scheduler, filesystem, Java, GUI, or process-global registration.
mod register_file;
mod riscv;
use logisim_core::Kind;
pub use register_file::RegisterFile;
pub use riscv::decode as decode_instruction;

/// Tools supplied by this pack; document identities and pin contracts live in core.
pub const TOOLS: [Kind; 2] = [Kind::RegisterFile, Kind::RiscVProbe];

pub fn identify(entry: &str, tool: &str) -> Option<Kind> {
    TOOLS
        .into_iter()
        .find(|kind| kind.external_identity() == Some((entry, tool)))
}
