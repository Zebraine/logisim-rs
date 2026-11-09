//! Fixed memory drawing positions derived from the logical terminal interface.
use super::{MemoryRole, MemoryTerminal};
use crate::{Component, Point};

impl Component {
    pub fn memory_terminals(&self) -> Vec<MemoryTerminal> {
        let Some(logic) = self.memory_logic() else {
            return vec![];
        };
        logic
            .ports()
            .map(|port| MemoryTerminal {
                role: port.role,
                bits: port.bits,
                offset: match port.role {
                    MemoryRole::Data => Point::ZERO,
                    MemoryRole::Address => Point::new(-140, 0),
                    MemoryRole::Input => Point::new(-140, 20),
                    MemoryRole::Select => Point::new(-90, 40),
                    MemoryRole::Clock => Point::new(-70, 40),
                    MemoryRole::Load => Point::new(-50, 40),
                    MemoryRole::Store => Point::new(-110, 40),
                    MemoryRole::Clear => Point::new(-30, 40),
                },
            })
            .collect()
    }
}
