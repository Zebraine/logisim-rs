//! Attribute-edit connection policy, separate from wire routing.
use crate::{Arithmetic, Component, Kind, Plexer, Point};
use std::collections::BTreeMap;

impl Component {
    /// Surviving port connections after an edit; absent entries keep their wire coordinates.
    pub(crate) fn connection_map(&self, replacement: &Self) -> BTreeMap<Point, Point> {
        let old = self;
        let before = old.ports();
        let after = replacement.ports();
        let pairs: Vec<_> = if old.kind == replacement.kind
            && (old.kind == Kind::PlaRom
                || old.kind.is_memory()
                || matches!(old.kind, Kind::DotMatrix | Kind::Joystick)
                || old.kind.is_transistor()
                || matches!(old.kind, Kind::Parity(_) | Kind::Latch(_) | Kind::Plugin)
                || (matches!(old.kind, Kind::Gate(_))
                    && old.gate_options.negated != replacement.gate_options.negated)
                || (old.kind.is_controlled()
                    && (old.bus.control_left != replacement.bus.control_left
                        || old.bus.size != replacement.bus.size))
                || (old.kind == Kind::Arithmetic(Arithmetic::BitAdder)
                    && old.bit_adder_inputs() != replacement.bit_adder_inputs())
                || (old.kind == Kind::Plexer(Plexer::DisplayDecoder)
                    && old.plexing.multibit != replacement.plexing.multibit))
        {
            // Observed input layout edits keep wire geometry; a simultaneous move translates it.
            let delta = replacement.position - old.position;
            before.iter().map(|p| (*p, *p + delta)).collect()
        } else if matches!(old.kind, Kind::Subcircuit(_)) && old.kind == replacement.kind {
            old.interface
                .ports
                .iter()
                .zip(&before)
                .filter_map(|(port, before)| {
                    replacement
                        .interface
                        .ports
                        .iter()
                        .position(|p| p.pin == port.pin)
                        .map(|index| (*before, after[index]))
                })
                .collect()
        } else if matches!(old.kind, Kind::Plexer(_)) && old.kind == replacement.kind {
            let new = replacement.plexer_terminals();
            old.plexer_terminals()
                .iter()
                .zip(&before)
                .filter_map(|(port, point)| {
                    new.iter()
                        .position(|p| p.role == port.role)
                        .map(|i| (*point, after[i]))
                })
                .collect()
        } else {
            before.iter().copied().zip(after).collect()
        };
        pairs.into_iter().filter(|(a, b)| a != b).collect()
    }
}
