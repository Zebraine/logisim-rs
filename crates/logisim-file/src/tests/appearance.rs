use crate::*;
use logisim_core::Point;

use super::support::*;

#[test]
fn custom_vector_artwork_and_styled_text_roundtrip_with_exact_pin_bindings() {
    let source = appearance_project(
        r##"
      <rect fill="#ffffff" height="30" rx="5" ry="5" stroke="#000000" stroke-width="2" width="30" x="50" y="50"/>
      <path d="M61,52 Q65,61 69,52" fill="none" stroke="#808080" stroke-width="2"/>
      <rect height="20" stroke="#000000" width="20" x="70" y="100"/>
      <ellipse cx="130.5" cy="130.0" fill="none" rx="20.5" ry="10.0" stroke="#000000"/>
      <polyline fill="none" points="170,90 190,100 210,90" stroke="#000000"/>
      <polygon fill="none" points="170,150 200,160 170,170" stroke="#000000"/>
      <text font-family="Dialog" font-size="18" font-style="italic" font-weight="bold" text-anchor="middle" x="100" y="201">A &amp; B</text>"##,
    );
    let imported = parse(&source).unwrap();
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    let instance = &imported.document.main_circuit().circuit.components[0];
    assert_eq!(
        instance.ports(),
        vec![Point::new(310, 150), Point::new(310, 180)]
    );
    assert_eq!(instance.port_width(0), Some(4));
    let a = imported
        .document
        .named("part")
        .unwrap()
        .circuit
        .appearance
        .as_ref()
        .unwrap();
    assert_eq!(a.shapes.len(), 7);
    assert_eq!(a.shapes[2].paint.fill, Some([0; 3]));
    let serialized = serialize(&imported.document).unwrap();
    let reloaded = parse(&serialized).unwrap();
    assert!(reloaded.warnings.is_empty());
    assert_eq!(serialize(&reloaded.document).unwrap(), serialized);
    let mut moved = imported.document;
    let part = moved.named("part").unwrap().id;
    let c = &mut moved.circuit_mut(part).unwrap().circuit;
    let pin = c.components[0].id;
    let mut p = c.component(pin).unwrap().clone();
    p.position = Point::new(330, 140);
    c.replace_component(p);
    moved.refresh_interfaces(true);
    let saved = serialize(&moved).unwrap();
    assert!(saved.contains("pin=\"330,140\""));
    assert_eq!(
        parse(&saved)
            .unwrap()
            .document
            .main_circuit()
            .circuit
            .components[0]
            .ports(),
        vec![Point::new(310, 150), Point::new(310, 180)]
    );
}

#[test]
fn unknown_or_invalid_appearance_is_reported_and_never_replaced_with_default_instances() {
    for art in [
        r#"<image href="file:///tmp/image.png"/>"#,
        r#"<path d="M0,0 L10,10"/>"#,
        r#"<rect x="0" y="0" width="50" height="30" transform="rotate(90)"/>"#,
        r#"<ellipse cx="NaN" cy="5" rx="3" ry="4"/>"#,
        r#"<circ-port pin="100,100" x="0" y="0" width="8" height="8"/>"#,
        r#"<circ-anchor facing="west" width="6" height="6" x="7" y="7"/>"#,
    ] {
        let read = parse(&appearance_project(art)).unwrap();
        assert!(!read.warnings.is_empty());
        assert!(read.document.main_circuit().circuit.components.is_empty());
        assert!(!serialize(&read.document).unwrap().contains("<appear>"));
    }
    let bad = appearance_project("").replace("pin=\"100,100\"", "pin=\"100,110\"");
    assert!(!parse(&bad).unwrap().warnings.is_empty());
}

#[test]
fn custom_appearance_save_rejects_ambiguous_internal_pin_positions() {
    let mut p = parse(&appearance_project("")).unwrap().document;
    let child = p.named("part").unwrap().id;
    let c = &mut p.circuit_mut(child).unwrap().circuit;
    c.components[1].position = c.components[0].position;
    assert!(
        serialize(&p)
            .unwrap_err()
            .0
            .contains("unique internal positions")
    );
}
