//! Explicit #TTL migration using the selected Wasm's definitions; never linked into the desktop.
use logisim_core::plugin::{PluginSource, api};
use std::collections::{BTreeMap, BTreeSet};
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\n', "&#10;")
        .replace('\r', "&#13;")
        .replace('\t', "&#9;")
}
fn attributes(n: roxmltree::Node<'_, '_>) -> Result<BTreeMap<String, String>, String> {
    let mut attrs: BTreeMap<String, String> = BTreeMap::new();
    for a in n.children().filter(|n| n.is_element()) {
        let key = a.attribute("name").ok_or("Missing attribute name")?;
        if !a.has_tag_name("a")
            || ![
                "facing",
                "VccGndPorts",
                "ShowInternalStructure",
                "label",
                "labelfont",
                "labelcolor",
            ]
            .contains(&key)
            || a.attributes().any(|a| !["name", "val"].contains(&a.name()))
            || a.children().any(|n| n.is_element() || n.is_text())
            || attrs
                .insert(
                    key.into(),
                    a.attribute("val").ok_or("Missing attribute value")?.into(),
                )
                .is_some()
        {
            return Err("Unsupported or duplicate TTL attributes".into());
        }
    }
    for key in ["VccGndPorts", "ShowInternalStructure"] {
        if attrs
            .get(key)
            .is_some_and(|s| !["true", "false"].contains(&s.as_str()))
        {
            return Err(format!("Invalid {key}"));
        }
    }
    Ok(attrs)
}
pub fn convert(
    xml: &str,
    manifest: &api::Manifest,
    source: PluginSource,
) -> Result<String, String> {
    convert_many(xml, &[(manifest, source)])
}
/// Route observed TTL names to explicit module definitions; ambiguous models fail atomically.
pub fn convert_many(
    xml: &str,
    modules: &[(&api::Manifest, PluginSource)],
) -> Result<String, String> {
    if xml.len() > logisim_file::MAX_BYTES {
        return Err("Input exceeds the circuit file limit".into());
    }
    if modules.is_empty() || modules.len() > 16 {
        return Err("Choose 1–16 TTL modules".into());
    }
    let mut definitions = BTreeMap::new();
    let mut identities = BTreeSet::new();
    for (manifest, source) in modules {
        manifest.validate()?;
        source.validate()?;
        if source.plugin != manifest.id
            || source.version != manifest.version
            || !identities.insert((&manifest.id, &manifest.version))
        {
            return Err("Plugin source identity mismatch or duplicate".into());
        }
        for definition in manifest.components() {
            let name = definition
                .tool
                .id
                .strip_suffix("-powered")
                .unwrap_or(&definition.tool.id);
            if name.is_empty() || !name.bytes().all(|b| b.is_ascii_digit()) {
                return Err("Expected numeric TTL tool identities".into());
            }
            if definitions
                .insert(definition.tool.id.clone(), definition)
                .is_some()
            {
                return Err("Ambiguous TTL model across modules".into());
            }
        }
    }
    let known = |name: Option<&str>| {
        name.is_some_and(|name| {
            !name.is_empty()
                && name.bytes().all(|b| b.is_ascii_digit())
                && definitions.contains_key(name)
        })
    };
    let tree = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 1_000_000,
        },
    )
    .map_err(|e| e.to_string())?;
    let root = tree.root_element();
    if !root.has_tag_name("project")
        || !matches!(
            root.attribute("source"),
            Some("2.16.2.2.jar" | "logisim-rs-plugin-v1")
        )
    {
        return Err("Expected a supported circuit file".into());
    }
    let mut libraries = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut edits = vec![];
    for lib in root.children().filter(|n| n.has_tag_name("lib")) {
        let id = lib.attribute("name").ok_or("Missing library identity")?;
        if !ids.insert(id) {
            return Err("Duplicate library identity".into());
        }
        if lib.attribute("desc") == Some("#TTL") {
            if lib
                .attributes()
                .any(|a| !["name", "desc"].contains(&a.name()))
            {
                return Err("Unsupported TTL library metadata".into());
            }
            for tool in lib.children().filter(|n| n.is_element()) {
                if !tool.has_tag_name("tool")
                    || tool.attributes().any(|a| a.name() != "name")
                    || !known(tool.attribute("name"))
                {
                    return Err("Unsupported TTL library tool".into());
                }
                if !attributes(tool)?.is_empty() {
                    return Err("TTL library defaults require explicit component attributes".into());
                }
            }
            libraries.insert(id);
            edits.push((lib.range(), String::new()));
        }
    }
    let mut lib_id = "ttl-wasm".to_owned();
    while ids.contains(lib_id.as_str()) {
        lib_id.push('_');
    }
    let mut used = BTreeSet::new();
    let mut count = 0;
    for c in root.descendants().filter(|n| n.has_tag_name("comp")) {
        if !c.attribute("lib").is_some_and(|id| libraries.contains(id)) {
            continue;
        }
        if c.attributes()
            .any(|a| !["lib", "name", "loc"].contains(&a.name()))
        {
            return Err("Unsupported TTL component metadata".into());
        }
        let name = c.attribute("name").ok_or("Missing TTL model")?;
        if !known(Some(name)) {
            return Err("Unsupported TTL model".into());
        }
        let attrs = attributes(c)?;
        let id = format!(
            "{name}{}",
            if attrs.get("VccGndPorts").is_some_and(|s| s == "true") {
                "-powered"
            } else {
                ""
            }
        );
        let mut definition = definitions
            .get(&id)
            .ok_or("Plugin has no compatible TTL tool")?
            .clone();
        used.insert((definition.plugin.clone(), definition.version.clone()));
        definition.properties.insert(
            "internal".into(),
            attrs
                .get("ShowInternalStructure")
                .cloned()
                .unwrap_or_else(|| "false".into()),
        );
        definition.validate()?;
        let json = serde_json::to_string(&definition).map_err(|e| e.to_string())?;
        let mut out = format!(
            "<comp lib=\"{lib_id}\" name=\"{}\" loc=\"{}\"><a name=\"definition\" val=\"{}\"/>",
            id,
            escape(c.attribute("loc").ok_or("Missing position")?),
            escape(&json)
        );
        for (key, default) in [
            ("facing", "east"),
            ("label", ""),
            ("labelfont", "Dialog plain 12"),
            ("labelcolor", "#000000"),
        ] {
            out.push_str(&format!(
                "<a name=\"{key}\" val=\"{}\"/>",
                escape(attrs.get(key).map(String::as_str).unwrap_or(default))
            ));
        }
        out.push_str("</comp>");
        edits.push((c.range(), out));
        count += 1;
    }
    if count == 0 {
        return Err("No supported legacy TTL components found".into());
    }
    for tool in root.descendants().filter(|n| {
        n.has_tag_name("tool") && n.attribute("lib").is_some_and(|l| libraries.contains(l))
    }) {
        if !known(tool.attribute("name")) {
            return Err("Unsupported TTL palette tool".into());
        }
        attributes(tool)?;
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
    let insert = xml[root.range().start..]
        .find('>')
        .ok_or("Invalid project tag")?
        + root.range().start
        + 1;
    let sources: Vec<_> = modules
        .iter()
        .filter(|(_, s)| used.contains(&(s.plugin.clone(), s.version.clone())))
        .map(|(_, s)| s)
        .collect();
    let sources = serde_json::to_string(&sources).map_err(|e| e.to_string())?;
    edits.push((insert..insert,format!("\n<lib name=\"{lib_id}\" desc=\"logisim-rs#plugin-v1\"><a name=\"sources\" val=\"{}\"/></lib>",escape(&sources))));
    // An insertion at the start of the first library must precede its removal.
    edits.sort_by_key(|(r, _)| (r.start, r.end));
    if edits.windows(2).any(|w| w[0].0.end > w[1].0.start) {
        return Err("Overlapping migration edits".into());
    }
    let mut out = xml.to_owned();
    for (r, text) in edits.into_iter().rev() {
        out.replace_range(r, &text);
    }
    let imported = logisim_file::parse(&out).map_err(|e| e.to_string())?;
    if !imported.warnings.is_empty() {
        return Err(format!(
            "Migration would omit unsupported content: {}",
            imported.warnings.join("\n")
        ));
    }
    Ok(out)
}
#[cfg(test)]
mod tests;
