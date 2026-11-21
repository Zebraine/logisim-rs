//! Optional document identities. Geometry, electrical behavior and local ID allocation are independent.
use crate::Id;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentIds {
    pub circuit: Option<u128>,
    /// Component and wire numeric IDs share one allocator within their circuit.
    pub items: BTreeMap<Id, u128>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AgentIdKind {
    Circuit,
    Component,
    Wire,
}

impl AgentIdKind {
    fn prefix(self) -> &'static str {
        match self {
            Self::Circuit => "k-",
            Self::Component => "c-",
            Self::Wire => "w-",
        }
    }
    pub fn encode(self, id: u128) -> String {
        format!("{}{id:032x}", self.prefix())
    }
    pub fn decode(self, text: &str) -> Option<u128> {
        let digits = text.strip_prefix(self.prefix())?;
        if digits.len() != 32
            || !digits
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return None;
        }
        u128::from_str_radix(digits, 16).ok()
    }
}
