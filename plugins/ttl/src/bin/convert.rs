//! Convert an observed legacy TTL circuit using definitions from an actual Wasm module.
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};
fn read(path: &std::path::Path, limit: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = vec![];
    fs::File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err("File exceeds its size limit".into());
    }
    Ok(bytes)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 3 {
        return Err("Usage: ttl-plugin-convert module.wasm source.circ new.circ".into());
    }
    let module = args[0].canonicalize()?;
    let plugin = logisim_plugin::Plugin::load(&read(&module, logisim_plugin::MAX_MODULE)?)?;
    let manifest = plugin.manifest();
    let base = args[2]
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."))
        .canonicalize()?;
    let source = logisim_core::plugin::PluginSource {
        plugin: manifest.id.clone(),
        version: manifest.version.clone(),
        path: module
            .strip_prefix(&base)
            .unwrap_or(&module)
            .to_string_lossy()
            .into_owned(),
        fallback: module.to_string_lossy().into_owned(),
    };
    let input = read(&args[1], logisim_file::MAX_BYTES)?;
    let output =
        logisim_ttl_plugin::migrate::convert(std::str::from_utf8(&input)?, manifest, source)?;
    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&args[2])?
        .write_all(output.as_bytes())?;
    println!(
        "Created {}. Legacy TTL palette entries removed; component attributes, positions and wires retained.",
        args[2].display()
    );
    Ok(())
}
