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
