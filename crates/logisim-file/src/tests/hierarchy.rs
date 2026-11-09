use crate::*;
use logisim_core::{Facing, Point};

use super::support::*;

#[test]
fn multiple_circuits_forward_references_and_instance_attributes_roundtrip() {
    let source = r##"<project source="2.16.2.2.jar" version="1.0"><lib name="0" desc="#Wiring"/><main name="top"/>
    <circuit name="top"><comp name="part" loc="(200,100)"><a name="facing" val="south"/><a name="label" val="unit"/><a name="labelloc" val="west"/></comp><wire from="(200,70)" to="(200,40)"/></circuit>
    <circuit name="part"><comp lib="0" name="Pin" loc="(100,100)"><a name="tristate" val="false"/><a name="width" val="4"/></comp><comp lib="0" name="Pin" loc="(200,100)"><a name="output" val="true"/><a name="facing" val="west"/><a name="width" val="4"/></comp><wire from="(100,100)" to="(200,100)"/></circuit></project>"##;
    let imported = parse(source).unwrap();
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    let c = &imported.document.main_circuit().circuit.components[0];
    assert_eq!(c.interface.name, "part");
    assert_eq!(c.ports(), vec![Point::new(200, 70), Point::new(200, 100)]);
    assert_eq!(c.port_width(1), Some(4));
    assert_eq!(c.label_facing, Facing::West);
    let serialized = serialize(&imported.document).unwrap();
    assert!(serialized.contains("<comp loc=\"(200,100)\" name=\"part\">"));
    let reloaded = parse(&serialized).unwrap();
    assert!(reloaded.warnings.is_empty());
    assert_eq!(serialize(&reloaded.document).unwrap(), serialized);
    let mut renamed = imported.document;
    let target = renamed.named("part").unwrap().id;
    renamed.rename(target, "new & part".into()).unwrap();
    renamed.main = target;
    let read = parse(&serialize(&renamed).unwrap()).unwrap();
    assert_eq!(read.document.main_circuit().name, "new & part");
    assert_eq!(read.document.circuits.len(), 2);
}

#[test]
fn recursive_missing_and_custom_interfaces_never_silently_change_coordinates() {
    let source = xml("<comp name=\"sub\" loc=\"(100,100)\"/>").replace(
        "</project>",
        "<circuit name=\"sub\"><comp name=\"main\" loc=\"(0,0)\"/></circuit></project>",
    );
    assert!(parse(&source).unwrap_err().0.contains("recursive"));
    let source = source.replace(
        "<comp name=\"main\" loc=\"(0,0)\"/>",
        "<appear><rect x=\"0\" y=\"0\" width=\"80\" height=\"40\"/></appear>",
    );
    let imported = parse(&source).unwrap();
    assert!(!imported.warnings.is_empty());
    assert!(
        imported
            .document
            .main_circuit()
            .circuit
            .components
            .is_empty()
    );
    assert!(!serialize(&imported.document).unwrap().contains("<appear>"));
    let source = source.replace(
        "<appear><rect x=\"0\" y=\"0\" width=\"80\" height=\"40\"/></appear>",
        "<comp lib=\"w\" name=\"Pin\" loc=\"(0,0)\"><a name=\"pull\" val=\"invalid\"/></comp>",
    );
    let imported = parse(&source).unwrap();
    assert!(
        imported
            .document
            .main_circuit()
            .circuit
            .components
            .is_empty()
    );
    assert!(
        imported
            .warnings
            .iter()
            .any(|w| w.contains("interface pins"))
    );
}
