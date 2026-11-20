use super::*;
use crate::tests::support::Temp;
use logisim_sim::ProjectSimulator;
use logisim_ui::{FileAction, Workspace};

#[test]
fn png_export_confirms_before_capture_and_atomically_replaces_only_the_chosen_target() {
    let temp = Temp::new();
    let path = temp.path("chosen.PNG");
    let mut ws = Workspace::default();
    let mut sim = ProjectSimulator::default();
    let preferences = Preferences::at(Some(temp.path("preferences.json")), &mut ws);
    let mut files = Files::default();
    let ctx = egui::Context::default();
    fs::write(&path, b"previous image").unwrap();
    let pick = |files: &mut Files, ws: &mut Workspace, sim: &mut ProjectSimulator| {
        files.request(FileAction::ExportImage, &ctx, ws, sim);
        assert!(!files.suspends_simulation());
        let dialog = files.dialog.take().unwrap();
        files.respond_to_image(dialog, Response::Pick(path.clone()), &preferences, &ctx);
        assert!(files.image_export.is_none());
        files.dialog.take().unwrap()
    };
    let dialog = pick(&mut files, &mut ws, &mut sim);
    assert!(matches!(dialog, Dialog::ReplaceImage(_)));
    files.respond_to_image(dialog, Response::Cancel, &preferences, &ctx);
    assert!(!files.busy());
    assert_eq!(fs::read(&path).unwrap(), b"previous image");
    let dialog = pick(&mut files, &mut ws, &mut sim);
    files.respond_to_image(dialog, Response::Replace, &preferences, &ctx);
    let mut capture = files.image_export.take().unwrap();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_max(
            egui::Pos2::ZERO,
            egui::pos2(4., 2.),
        )),
        ..Default::default()
    };
    let canvas = egui::Rect::from_min_max(egui::pos2(3., 1.), egui::pos2(4., 2.));
    let mut output = ctx.run_ui(input.clone(), |ui| {
        assert!(capture.poll(ui.ctx(), canvas).is_none());
    });
    output.textures_delta.clear();
    let callback = output.viewport_output[&egui::ViewportId::ROOT]
        .commands
        .iter()
        .find_map(|cmd| {
            if let egui::ViewportCommand::Screenshot(cb) = cmd {
                Some(cb.clone())
            } else {
                None
            }
        })
        .unwrap();
    let mut frame = egui::ColorImage::filled([4, 2], egui::Color32::RED);
    frame[(3, 1)] = egui::Color32::from_rgb(0, 128, 255);
    callback.complete(std::sync::Arc::new(frame));
    ctx.run_ui(input, |ui| {
        capture
            .poll(ui.ctx(), canvas)
            .expect("readback completed")
            .unwrap();
    })
    .textures_delta
    .clear();
    let png = ::image::open(&path).unwrap().into_rgba8();
    assert_eq!(png.dimensions(), (1, 1));
    assert_eq!(png.as_raw(), &[0, 128, 255, 255]);
    assert!(ws.recent().is_empty() && !files.present(&ws).dirty);
}

#[test]
fn png_export_protects_changed_files_read_only_targets_and_recovery_directory() {
    let temp = Temp::new();
    let mut ws = Workspace::default();
    let preferences = Preferences::at(Some(temp.path("preferences.json")), &mut ws);
    let files = Files::default();
    let path = temp.path("existing.png");
    let target = files.image_target(&path, &preferences).unwrap();
    fs::write(&path, b"created after picker").unwrap();
    assert!(target.write(1, 1, &[0; 4]).is_err());
    let target = files.image_target(&path, &preferences).unwrap();
    fs::write(&path, b"modified externally").unwrap();
    assert!(target.write(1, 1, &[0; 4]).is_err());
    let target = files.image_target(&path, &preferences).unwrap();
    let writable = fs::metadata(&path).unwrap().permissions();
    let mut readonly = writable.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    assert!(target.write(1, 1, &[0; 4]).is_err());
    fs::set_permissions(&path, writable).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"modified externally");
    fs::create_dir_all(temp.path("recovery/subdir")).unwrap();
    for invalid in [
        "existing.jpg",
        "recovery/subdir/image.png",
        "recovery",
        "missing/image.png",
    ] {
        assert!(
            files
                .image_target(&temp.path(invalid), &preferences)
                .is_err()
        );
    }
}
