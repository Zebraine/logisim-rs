use super::*;
#[test]
fn catalog_is_complete_for_its_keys_and_keeps_format_arguments_opaque() {
    assert!(KEYS.windows(2).all(|w| w[0] < w[1]));
    for (language, source) in [
        (Language::English, include_str!("../../locales/en.json")),
        (
            Language::SimplifiedChinese,
            include_str!("../../locales/zh-CN.json"),
        ),
    ] {
        let source: serde_json::Value = serde_json::from_str(source).unwrap();
        let messages = source["messages"].as_object().unwrap();
        assert_eq!(language.name(), source["name"].as_str().unwrap());
        assert_eq!(KEYS.len(), messages.len());
        assert_eq!(KEYS.len(), language.catalog().messages.len());
        for &key in KEYS {
            let translated = messages[key].as_str().unwrap();
            assert!(!translated.is_empty(), "{key}");
            assert_eq!(
                placeholder_count(key),
                placeholder_count(translated),
                "{key}"
            );
            assert_eq!(with_language(language, || tr(key).to_owned()), translated);
        }
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
