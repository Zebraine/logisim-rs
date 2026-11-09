use super::*;
use crate::{Facing, Gate, Project, generation::GateBasis};

fn fixture() -> (Analysis, CircuitDefinition) {
    let mut a = Analysis::default();
    for name in ["A", "B"] {
        a.add_input(name.into()).unwrap();
    }
    a.add_output("Y".into()).unwrap();
    a.apply_expression(0, "A & B").unwrap();
    let mut d = Project::single("child", a.generate().unwrap())
        .main_circuit()
        .clone();
    d.circuit.label.text = "Shared label".into();
    (a, d)
}

#[test]
fn unchanged_custom_interface_survives_variable_reordering_and_fresh_internal_ids() {
    let (mut a, mut d) = fixture();
    let mut appearance = d.editable_appearance();
    appearance.facing = Facing::North;
    for at in appearance.ports.values_mut() {
        *at = *at + Point::new(40, 70);
    }
    d.circuit.appearance = Some(appearance.clone());
    let target = RebuildTarget::bind(&d, &a).unwrap();
    let interface = d.interface();
    a.swap(true, 0, 1).unwrap();
    let old_ids = d.circuit.ids();
    let new = target
        .rebuild(&d, a.generate_with_basis(GateBasis::Nand).unwrap())
        .unwrap();
    assert_eq!(new.appearance, Some(appearance));
    assert_eq!(new.label, d.circuit.label);
    for c in &new.components {
        if c.kind == Kind::Pin {
            let old = d.circuit.component(c.id).unwrap();
            assert_eq!(
                (c.label.as_str(), c.facing, c.output),
                (old.label.as_str(), old.facing, old.output)
            );
        } else {
            assert!(!old_ids.contains(&c.id));
            assert_eq!(c.kind, Kind::Gate(Gate::Nand));
        }
    }
    assert!(new.wires.iter().all(|w| !old_ids.contains(&w.id)));
    d.circuit = new;
    assert_eq!(d.interface(), interface);
    let mut p = Project::single("child", d.circuit);
    p.refresh_interfaces(false);
    p.validate().unwrap();
}

#[test]
fn default_geometry_and_unnamed_pin_bindings_are_preserved() {
    let (mut a, mut d) = fixture();
    let target = RebuildTarget::bind(&d, &a).unwrap();
    assert!(
        target
            .rebuild(&d, a.generate().unwrap())
            .unwrap()
            .appearance
            .is_none()
    );
    for c in &mut d.circuit.components {
        if c.kind == Kind::Pin {
            c.label.clear();
        }
    }
    let target = RebuildTarget::bind(&d, &a).unwrap();
    let old = d.interface();
    // Reordering the internal input Pins would reverse the default package ports.
    a.swap(true, 0, 1).unwrap();
    d.circuit = target.rebuild(&d, a.generate().unwrap()).unwrap();
    let new = d.interface();
    assert!(new.appearance.is_some());
    assert_eq!(new.ports, old.ports);
    assert_eq!((new.min, new.max), (old.min, old.max));
    assert!(analysis_pins(&d.circuit).iter().all(|p| p.label.is_empty()));
}

#[test]
fn changed_interface_uses_list_order_and_keeps_artwork_and_anchor() {
    let (mut a, mut d) = fixture();
    let appearance = d.editable_appearance();
    let (min, max) = appearance.bounds();
    d.circuit.appearance = Some(appearance.clone());
    let target = RebuildTarget::bind(&d, &a).unwrap();
    let old = d.circuit.clone();
    a.add_input("C".into()).unwrap();
    a.swap(true, 0, 2).unwrap();
    a.rename(true, 1, "Renamed".into()).unwrap();
    a.add_output("Z".into()).unwrap();
    a.apply_expression(1, "C").unwrap();
    a.swap(false, 0, 1).unwrap();
    let new = target.rebuild(&d, a.generate().unwrap()).unwrap();
    let ports = &new.appearance.as_ref().unwrap().ports;
    assert_eq!(new.appearance.as_ref().unwrap().shapes, appearance.shapes);
    assert_eq!(new.appearance.as_ref().unwrap().anchor, appearance.anchor);
    for (output, labels) in [(false, vec!["C", "Renamed", "A"]), (true, vec!["Z", "Y"])] {
        for (i, label) in labels.iter().enumerate() {
            let pin = new
                .components
                .iter()
                .find(|p| p.kind == Kind::Pin && p.label == *label)
                .unwrap();
            assert_eq!(
                ports[&pin.id],
                Point::new(
                    if output { max.x } else { min.x },
                    min.y + 10 * (i as i32 + 1)
                )
            );
            if let Some(original) = old
                .components
                .iter()
                .find(|p| p.kind == Kind::Pin && p.label == *label)
            {
                assert_eq!(pin.id, original.id);
            } else {
                assert!(!old.ids().contains(&pin.id));
            }
        }
    }
    d.circuit = new;
    assert_eq!(d.interface().ports.len(), 5);
    Project::single("child", d.circuit).validate().unwrap();
}

#[test]
fn stale_targets_and_mismatched_analysis_cannot_replace_a_definition() {
    let (mut a, mut d) = fixture();
    let target = RebuildTarget::bind(&d, &a).unwrap();
    let generated = a.generate().unwrap();
    d.circuit.label.text = "changed".into();
    assert!(target.rebuild(&d, generated).is_err());
    a.rename(true, 0, "Other".into()).unwrap();
    assert!(RebuildTarget::bind(&d, &a).is_err());
}
