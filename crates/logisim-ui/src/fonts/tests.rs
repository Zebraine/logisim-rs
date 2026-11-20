use super::*;
use egui::FontFamily::{Monospace, Proportional};

#[test]
fn canvas_layout_uses_the_installed_face_for_preset_and_manual_serif() {
    let ctx = egui::Context::default();
    install_serif(&ctx);
    let mut system = Collection::new(Default::default());
    if ["DejaVu Serif", "Noto Serif", "Georgia", "Times New Roman"]
        .iter()
        .filter_map(|name| system.family_by_name(name))
        .any(|family| {
            family
                .default_font()
                .and_then(|face| face.load(None))
                .is_some()
        })
    {
        assert!(
            SYSTEM_SERIF.is_some(),
            "a readable system serif should be registered"
        );
    }
    let expected = SYSTEM_SERIF
        .as_ref()
        .map_or(Proportional, |_| SERIF.clone());
    ctx.run_ui(Default::default(), |ui| {
        if let Some(face) = SYSTEM_SERIF.as_ref() {
            ui.fonts(|fonts| {
                assert_eq!(fonts.definitions().families[&*SERIF], [face.name.as_str()])
            });
        }
        for (name, family) in [
            (crate::FontPreset::Serif.family(), expected.clone()),
            ("serif", expected.clone()),
            ("Dialog", Proportional),
            ("Monospaced", Monospace),
        ] {
            let font = ArtworkFont {
                family: name.into(),
                ..Default::default()
            };
            let text = Text {
                text: "Fine Serif",
                font: &font,
                color: Color32::BLACK,
                zoom: 1.,
                angle: 0.,
            };
            assert_eq!(
                text.layout(ui.painter()).job.sections[0]
                    .format
                    .font_id
                    .family,
                family
            );
        }
    })
    .textures_delta
    .clear();
}

#[test]
fn without_a_serif_face_labels_use_proportional() {
    let mut empty = Collection::new(fontique::CollectionOptions {
        system_fonts: false,
        ..Default::default()
    });
    assert!(resolve_serif(&mut empty).is_none());
    egui::Context::default()
        .run_ui(Default::default(), |ui| {
            let font = ArtworkFont {
                family: "Serif".into(),
                ..Default::default()
            };
            let text = Text {
                text: "Hello",
                font: &font,
                color: Color32::BLACK,
                zoom: 1.,
                angle: 0.,
            };
            assert_eq!(
                text.layout(ui.painter()).job.sections[0]
                    .format
                    .font_id
                    .family,
                Proportional
            );
        })
        .textures_delta
        .clear();
}
