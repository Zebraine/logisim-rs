//! Explicit migration owned by this plugin package, never linked into the desktop.
//! The two legacy identities/attributes were specified from observed course files.
use logisim_core::plugin::{PluginSource, api};
use std::collections::BTreeMap;
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\n', "&#10;")
        .replace('\r', "&#13;")
        .replace('\t', "&#9;")
}
pub fn convert(
    xml: &str,
    manifest: &api::Manifest,
    source: PluginSource,
) -> Result<String, String> {
    if xml.len() > logisim_file::MAX_BYTES {
        return Err("Input exceeds the circuit file limit.".into());
    }
    manifest.validate()?;
    source.validate()?;
    if source.plugin != manifest.id || source.version != manifest.version {
        return Err("Plugin source identity mismatch.".into());
    }
    let tree = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 1_000_000,
        },
    )
    .map_err(|e| e.to_string())?;
    let root = tree.root_element();
    if !root.has_tag_name("project") || root.attribute("source") != Some("2.16.2.2.jar") {
        return Err("Expected an observed Logisim-ITA course circuit.".into());
    }
    let mut libraries = BTreeMap::new();
    let mut library_ids = std::collections::BTreeSet::new();
    let mut edits = vec![];
    for lib in root.children().filter(|n| n.has_tag_name("lib")) {
        let id = lib.attribute("name").ok_or("Library has no identity")?;
        if !library_ids.insert(id) {
            return Err("Duplicate library identity; migration cancelled.".into());
        }
        let Some(descriptor) = lib.attribute("desc") else {
            continue;
        };
        let Some((_, entry)) = descriptor
            .strip_prefix("jar#")
            .and_then(|s| s.rsplit_once('#'))
        else {
            continue;
        };
        if matches!(
            entry,
            "edu.cornell.cs3410.Components" | "hust2020.Components"
        ) {
            libraries.insert(id, entry);
            edits.push((lib.range(), String::new()));
        }
    }
    let mut lib_id = "course-wasm".to_owned();
    while root
        .children()
        .any(|n| n.has_tag_name("lib") && n.attribute("name") == Some(lib_id.as_str()))
    {
        lib_id.push('_');
    }
    let definitions = manifest.components();
    let mut count = 0;
    for c in root.descendants().filter(|n| n.has_tag_name("comp")) {
        let Some(entry) = c.attribute("lib").and_then(|l| libraries.get(l)) else {
            continue;
        };
        let tool = match (*entry, c.attribute("name")) {
            ("edu.cornell.cs3410.Components", Some("RegisterFile")) => "register-file",
            ("hust2020.Components", Some("RiscV Probe")) => "riscv-probe",
            _ => return Err("Unsupported course-library component; migration cancelled.".into()),
        };
        if c.attributes()
            .any(|a| !["lib", "name", "loc"].contains(&a.name()))
        {
            return Err("Unsupported course component metadata.".into());
        }
        let mut definition = definitions
            .iter()
            .find(|d| d.tool.id == tool)
            .ok_or("Plugin has no compatible tool")?
            .clone();
        let mut attributes = std::collections::BTreeSet::new();
        for a in c.children().filter(|n| n.is_element()) {
            if !a.has_tag_name("a")
                || tool != "register-file"
                || a.attribute("name") != Some("trigger")
                || !attributes.insert("trigger")
                || a.attributes().any(|a| !["name", "val"].contains(&a.name()))
                || a.children().any(|n| n.is_element() || n.is_text())
            {
                return Err("Unsupported course component attributes.".into());
            }
            definition.properties.insert(
                "trigger".into(),
                a.attribute("val").ok_or("Missing trigger value")?.into(),
            );
        }
        definition.validate()?;
        let json = serde_json::to_string(&definition).map_err(|e| e.to_string())?;
        let position = escape(c.attribute("loc").ok_or("Missing component position")?);
        edits.push((c.range(),format!("<comp lib=\"{lib_id}\" name=\"{tool}\" loc=\"{position}\"><a name=\"definition\" val=\"{}\"/></comp>", escape(&json))));
        count += 1;
    }
    if count == 0 {
        return Err("No supported legacy course components found.".into());
    }
    // Palette defaults are not instance attributes. Remove references to the old libraries.
    for tool in root.descendants().filter(|n| {
        n.has_tag_name("tool")
            && n.attribute("lib")
                .is_some_and(|id| libraries.contains_key(id))
    }) {
        if !edits
            .iter()
            .any(|(r, _)| r.start <= tool.range().start && r.end >= tool.range().end)
        {
            edits.push((tool.range(), String::new()));
        }
    }
    edits.push((
        root.attribute_node("source").unwrap().range_value(),
        "logisim-rs-plugin-v1".into(),
    ));
    let insert_at = xml[root.range().start..]
        .find('>')
        .ok_or("Invalid project opening tag")?
        + root.range().start
        + 1;
    let sources = serde_json::to_string(&vec![source]).map_err(|e| e.to_string())?;
    edits.push((insert_at..insert_at, format!("\n  <lib name=\"{lib_id}\" desc=\"logisim-rs#plugin-v1\"><a name=\"sources\" val=\"{}\"/></lib>",escape(&sources))));
    edits.sort_by_key(|(r, _)| r.start);
    if edits.windows(2).any(|w| w[0].0.end > w[1].0.start) {
        return Err("Overlapping XML migration edits.".into());
    }
    let mut output = xml.to_owned();
    for (range, replacement) in edits.into_iter().rev() {
        output.replace_range(range, &replacement);
    }
    let imported = logisim_file::parse(&output).map_err(|e| e.to_string())?;
    if !imported.warnings.is_empty() {
        return Err(format!(
            "Migration would omit unsupported content: {}",
            imported.warnings.join("\n")
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests;
