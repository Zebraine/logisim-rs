//! The names accepted by add_component and the session-independent component directory.
use super::{builtins, edit, properties};
use crate::Result;
use logisim_core::{Gate, Kind, Parity};
use serde_json::{Value, json};

const READ_ONLY: [Kind; 8] = [
    Kind::Rom,
    Kind::Ram,
    Kind::PlaRom,
    Kind::Plugin,
    Kind::Slider,
    Kind::Joystick,
    Kind::Keyboard,
    Kind::Tty,
];

pub(super) struct Entry {
    pub kind: Kind,
    alias: Option<&'static str>,
}

impl Entry {
    fn named(kind: Kind) -> Self {
        Self { kind, alias: None }
    }
    pub fn requires_circuit_id(&self) -> bool {
        matches!(self.kind, Kind::Subcircuit(_))
    }
}

fn entries() -> impl Iterator<Item = Entry> {
    [Kind::Pin, Kind::Probe, Kind::Tunnel, Kind::Splitter]
        .into_iter()
        .chain(Gate::ALL.map(Kind::Gate))
        .chain(Parity::ALL.map(Kind::Parity))
        // Only the kind's name is used here. Placement resolves the caller's actual target ID.
        .chain([Kind::Subcircuit(0)])
        .map(Entry::named)
        .chain(builtins::names().map(|(kind, alias)| Entry { kind, alias }))
        .chain(READ_ONLY.map(Entry::named))
}

pub(super) fn placement(name: &str) -> Result<Entry> {
    entries()
        .find(|entry| {
            entry.kind.name().eq_ignore_ascii_case(name)
                || entry
                    .alias
                    .is_some_and(|alias| alias.eq_ignore_ascii_case(name))
        })
        .filter(|entry| !properties::fields(entry.kind).is_empty())
        .ok_or_else(|| edit::unsupported(name))
}

pub(crate) fn catalog() -> Value {
    let mut kinds: Vec<_> = entries().collect();
    kinds.sort_unstable_by_key(|entry| entry.kind.name());
    let kinds: Vec<_> = kinds
        .into_iter()
        .map(|entry| {
            let fields = properties::fields(entry.kind);
            let writable = !fields.is_empty();
            json!({
                "name": entry.kind.name(),
                "aliases": entry.alias.as_slice(),
                "addable": writable,
                "editable": writable,
                "requires_circuit_id": entry.requires_circuit_id(),
                "properties": properties::property_schema(entry.kind, &fields),
            })
        })
        .collect();
    json!({"schema":1,"kinds":kinds})
}
