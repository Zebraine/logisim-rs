//! Library descriptors are inert format metadata. No filesystem or JAR loader.
use crate::{
    Error, Result,
    xml::{attribute, escape},
};
use logisim_core::{Component, Kind, Trigger};
use std::{collections::BTreeMap, fmt::Write};

pub(crate) fn identify(descriptor: &str, name: &str) -> Option<Kind> {
    let (path, entry) = descriptor.strip_prefix("jar#")?.rsplit_once('#')?;
    if path.is_empty() || path.contains('#') {
        return None;
    }
    [Kind::RegisterFile, Kind::RiscVProbe]
        .into_iter()
        .find(|k| k.external_identity() == Some((entry, name)))
}
pub(crate) fn read(
    c: &mut Component,
    descriptor: &str,
    attrs: &BTreeMap<String, String>,
) -> Result<()> {
    for key in attrs.keys() {
        if c.kind != Kind::RegisterFile || key != "trigger" {
            return Err(Error(format!(
                "Unsupported external component attribute: {key}"
            )));
        }
    }
    c.external_library = descriptor.into();
    if c.kind == Kind::RegisterFile {
        c.sequential.trigger = match attrs.get("trigger").map(String::as_str).unwrap_or("rising") {
            "rising" => Trigger::Rising,
            "falling" => Trigger::Falling,
            "high" => Trigger::High,
            "low" => Trigger::Low,
            s => return Err(Error(format!("Unsupported register-file trigger: {s}"))),
        };
    }
    Ok(())
}
pub(crate) fn libraries(
    document: &crate::Document,
    out: &mut String,
) -> Result<BTreeMap<String, usize>> {
    let mut libraries = BTreeMap::new();
    for c in document
        .circuits
        .iter()
        .flat_map(|d| &d.circuit.components)
        .filter(|c| c.kind.is_external())
    {
        let (_, name) = c.kind.external_identity().expect("external identity");
        if identify(&c.external_library, name) != Some(c.kind) {
            return Err(Error(
                "External library identity does not match its component.".into(),
            ));
        }
        let next = libraries.len() + 8;
        libraries.entry(c.external_library.clone()).or_insert(next);
    }
    for (desc, id) in &libraries {
        writeln!(out, "  <lib desc=\"{}\" name=\"{id}\"/>", escape(desc)?).unwrap();
    }
    Ok(libraries)
}
pub(crate) fn write(out: &mut String, c: &Component, lib: usize) -> Result<()> {
    let mut expected = Component::new(c.kind, c.position);
    expected.id = c.id;
    expected.external_library = c.external_library.clone();
    if c.kind == Kind::RegisterFile {
        expected.sequential.trigger = c.sequential.trigger;
    }
    if c != &expected {
        return Err(Error("Unsupported external component attributes.".into()));
    }
    let (_, name) = c.kind.external_identity().expect("external identity");
    writeln!(
        out,
        "    <comp lib=\"{lib}\" loc=\"({},{})\" name=\"{name}\">",
        c.position.x, c.position.y
    )
    .unwrap();
    if c.kind == Kind::RegisterFile && c.sequential.trigger != Trigger::Rising {
        attribute(
            out,
            "trigger",
            match c.sequential.trigger {
                Trigger::Rising => "rising",
                Trigger::Falling => "falling",
                Trigger::High => "high",
                Trigger::Low => "low",
            },
        )?;
    }
    out.push_str("    </comp>\n");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use logisim_core::{Circuit, Point};
    #[test]
    fn native_libraries_round_trip_without_files_or_java() {
        let mut circuit = Circuit::default();
        for trigger in Trigger::ALL {
            let mut c = Component::new(
                Kind::RegisterFile,
                Point::new(300, 100 + 100 * circuit.components.len() as i32),
            );
            c.sequential.trigger = trigger;
            c.external_library = "jar#missing/path & name.jar#edu.cornell.cs3410.Components".into();
            circuit.add_component(c);
        }
        circuit.add_component(Component::new(Kind::RiscVProbe, Point::new(600, 100)));
        let document = crate::Document::single("main", circuit);
        let xml = crate::serialize(&document).unwrap();
        assert_eq!(xml.matches("desc=\"jar#").count(), 2);
        let imported = crate::parse(&xml).unwrap();
        assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
        assert_eq!(crate::serialize(&imported.document).unwrap(), xml);
        assert!(!xml.contains("java"));
    }
    #[test]
    fn unknown_identities_and_properties_are_reported() {
        let c = Component::new(Kind::RegisterFile, Point::ZERO);
        let mut circuit = Circuit::default();
        circuit.add_component(c);
        let xml = crate::serialize(&crate::Document::single("main", circuit)).unwrap();
        for bad in [
            xml.replace("edu.cornell.cs3410.Components", "other.Components"),
            xml.replace("name=\"RegisterFile\"", "name=\"Other\""),
            xml.replace("    </comp>", "<a name=\"width\" val=\"8\"/></comp>"),
            xml.replace(
                "    </comp>",
                "<a name=\"trigger\" val=\"invalid\"/></comp>",
            ),
        ] {
            let imported = crate::parse(&bad).unwrap();
            assert!(!imported.warnings.is_empty());
            assert!(
                imported
                    .document
                    .main_circuit()
                    .circuit
                    .components
                    .is_empty()
            );
        }
    }
    #[test]
    fn palette_defaults_do_not_change_instance_trigger_and_omissions_are_reported() {
        let mut circuit = logisim_core::Circuit::default();
        circuit.add_component(Component::new(
            Kind::RegisterFile,
            logisim_core::Point::ZERO,
        ));
        let xml=crate::serialize(&crate::Document::single("main",circuit)).unwrap()
            .replace("name=\"8\"/>","name=\"8\"><tool name=\"RegisterFile\"><a name=\"trigger\" val=\"falling\"/></tool><tool name=\"Other\"><a name=\"width\" val=\"8\"/></tool></lib>");
        let imported = crate::parse(&xml).unwrap();
        assert_eq!(imported.warnings.len(), 1);
        let component = &imported.document.main_circuit().circuit.components[0];
        assert_eq!(component.kind, Kind::RegisterFile);
        assert_eq!(component.sequential.trigger, Trigger::Rising);
    }
    #[test]
    fn serializer_rejects_inexpressible_external_fields() {
        for kind in [Kind::RegisterFile, Kind::RiscVProbe] {
            let mut circuit = Circuit::default();
            let mut c = Component::new(kind, Point::ZERO);
            c.bits = 8;
            circuit.add_component(c);
            assert!(crate::serialize(&crate::Document::single("main", circuit)).is_err());
        }
    }
}
