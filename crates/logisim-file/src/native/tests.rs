use super::*;
#[test]
fn required_ids_and_empty_document_envelope() {
    let mut p = Project::default();
    assert!(serialize_native(&p).is_err());
    p.main_circuit_mut().circuit.agent_ids.circuit = Some(42);
    let bytes = serialize_native(&p).unwrap();
    assert_eq!(p, parse_native(&bytes).unwrap());
    assert_eq!(
        bytes,
        serialize_native(&parse_native(&bytes).unwrap()).unwrap()
    );
}

#[test]
fn persistent_targets_survive_reordering_and_reject_bad_references() {
    let mut p = Project::default();
    let sub = p.add("child".into()).unwrap();
    let pin = p
        .circuit_mut(sub)
        .unwrap()
        .circuit
        .add_component(logisim_core::Component::new(
            logisim_core::Kind::Pin,
            Point::ZERO,
        ));
    let a = p.circuit(sub).unwrap().editable_appearance();
    let c = &mut p.circuit_mut(sub).unwrap().circuit;
    c.appearance = Some(a);
    c.agent_ids.circuit = Some(1);
    c.agent_ids.items.insert(pin, 2);
    let instance = p.instance(sub, Point::new(200, 100)).unwrap();
    let c = &mut p.main_circuit_mut().circuit;
    c.agent_ids.circuit = Some(3);
    let id = c.add_component(instance);
    c.agent_ids.items.insert(id, 4);
    let bytes = serialize_native(&p).unwrap();
    let reopened = parse_native(&bytes).unwrap();
    assert_ne!(p.main, reopened.main);
    assert_eq!(bytes, serialize_native(&reopened).unwrap());
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for (pointer, bad) in [
        ("/version", "7"),
        ("/format", "other"),
        ("/main", "123"),
        (
            "/circuits/0/appearance/ports/0/pin",
            "c-00000000000000000000000000000004",
        ),
        (
            "/circuits/1/components/0/agent_id",
            "c-00000000000000000000000000000002",
        ),
        (
            "/circuits/1/components/0/kind/subcircuit",
            "k-00000000000000000000000000000009",
        ),
    ] {
        let mut v = value.clone();
        *v.pointer_mut(pointer).unwrap() = serde_json::json!(bad);
        assert!(
            parse_native(&serde_json::to_vec(&v).unwrap()).is_err(),
            "{pointer}"
        );
    }
}
