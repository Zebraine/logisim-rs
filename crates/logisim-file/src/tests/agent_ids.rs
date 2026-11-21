use crate::{parse, serialize, validate_component};
use logisim_core::{AgentIdKind, Component, Kind, Point};

const XML: &str = r##"<project source="2.16.2.2.jar" version="1.0">
<lib name="w" desc="#Wiring"/><main name="top"/>
<circuit name="top" agent-id="k-0123456789abcdef0123456789abcdef">
<comp lib="w" name="Pin" loc="(0,0)" agent-id="c-0123456789abcdef0123456789abcdef"/>
<wire from="(0,0)" to="(50,0)" agent-id="w-0123456789abcdef0123456789abcdef"/>
<comp name="child" loc="(120,80)" agent-id="c-0123456789abcdef0123456789abcdee"/>
</circuit><circuit name="child"/></project>"##;

#[test]
fn optional_ids_survive_codec_on_builtin_instance_wire_and_definition() {
    let import = parse(XML).unwrap();
    assert!(import.warnings.is_empty() && !import.partial);
    let top = &import.document.main_circuit().circuit;
    assert_eq!(
        top.agent_ids.circuit,
        Some(0x0123456789abcdef0123456789abcdef)
    );
    assert!(
        import
            .document
            .named("child")
            .unwrap()
            .circuit
            .agent_ids
            .circuit
            .is_none()
    );
    for c in &top.components {
        validate_component(c, &import.document).unwrap();
    }
    let xml = serialize(&import.document).unwrap();
    assert_eq!(xml.matches(" agent-id=").count(), 4);
    assert!(xml.contains(r#"<circuit name="child">"#));
    let reopened = parse(&xml).unwrap();
    assert!(reopened.warnings.is_empty());
    assert_eq!(serialize(&reopened.document).unwrap(), xml);
    let mut plain = crate::Document::default();
    plain
        .main_circuit_mut()
        .circuit
        .add_component(Component::new(Kind::Pin, Point::ZERO));
    assert!(!serialize(&plain).unwrap().contains("agent-id"));
}

#[test]
fn wrong_prefix_case_length_and_duplicates_warn_without_dropping_objects() {
    let xml = XML.replace(
        r#"<circuit name="child"/>"#,
        r#"<circuit name="child" agent-id="k-0123456789abcdef0123456789abcdef">
      <comp lib="w" name="Pin" loc="(0,0)" agent-id="c-0123456789abcdef0123456789abcdef"/>
      <comp lib="w" name="Pin" loc="(50,0)" agent-id="w-0123456789abcdef0123456789abcdef"/>
      <comp lib="w" name="Pin" loc="(100,0)" agent-id="c-0123456789abcdef0123456789abcdeF"/>
      <wire from="(0,0)" to="(50,0)" agent-id="w-12"/>
      </circuit>"#,
    );
    let import = parse(&xml).unwrap();
    assert!(!import.partial);
    let notes = import.warnings.join("\n");
    assert!(notes.contains("Duplicate agent-id") && notes.contains("Invalid agent-id"));
    let child = &import.document.named("child").unwrap().circuit;
    assert!(child.agent_ids.items.is_empty() && child.agent_ids.circuit.is_none());
    assert_eq!((child.components.len(), child.wires.len()), (3, 1));
    assert_eq!(
        import.document.main_circuit().circuit.agent_ids.items.len(),
        3
    );
    let mut document = import.document;
    let top = &mut document.main_circuit_mut().circuit;
    let duplicate = top.agent_ids.items[&top.components[0].id];
    top.agent_ids.items.insert(top.components[1].id, duplicate);
    assert!(
        serialize(&document)
            .unwrap_err()
            .0
            .contains("Duplicate agent-id")
    );
}

#[test]
fn normalization_keeps_an_existing_wire_tag_when_an_extension_supplies_the_numeric_id() {
    let mut doc = parse(XML).unwrap().document;
    let c = &mut doc.main_circuit_mut().circuit;
    let original = c.wires[0];
    let tag = c.agent_ids.items[&original.id];
    c.remove(&std::collections::BTreeSet::from([c.components[0].id]));
    c.add_wire(Point::new(-20, 0), original.a).unwrap();
    c.normalize_wires();
    assert_eq!(c.wires.len(), 1);
    assert_ne!(c.wires[0].id, original.id);
    assert_eq!(c.agent_ids.items[&c.wires[0].id], tag);
    let imported = doc.with_library(&doc, "copy").unwrap();
    assert_eq!(
        imported.project.main_circuit().circuit.agent_ids,
        doc.main_circuit().circuit.agent_ids
    );
    assert!(
        imported.circuits.iter().all(
            |id| imported.project.circuit(*id).unwrap().circuit.agent_ids == Default::default()
        )
    );
    assert_eq!(
        serialize(&imported.project)
            .unwrap()
            .matches(&AgentIdKind::Wire.encode(tag))
            .count(),
        1
    );
}
