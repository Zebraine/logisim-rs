//! Closed pure-region IR. Executable memory is process-local and owned by Code.
#![cfg(feature = "circuit-jit-experiment")]

/// One bus in three planes, least significant bit first: ones, unknown, error.
pub type Word = [u32; 3];
pub const MAX_CHAIN: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Operand {
    Input,
    Constant(Word),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Region {
    /// Frame: one input followed by the drive of every gate in topological order.
    Unary { width: u8, invert: Vec<bool> },
    /// Frame: A, B, carry in, sum, carry out. Constant operands are immediate.
    Add { width: u8, operands: [Operand; 3] },
}

impl Region {
    fn frame_len(&self) -> Result<usize, String> {
        match self {
            Self::Unary {
                width: 1..=32,
                invert,
            } if (1..=MAX_CHAIN).contains(&invert.len()) => Ok(1 + invert.len()),
            Self::Add { width: 1..=32, .. } => Ok(5),
            _ => Err("Unsupported bus width or region length".into()),
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod native;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use native::Code;

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
pub struct Code(std::convert::Infallible);
#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
impl Code {
    pub fn compile(region: &Region) -> Result<Self, String> {
        region.frame_len()?;
        Err("The prototype supports Linux x86-64 System V".into())
    }
    pub fn run(&self, _: &mut [Word]) {
        match self.0 {}
    }
}
