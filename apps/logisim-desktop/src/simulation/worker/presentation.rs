//! Keep the last visible values during an edit, without attaching them to replaced objects.
use logisim_core::{Circuit, Kind};
use logisim_ui::SimulationView;
use std::collections::BTreeSet;

pub(super) fn retain(view: &mut SimulationView, before: &Circuit, after: &Circuit) {
    let old: std::collections::BTreeMap<_, _> =
        before.components.iter().map(|c| (c.id, c)).collect();
    let mut ids = BTreeSet::new();
    let mut ports = BTreeSet::new();
    for c in &after.components {
        let Some(previous) = old.get(&c.id).filter(|p| {
            p.kind == c.kind
                && (c.kind != Kind::Plugin
                    || p.plugin.as_ref().map(|p| (&p.plugin, &p.version, &p.tool))
                        == c.plugin.as_ref().map(|p| (&p.plugin, &p.version, &p.tool)))
        }) else {
            continue;
        };
        ids.insert(c.id);
        for (index, (a, b)) in previous.ports().iter().zip(c.ports()).enumerate() {
            if *a == b && previous.port_width(index) == c.port_width(index) {
                ports.insert((c.id, index));
            }
        }
    }
    macro_rules! keep {
        ($($field:ident),* $(,)?) => { $(view.$field.retain(|id, _| ids.contains(id));)* };
    }
    keep!(
        plugin_graphics,
        plugin_errors,
        buttons,
        joysticks,
        keyboards,
        ttys,
        indicators,
        rgb,
        matrices,
        storage,
        memories,
        inputs
    );
    view.clocks.retain(|id| ids.contains(id));
    view.ports.retain(|port, _| ports.contains(port));
    let old_wires: std::collections::BTreeMap<_, _> =
        before.wires.iter().map(|w| (w.id, w)).collect();
    let wires: BTreeSet<_> = after
        .wires
        .iter()
        .filter(|w| old_wires.get(&w.id) == Some(w))
        .map(|w| w.id)
        .collect();
    view.wires.retain(|id, _| wires.contains(id));
    view.issues.retain(|issue| {
        issue
            .targets
            .iter()
            .all(|id| ids.contains(id) || wires.contains(id))
    });
}
