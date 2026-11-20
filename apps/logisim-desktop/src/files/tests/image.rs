use super::*;
use crate::tests::support::Temp;
use std::fs;

#[test]
fn dropped_renderer_callback_reports_error_without_replacing_existing_file() {
    let temp = Temp::new();
    let path = temp.path("chosen.png");
    fs::write(&path, b"original").unwrap();
    let mut h = Harness::default();
    h.files
        .request(FileAction::ExportImage, &h.ctx, &mut h.ws, &mut h.sim);
    let picker = h.files.dialog.take().unwrap();
    h.files
        .respond_to_image(picker, Response::Pick(path.clone()), &h.preferences, &h.ctx);
    let confirm = h.files.dialog.take().unwrap();
    h.files
        .respond_to_image(confirm, Response::Replace, &h.preferences, &h.ctx);
    // The headless harness drops viewport commands without completing renderer callbacks.
    for _ in 0..4 {
        h.frame(vec![]);
    }
    assert!(
        matches!(h.files.dialog, Some(Dialog::ImageMessage{title:"Image export failed",ref body})
        if body.contains("renderer") && body.contains("chosen.png"))
    );
    assert_eq!(fs::read(&path).unwrap(), b"original");
    assert!(!h.files.present(&h.ws).dirty);
}

fn click_in(h: &mut Harness, size: egui::Vec2, label: &str) {
    let labels = h.frame_at(vec![], size);
    let pos = labels
        .iter()
        .rev()
        .find(|(s, _)| s == label)
        .unwrap_or_else(|| panic!("missing {label}"))
        .1;
    for pressed in [true, false] {
        h.frame_at(
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    pressed,
                    button: PointerButton::Primary,
                    modifiers: Modifiers::NONE,
                },
            ],
            size,
        );
    }
}

#[test]
fn png_picker_filters_extends_and_rejects_names_and_localizes_the_controls() {
    use logisim_ui::{Language, ThemeMode, i18n::with_language};
    let dir = Temp::new();
    fs::write(dir.path("visible.PNG"), []).unwrap();
    fs::write(dir.path("hidden.circ"), []).unwrap();
    for language in Language::ALL {
        for theme in [ThemeMode::Light, ThemeMode::Dark] {
            for size in [egui::vec2(720., 480.), egui::vec2(1200., 820.)] {
                let mut h = Harness::default();
                logisim_ui::configure(&h.ctx);
                h.ws.apply_settings(logisim_ui::Settings {
                    language,
                    theme,
                    ..Default::default()
                });
                h.files.directory.clone_from(&dir.0);
                for _ in 0..3 {
                    h.frame_at(vec![], size);
                }
                let t = |key| with_language(language, || tr(key));
                click_in(&mut h, size, t("File"));
                click_in(&mut h, size, t("Export Image…"));
                for _ in 0..3 {
                    h.frame_at(vec![], size);
                }
                let labels = h.frame_at(vec![], size);
                for label in ["visible.PNG", "circuit.png", t("Cancel"), t("Save")] {
                    let pos = labels.iter().rev().find(|(s, _)| s == label).unwrap().1;
                    assert!(Rect::from_min_size(Pos2::ZERO, size).contains(pos));
                }
                assert!(!labels.iter().any(|(s, _)| s == "hidden.circ"));
                let key = |key, modifiers| Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                };
                h.frame_at(
                    vec![
                        key(egui::Key::A, Modifiers::COMMAND),
                        Event::Text("wrong.jpg".into()),
                    ],
                    size,
                );
                click_in(&mut h, size, t("Save"));
                assert!(
                    h.frame_at(vec![], size)
                        .iter()
                        .any(|(s, _)| s == t("Choose a .png image file."))
                );
                assert!(!h.files.capturing_image());
                click_in(&mut h, size, t("Cancel"));
                assert!(!h.files.busy() && !h.files.present(&h.ws).dirty);
                h.files
                    .request(FileAction::ExportImage, &h.ctx, &mut h.ws, &mut h.sim);
                for _ in 0..3 {
                    h.frame_at(vec![], size);
                }
                h.frame_at(
                    vec![
                        key(egui::Key::A, Modifiers::COMMAND),
                        Event::Text("added".into()),
                    ],
                    size,
                );
                click_in(&mut h, size, t("Save"));
                assert_eq!(
                    h.files.image_export.as_ref().unwrap().target.path,
                    dir.path("added.png")
                );
            }
        }
    }
}
