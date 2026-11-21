//! Optional agent-id attributes are independent of the ordinary component attribute codec.
use crate::{Error, Result, diagnostics::Warnings};
use logisim_core::AgentIdKind;
use roxmltree::Node;
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct Ids(HashSet<(AgentIdKind, u128)>);

impl Ids {
    pub fn read(
        &mut self,
        node: Node<'_, '_>,
        kind: AgentIdKind,
        circuit: &str,
        warnings: &mut Warnings,
    ) -> Option<u128> {
        let text = node.attribute("agent-id")?;
        let reason = match kind.decode(text) {
            Some(value) if self.0.insert((kind, value)) => return Some(value),
            Some(_) => "Duplicate agent-id; kept the first occurrence.",
            None => "Invalid agent-id; ignored this identifier.",
        };
        warnings.component(
            circuit,
            node.tag_name().name(),
            node.attribute("loc")
                .or(node.attribute("from"))
                .unwrap_or(circuit)
                .to_owned(),
            reason,
            false,
        );
        None
    }

    pub fn attribute(&mut self, kind: AgentIdKind, value: Option<u128>) -> Result<String> {
        let Some(value) = value else {
            return Ok(String::new());
        };
        let id = kind.encode(value);
        if !self.0.insert((kind, value)) {
            return Err(Error(format!("Duplicate agent-id {id}.")));
        }
        Ok(format!(" agent-id=\"{id}\""))
    }
}
