//! Accounting shared by flat and hierarchical schedulers, independent of their recovery policy.
use crate::PROPAGATION_LIMIT;

#[derive(Default)]
pub(crate) struct Propagation {
    steps: u64,
    unsettled: usize,
}
impl Propagation {
    pub(crate) fn steps(&self) -> u64 {
        self.steps
    }
    /// Account for one committed batch; settled batches start a fresh convergence budget.
    pub(crate) fn record_batch(&mut self, pending: bool) -> bool {
        self.steps = self.steps.saturating_add(1);
        self.unsettled = if pending {
            self.unsettled.saturating_add(1)
        } else {
            0
        };
        self.unsettled >= PROPAGATION_LIMIT
    }
    /// New stimulus retries convergence without erasing the displayed propagation count.
    pub(crate) fn restart(&mut self) {
        self.unsettled = 0;
    }
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests;
