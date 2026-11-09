//! The host owns file access; decoded data is committed only after full validation.
use crate::simulation::Host;
use logisim_core::{Kind, MemoryContents};
use logisim_file::HexError;
use logisim_ui::{MemoryImportTarget, Workspace, i18n::tr, tf};
use std::path::Path;

fn read(path: &Path, address_bits: u8, data_bits: u8) -> Result<MemoryContents, String> {
    let bytes = crate::storage::read_file(path, "Choose a regular .hex file.")
        .map_err(|e| tr(&e).to_owned())?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| tr("The memory image is not valid UTF-8 text.").to_owned())?;
    logisim_file::parse_hex(text, address_bits, data_bits).map_err(|error| match error {
        HexError::TooLarge => tr("File exceeds the 8 MiB limit.").into(),
        HexError::Dimensions => tr("The memory dimensions are not supported.").into(),
        HexError::Header => {
            tr("Unsupported memory image format. Expected v2.0 raw on the first line.").into()
        }
        HexError::Word { line } => tf!("Line {}: invalid hexadecimal word.", line),
        HexError::Count { line } => tf!(
            "Line {}: expected a nonnegative decimal repeat count.",
            line
        ),
        HexError::Capacity { line } => tf!(
            "Line {}: the image exceeds this memory's address capacity.",
            line
        ),
    })
}

pub(crate) fn load(
    path: &Path,
    target: &MemoryImportTarget,
    workspace: &mut Workspace,
    sim: &mut impl Host,
) -> Result<(), String> {
    let c = target.component();
    let contents = read(path, c.memory.address_bits, c.bits)?;
    if !workspace.memory_import_is_current(target) {
        return Err(
            tr("The memory target changed. Open its contents editor and try again.").into(),
        );
    }
    if c.kind == Kind::Ram && !sim.replace_memory(target.path(), c.id, contents.clone()) {
        return Err(tr("The RAM simulation instance is no longer available.").into());
    }
    workspace.complete_memory_import(target, contents);
    Ok(())
}

pub(crate) fn load_plugin(
    path: &Path,
    target: &logisim_ui::PluginImportTarget,
    workspace: &mut Workspace,
) -> Result<(), String> {
    let import = target.import();
    let contents = read(path, import.address_bits, import.data_bits)?;
    if !workspace.complete_plugin_import(target, &contents) {
        return Err(tr("The plugin target changed. Select it and try again.").into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
