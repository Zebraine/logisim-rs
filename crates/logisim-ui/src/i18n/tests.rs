use super::*;
#[test]
fn catalog_is_complete_for_its_keys_and_keeps_format_arguments_opaque() {
    assert!(MESSAGES.windows(2).all(|w| w[0].0 < w[1].0));
    for &(en, zh) in MESSAGES {
        assert!(!zh.is_empty(), "{en}");
        assert_eq!(en.matches("{}").count(), zh.matches("{}").count(), "{en}");
        assert_eq!(with_language(Language::English, || tr(en).to_owned()), en);
        assert_eq!(
            with_language(Language::SimplifiedChinese, || tr(en).to_owned()),
            zh
        );
    }
    with_language(Language::SimplifiedChinese, || {
        assert_eq!(tf!("Circuit: {}", "File {} 电路"), "电路：File {} 电路");
        assert_eq!(
            tf!("{} components · {} wires", 3, 12),
            "3 个元件 · 12 条导线"
        );
        assert_eq!(
            tr("An unknown extension label"),
            "An unknown extension label"
        );
    });
}
#[test]
fn language_scopes_restore_after_nesting_panics_and_threads() {
    with_language(Language::SimplifiedChinese, || {
        assert_eq!(tr("File"), "文件");
        with_language(Language::English, || assert_eq!(tr("File"), "File"));
        assert_eq!(tr("File"), "文件");
        std::thread::spawn(|| assert_eq!(tr("File"), "File"))
            .join()
            .unwrap();
    });
    let _ = std::panic::catch_unwind(|| {
        with_language(Language::SimplifiedChinese, || panic!("scope test"))
    });
    assert_eq!(tr("File"), "File");
}
