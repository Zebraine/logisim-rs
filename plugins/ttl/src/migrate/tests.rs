use super::*;
fn xml(name: &str, attrs: &str) -> String {
    format!(
        r##"<project source="2.16.2.2.jar" version="1.0"><lib name="t" desc="#TTL"/><main name="main"/><circuit name="main"><wire from="(290,230)" to="(290,270)"/><comp lib="t" name="{name}" loc="(280,200)">{attrs}</comp></circuit></project>"##
    )
}
fn source() -> PluginSource {
    PluginSource {
        plugin: crate::graphics::manifest().id.clone(),
        version: crate::graphics::manifest().version.clone(),
        path: "ttl.wasm".into(),
        fallback: String::new(),
    }
}
#[test]
fn mixed_batches_use_their_own_definitions_and_reject_ambiguous_or_missing_modules() {
    let api::Response::Manifest(extra) =
        logisim_ttl_combinational_plugin::handle(api::Request::Describe)
    else {
        panic!()
    };
    let extra_source = PluginSource {
        plugin: extra.id.clone(),
        version: extra.version.clone(),
        path: "ttl-combinational.wasm".into(),
        fallback: String::new(),
    };
    let modules = [
        (crate::graphics::manifest(), source()),
        (&extra, extra_source.clone()),
    ];
    let input = xml("7400", "").replace("</circuit>", r#"<comp lib="t" name="74283" loc="(500,200)"><a name="VccGndPorts" val="true"/></comp></circuit>"#);
    assert!(convert(&input, crate::graphics::manifest(), source()).is_err());
    assert!(convert(&input, &extra, extra_source.clone()).is_err());
    let converted = convert_many(&input, &modules).unwrap();
    let project = logisim_file::parse(&converted).unwrap().document;
    let components = &project.main_circuit().circuit.components;
    assert_eq!(
        components[0].plugin.as_ref().unwrap().plugin,
        source().plugin
    );
    assert_eq!(components[1].plugin.as_ref().unwrap().plugin, extra.id);
    assert_eq!(components[1].ports().len(), 16);
    assert_eq!(project.plugin_sources.len(), 2);
    let mut duplicate = extra.clone();
    duplicate.id = "test.ambiguous".into();
    let duplicate_source = PluginSource {
        plugin: duplicate.id.clone(),
        ..extra_source.clone()
    };
    assert!(
        convert_many(
            &input,
            &[
                (&extra, extra_source.clone()),
                (&duplicate, duplicate_source)
            ]
        )
        .is_err()
    );
    assert!(
        convert_many(
            &input,
            &[(&extra, extra_source.clone()), (&extra, extra_source)]
        )
        .is_err()
    );
    assert!(convert_many(&input.replace("74283", "74165"), &modules).is_err());
}
#[test]
fn all_models_options_and_orientations_migrate_without_moving_wires() {
    for kind in crate::model::Ttl::ALL {
        for facing in logisim_core::Facing::ALL {
            for power in [false, true] {
                for internal in [false, true] {
                    let input = xml(
                        kind.name(),
                        &format!(
                            r##"<a name="facing" val="{}"/><a name="VccGndPorts" val="{power}"/><a name="ShowInternalStructure" val="{internal}"/><a name="label" val="芯片 &amp; label"/><a name="labelfont" val="Dialog bold 18"/><a name="labelcolor" val="#123456"/>"##,
                            facing.name().to_lowercase()
                        ),
                    );
                    let text = convert(&input, crate::graphics::manifest(), source()).unwrap();
                    let p = logisim_file::parse(&text).unwrap().document;
                    let c = &p.main_circuit().circuit.components[0];
                    assert_eq!(c.kind, logisim_core::Kind::Plugin);
                    assert_eq!(c.facing, facing);
                    assert_eq!(c.label, "芯片 & label");
                    assert_eq!(c.font.size, 18);
                    assert!(c.font.bold);
                    assert_eq!(c.color, [0x12, 0x34, 0x56]);
                    assert_eq!(
                        c.plugin.as_ref().unwrap().properties["internal"],
                        internal.to_string()
                    );
                    assert_eq!(c.ports().len(), if power { 14 } else { 12 });
                    assert_eq!(
                        c.ports()[0],
                        logisim_core::Point::new(280, 200)
                            + facing.transform(logisim_core::Point::new(10, 30))
                    );
                    assert_eq!(
                        p.main_circuit().circuit.wires[0].a,
                        logisim_core::Point::new(290, 230)
                    );
                    let saved = logisim_file::serialize(&p).unwrap();
                    assert_eq!(
                        logisim_file::serialize(&logisim_file::parse(&saved).unwrap().document)
                            .unwrap(),
                        saved
                    );
                }
            }
        }
    }
}
#[test]
fn unknown_models_attributes_and_omissions_are_rejected() {
    for input in [
        xml("74125", ""),
        xml("7400-powered", ""),
        xml("7400", r#"<a name="VccGndPorts" val="yes"/>"#),
        xml("7400", r#"<a name="width" val="1"/>"#),
        xml("7400", r#"<a name="facing" val="up"/>"#),
        xml("7400", r##"<a name="labelcolor" val="#12"/>"##),
        xml(
            "7400",
            r#"<a name="label" val="a"/><a name="label" val="b"/>"#,
        ),
        xml("7400", "").replace("<main", r##"<lib name="t" desc="#TTL"/><main"##),
        xml("7400", "").replace(
            r##"<lib name="t" desc="#TTL"/>"##,
            r##"<lib name="t" desc="#TTL"><tool name="7400"><a name="VccGndPorts" val="true"/></tool></lib>"##,
        ),
        xml("7400", "").replace(
            "</circuit>",
            r#"<comp name="missing" loc="(1,2)"/></circuit>"#,
        ),
    ] {
        assert!(
            convert(&input, crate::graphics::manifest(), source()).is_err(),
            "{input}"
        );
    }
    let native = logisim_file::parse(&xml("7400", "")).unwrap();
    assert!(!native.warnings.is_empty());
    assert!(native.document.main_circuit().circuit.components.is_empty());
}
