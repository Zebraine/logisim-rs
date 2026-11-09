use logisim_plugin_api::presentation::Presentation;
pub fn metadata() -> Presentation {
    Presentation {
        translations: [(
            "zh-CN".into(),
            serde_json::from_str(include_str!("../locales/zh-CN.json"))
                .expect("bundled translations"),
        )]
        .into(),
        imports: vec![logisim_plugin_api::presentation::MemoryImport {
            tool: "register-file".into(),
            action: "write".into(),
            label: "Import .hex…".into(),
            address_bits: 5,
            data_bits: 32,
            argument_prefix: "image ".into(),
        }],
    }
}
