//! Read through the desktop wire schema. Preserve local-only fields verbatim as JSON values.
use crate::{Error, Operation, Result, digest::sha256, io};
use logisim_preferences::{
    model::{CONFIG_VERSION, MAX_TICK_HZ, UiScale},
    schema::File,
    storage::{WriteError, write_preferences},
};
use serde_json::{Value, json};
use std::{collections::HashSet, path::Path};

fn invalid(message: impl ToString) -> Error {
    Error::new("invalid_settings", message)
}

fn portable(bytes: &[u8]) -> Result<Value> {
    let file: File = serde_json::from_slice(bytes).map_err(invalid)?;
    file.validate_local().map_err(invalid)?;
    let mut settings = serde_json::to_value(file.portable()).map_err(invalid)?;
    settings.as_object_mut().unwrap().remove("version");
    Ok(settings)
}

pub fn catalog() -> Value {
    let defaults = portable(&serde_json::to_vec(&json!({"version":CONFIG_VERSION})).unwrap())
        .expect("valid schema defaults");
    let mut entries = Vec::new();
    for (section, fields) in defaults.as_object().unwrap() {
        for (name, default) in fields.as_object().unwrap() {
            let key = format!("{section}.{name}");
            let mut info = match key.as_str() {
                "general.language" => choices(json!(["en", "zh-CN"])),
                "appearance.theme" => choices(json!(["light", "dark"])),
                "appearance.density" => choices(json!(["comfortable", "compact"])),
                "appearance.accent" => choices(json!(["indigo", "blue", "teal"])),
                "appearance.signal_palette" => {
                    choices(json!(["default", "high-contrast", "colorblind-safe"]))
                }
                "appearance.ui_scale" => {
                    choices(json!(UiScale::ALL.map(|s| f64::from(s.percent()) / 100.0)))
                }
                "editor.default_zoom" => choices(json!(["fit", "100%"])),
                "editor.default_probe_radix" => {
                    choices(json!(["binary", "octal", "signed", "unsigned", "hex"]))
                }
                "editor.default_gate_size" => choices(json!(["narrow", "medium", "wide"])),
                "editor.default_font_family" => choices(json!(["dialog", "monospace", "serif"])),
                "simulation.mode" => choices(json!(["stable", "event"])),
                "simulation.tick_hz" => range(1, MAX_TICK_HZ),
                "editor.default_gate_inputs" => range(2, 32),
                "editor.default_pin_bits" | "editor.default_tunnel_width" => range(1, 32),
                "editor.default_font_size" => {
                    json!({"type":"integer","minimum":8,"maximum":32,"nullable":true})
                }
                "files.autosave_minutes" => range(1, 30),
                "workspace.recent_limit" => range(1, 20),
                _ if section == "shortcuts" => json!({"type":"chord"}),
                _ if default.is_boolean() => json!({"type":"bool"}),
                _ => unreachable!("missing catalog metadata for {key}"),
            };
            info["key"] = key.into();
            info["default"] = default.clone();
            entries.push(info);
        }
    }
    json!({"version":CONFIG_VERSION,"entries":entries})
}
fn choices(values: Value) -> Value {
    json!({"type":"enum","values":values})
}
fn range(min: u32, max: u32) -> Value {
    json!({"type":"integer","minimum":min,"maximum":max})
}

pub fn get(path: &Path) -> Result<Value> {
    let bytes = io::read(path)?;
    Ok(
        json!({"path":path,"version":CONFIG_VERSION,"source_sha256":sha256(&bytes),"settings":portable(&bytes)?,"warnings":[]}),
    )
}

pub fn patch(
    path: &Path,
    expected: Option<&str>,
    ops: Vec<Operation>,
    dry_run: bool,
) -> Result<Value> {
    let path = path.canonicalize().map_err(io::file_error)?;
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("circ"))
    {
        return Err(Error::invalid("Expected a preferences file."));
    }
    let bytes = io::read(&path)?;
    let before_hash = sha256(&bytes);
    if let Some(expected) = expected {
        if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::invalid("Expected 64 hexadecimal SHA-256 digits."));
        }
        if !expected.eq_ignore_ascii_case(&before_hash) {
            return Err(Error::new("conflict", "Source SHA-256 does not match."));
        }
    }
    let before = portable(&bytes)?;
    let mut document: Value = serde_json::from_slice(&bytes).map_err(invalid)?;
    let mut seen = HashSet::new();
    for op in ops {
        let (section, field) = op
            .key
            .split_once('.')
            .ok_or_else(|| Error::invalid("Invalid setting key."))?;
        if before.get(section).and_then(|v| v.get(field)).is_none() {
            return Err(Error::new(
                "unknown_setting",
                format!("Unknown setting: {}", op.key),
            ));
        }
        if !seen.insert(op.key.clone()) {
            return Err(Error::invalid(format!(
                "Duplicate operation for {}.",
                op.key
            )));
        }
        document[section][field] = op.value;
    }
    document["version"] = CONFIG_VERSION.into();
    let mut encoded = serde_json::to_vec_pretty(&document).map_err(invalid)?;
    encoded.push(b'\n');
    if encoded.len() > logisim_file::MAX_BYTES {
        return Err(Error::new("too_large", "Result exceeds 8 MiB."));
    }
    let after = portable(&encoded)?;
    let changed = before != after;
    if !dry_run && changed {
        write_preferences(&path, &encoded, Some(&bytes)).map_err(|e| {
            Error::new(
                match e {
                    WriteError::Conflict => "conflict",
                    WriteError::Io(_) => "io",
                },
                e,
            )
        })?;
    }
    let result_hash = if changed {
        sha256(&encoded)
    } else {
        before_hash.clone()
    };
    Ok(
        json!({"path":path,"version":CONFIG_VERSION,"dry_run":dry_run,"changed":changed,
        "source_sha256":if dry_run {&before_hash}else{&result_hash},"previous_sha256":before_hash,"result_sha256":result_hash,"settings":after,"warnings":[]}),
    )
}
