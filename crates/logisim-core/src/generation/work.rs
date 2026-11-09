use super::{Error, Result};

/// Deterministic work bound; elapsed time and threads belong to the host.
pub(super) struct Work<'a> {
    remaining: usize,
    until_poll: usize,
    cancelled: &'a dyn Fn() -> bool,
}
impl<'a> Work<'a> {
    pub fn new(cancelled: &'a dyn Fn() -> bool) -> Self {
        Self {
            remaining: 20_000_000,
            until_poll: 0,
            cancelled,
        }
    }
    pub fn step(&mut self) -> Result<()> {
        if self.until_poll == 0 {
            if (self.cancelled)() {
                return Err(Error::new("Computation cancelled."));
            }
            self.until_poll = 256;
        }
        self.until_poll -= 1;
        self.remaining = self.remaining.checked_sub(1).ok_or_else(Self::exhausted)?;
        Ok(())
    }
    pub fn spend(&mut self, units: usize) -> Result<()> {
        self.step()?;
        self.remaining = self
            .remaining
            .checked_sub(units)
            .ok_or_else(Self::exhausted)?;
        Ok(())
    }
    pub fn exhausted() -> Error {
        Error::new("Simplification exceeded its work or size limit. The truth table is preserved.")
    }
}
