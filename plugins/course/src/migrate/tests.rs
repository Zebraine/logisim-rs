use super::*;
const LEGACY: &str = r##"<project source="2.16.2.2.jar" version="1.0">
      <lib name="rf" desc="jar#missing.jar#edu.cornell.cs3410.Components">
        <tool name="RegisterFile"><a name="trigger" val="falling"/></tool>
      </lib>
      <lib name="probe" desc="jar#missing.jar#hust2020.Components"/>
      <lib name="0" desc="#Wiring"/>
      <main name="main"/>
      <circuit name="main">
        <wire from="(100,200)" to="(200,200)"/>
        <comp lib="rf" name="RegisterFile" loc="(300,200)"><a name="trigger" val="low"/></comp>
        <comp lib="probe" name="RiscV Probe" loc="(400,200)"/>
        <comp lib="0" name="Constant" loc="(100,200)"><a name="value" val="0x1"/></comp>
      </circuit>
    </project>"##;
fn source() -> PluginSource {
    let m = crate::graphics::manifest();
    PluginSource {
        plugin: m.id.clone(),
        version: m.version.clone(),
        path: "course.wasm".into(),
        fallback: String::new(),
    }
}
#[test]
fn migration_preserves_wires_positions_properties_and_uses_manifest_graphics() {
    let mut manifest = crate::graphics::manifest().clone();
    manifest.tools[0].name = "Plugin supplied display name".into();
    let output = convert(LEGACY, &manifest, source()).unwrap();
    assert!(!output.contains("jar#"));
    assert!(!output.contains("<tool"));
    assert!(output.contains(r#"<wire from="(100,200)" to="(200,200)"/>"#));
    let parsed = logisim_file::parse(&output).unwrap();
    assert!(parsed.warnings.is_empty());
    assert_eq!(parsed.document.plugin_sources, vec![source()]);
    let c = &parsed.document.main_circuit().circuit;
    assert_eq!(c.components.len(), 3);
    let rf = c
        .components
        .iter()
        .find(|c| c.position == logisim_core::Point::new(300, 200))
        .unwrap();
    let definition = rf.plugin.as_ref().unwrap();
    assert_eq!(definition.tool, manifest.tools[0]);
    assert_eq!(definition.properties["trigger"], "low");
    assert_eq!(c.wires.len(), 1);
    let defaults = convert(
        &LEGACY.replace(r#"<a name="trigger" val="low"/>"#, ""),
        &manifest,
        source(),
    )
    .unwrap();
    let defaults = logisim_file::parse(&defaults).unwrap();
    let rf = defaults
        .document
        .main_circuit()
        .circuit
        .components
        .iter()
        .find_map(|c| c.plugin.as_ref().filter(|p| p.tool.id == "register-file"))
        .unwrap();
    assert_eq!(rf.properties["trigger"], "rising");
}
#[test]
fn migration_refuses_unknown_components_attributes_and_omissions() {
    let manifest = crate::graphics::manifest();
    for broken in [
        LEGACY.replace("name=\"probe\" desc=", "name=\"rf\" desc="),
        LEGACY.replace("RiscV Probe", "Unknown"),
        LEGACY.replace("val=\"low\"", "val=\"invalid\""),
        LEGACY.replace("val=\"low\"", "val=\"low\" extra=\"x\""),
        LEGACY.replace("name=\"Constant\"", "name=\"Unimplemented\""),
        LEGACY.replace("loc=\"(300,200)\"", "loc=\"(300,200)\" rotation=\"90\""),
    ] {
        assert_ne!(LEGACY, broken);
        assert!(convert(&broken, manifest, source()).is_err(), "{broken}");
    }
    let mut wrong = source();
    wrong.version = "other".into();
    assert!(convert(LEGACY, manifest, wrong).is_err());
}
