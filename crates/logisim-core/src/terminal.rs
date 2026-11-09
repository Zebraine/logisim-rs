//! Character terminal settings only; text buffers and cursor belong to simulator instances.
use crate::{Kind, Trigger};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalOptions {
    pub buffer_len: u16,
    pub rows: u8,
    pub columns: u8,
    pub trigger: Trigger,
}
impl Default for TerminalOptions {
    fn default() -> Self {
        Self {
            buffer_len: 32,
            rows: 8,
            columns: 32,
            trigger: Trigger::Rising,
        }
    }
}
impl Kind {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Keyboard | Self::Tty)
    }
}

#[cfg(test)]
mod tests;
