use logisim_plugin_api::presentation::Presentation;
pub fn metadata() -> Presentation {
    Presentation {
        translations: [(
            "zh-CN".into(),
            serde_json::from_str(include_str!("../locales/zh-CN.json"))
                .expect("bundled translations"),
        )]
        .into(),
        imports: vec![],
    }
}
