//! The host owns file access; decoded data is committed only after full validation.
use logisim_core::{Kind, MemoryContents};
use logisim_file::HexError;
use logisim_sim::ProjectSimulator;
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
    sim: &mut ProjectSimulator,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_tests::Temp;
    use logisim_ui::{Language, i18n::with_language};

    #[test]
    fn reading_images_is_bounded_translated_and_never_writes_the_source() {
        let dir = Temp::new();
        let path = dir.path("memory.hex");
        let source = b"v2.0 raw\n12 2*ab";
        std::fs::write(&path, source).unwrap();
        assert_eq!(read(&path, 4, 8).unwrap().get(2), 0xab);
        assert_eq!(std::fs::read(&path).unwrap(), source);
        for language in Language::ALL {
            with_language(language, || {
                assert!(read(&dir.path("absent.hex"), 4, 8).is_err());
                assert_eq!(
                    read(&dir.0, 4, 8).unwrap_err(),
                    tr("Choose a regular .hex file.")
                );
                std::fs::write(&path, [0xff]).unwrap();
                assert_eq!(
                    read(&path, 4, 8).unwrap_err(),
                    tr("The memory image is not valid UTF-8 text.")
                );
                std::fs::write(&path, b"v2.0 raw\n12 invalid").unwrap();
                assert_eq!(
                    read(&path, 4, 8).unwrap_err(),
                    tf!("Line {}: invalid hexadecimal word.", 2)
                );
                let original = std::fs::read(&path).unwrap();
                assert!(read(&path, 4, 8).is_err());
                assert_eq!(std::fs::read(&path).unwrap(), original);
            });
        }
        std::fs::File::create(&path)
            .unwrap()
            .set_len(logisim_file::MAX_BYTES as u64 + 1)
            .unwrap();
        assert_eq!(
            read(&path, 4, 8).unwrap_err(),
            "File exceeds the 8 MiB limit."
        );
    }
}
