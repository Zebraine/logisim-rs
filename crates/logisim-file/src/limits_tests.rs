use super::*;

fn project(circuits: &str) -> String {
    format!(
        r##"<project source="2.16.2.2.jar" version="1.0"><lib name="w" desc="#Wiring"/><lib name="b" desc="#Base"/><main name="main"/>{circuits}</project>"##
    )
}

#[test]
fn large_multi_circuit_project_roundtrips_beyond_old_item_and_node_budgets() {
    let mut circuits = "<!--padding-->".repeat(100_001);
    for name in ["main", "second"] {
        circuits.push_str(&format!(r#"<circuit name="{name}">"#));
        for i in 0..2500 {
            circuits.push_str(&format!(
                r#"<comp lib="w" name="Constant" loc="({},0)"/>"#,
                i * 10
            ));
        }
        circuits.push_str("</circuit>");
    }
    let imported = parse(&project(&circuits)).unwrap();
    assert!(imported.warnings.is_empty());
    assert_eq!(imported.document.circuits.len(), 2);
    let saved = serialize(&imported.document).unwrap();
    let restored = parse(&saved).unwrap();
    assert_eq!(restored.document, imported.document);
}

#[test]
fn raw_unsupported_items_still_count_toward_circuit_and_project_limits() {
    let item = r#"<comp lib="w" name="Unknown" loc="(0,0)"/>"#;
    let over = format!(
        r#"<circuit name="main">{}</circuit>"#,
        item.repeat(MAX_CIRCUIT_ITEMS + 1)
    );
    assert!(
        parse(&project(&over))
            .unwrap_err()
            .0
            .contains("Circuit 'main' exceeds")
    );
    let mut many = String::new();
    for i in 0..=MAX_ITEMS / MAX_CIRCUIT_ITEMS {
        let name = if i == 0 {
            "main".into()
        } else {
            format!("part{i}")
        };
        many.push_str(&format!(
            r#"<circuit name="{name}">{}</circuit>"#,
            item.repeat(MAX_CIRCUIT_ITEMS)
        ));
    }
    assert!(
        parse(&project(&many))
            .unwrap_err()
            .0
            .contains("Project exceeds")
    );
}

#[test]
fn omissions_remain_visible_after_hundreds_of_unique_display_notes() {
    let mut body = String::new();
    for i in 0..250 {
        body.push_str(&format!(r#"<comp lib="b" name="Text" loc="({i},0)"><a name="font" val="Family{i} plain 12"/></comp>"#));
    }
    for i in 0..8 {
        body.push_str(&format!(
            r#"<comp lib="w" name="MissingDevice" loc="({i},50)"/>"#
        ));
    }
    let imported = parse(&project(&format!(
        r#"<circuit name="main">{body}</circuit>"#
    )))
    .unwrap();
    assert!(imported.warnings[0].contains("Circuit 'main': Skipped MissingDevice"));
    assert!(imported.warnings[0].contains("8 occurrences"));
    assert!(imported.warnings[0].contains("(0,50)"));
    assert!(
        imported
            .warnings
            .last()
            .unwrap()
            .contains("further diagnostic groups")
    );
}

#[test]
fn blocked_instances_are_reported_with_parent_and_location_in_both_definition_orders() {
    let parent = r#"<circuit name="main"><comp name="child" loc="(100,200)"/></circuit>"#;
    let child = r#"<circuit name="child"><comp lib="w" name="Pin" loc="(0,0)"><a name="unimplemented" val="true"/></comp></circuit>"#;
    for circuits in [format!("{parent}{child}"), format!("{child}{parent}")] {
        let imported = parse(&project(&circuits)).unwrap();
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
                .any(|w| w.contains("Circuit 'main': Skipped child") && w.contains("(100,200)"))
        );
    }
}
