//! Native course component pack. State is owned by the caller, one value per instance.
//! No scheduler, filesystem, Java, GUI, or process-global registration.
mod register_file;
mod riscv;
pub use register_file::RegisterFile;
pub use riscv::decode as decode_instruction;
