use super::*;
use crate::tests::support::Temp;
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
