use super::*;
use crate::tests::support::Temp;
use std::ffi::OsStr;

#[test]
fn commands_treat_the_entire_path_as_data_on_both_platforms() {
    let path = Path::new("/test dir/O'Brien [1]; $print.png");
    let lp = printer_command(path, false);
    assert_eq!(lp.get_program(), "lp");
    assert_eq!(lp.get_args().collect::<Vec<_>>(), [path.as_os_str()]);
    let ps = printer_command(path, true);
    assert_eq!(ps.get_program(), "powershell.exe");
    assert_eq!(
        ps.get_envs().collect::<Vec<_>>(),
        [(OsStr::new("LOGISIM_PRINT_FILE"), Some(path.as_os_str()))]
    );
    let args = ps.get_args().collect::<Vec<_>>();
    assert_eq!(&args[..3], ["-NoProfile", "-NonInteractive", "-Command"]);
    let script = args[3].to_str().unwrap();
    assert!(script.contains("-FilePath $env:LOGISIM_PRINT_FILE -Verb Print -Wait"));
    assert!(!script.contains("O'Brien"));
}

#[test]
fn only_confirmed_jobs_write_full_resolution_pngs_and_clean_up_after_errors() {
    let dir = Temp::new();
    let image = ColorImage::filled([11, 5], egui::Color32::from_rgb(4, 101, 53));
    let mut paths = Vec::new();
    for status in [Ok(()), Err("printer offline".to_owned())] {
        let result = submit(&dir.0, &image, |path| {
            paths.push(path.to_owned());
            let png = ::image::open(path).unwrap().into_rgba8();
            assert_eq!(png.dimensions(), (11, 5));
            assert!(png.pixels().all(|p| p.0 == [4, 101, 53, 255]));
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(fs::metadata(path).unwrap().permissions().mode() & 0o077, 0);
            }
            status.clone()
        });
        assert_eq!(result, status);
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 0);
    }
    assert_ne!(paths[0], paths[1]);
    assert!(submit(&dir.path("absent"), &image, |_| panic!("unexpected job")).is_err());
    let err = run(Command::new(dir.path("missing-printer-command"))).unwrap_err();
    assert!(err.contains("missing-printer-command"));
}

#[test]
fn preview_cancel_releases_only_the_preview_and_never_requests_another_capture() {
    let mut files = Files::default();
    let mut workspace = logisim_ui::Workspace::default();
    let mut simulator = logisim_sim::ProjectSimulator::default();
    let ctx = Context::default();
    let start = workspace.project().clone();
    let input = egui::RawInput {
        max_texture_side: Some(2048),
        ..Default::default()
    };
    ctx.run_ui(input.clone(), |_| {}).textures_delta.clear();
    files.request(
        logisim_ui::FileAction::Print,
        &ctx,
        &mut workspace,
        &mut simulator,
    );
    assert!(files.capturing_image() && !files.suspends_simulation());
    let capture = files.capture.take().unwrap();
    let image = ColorImage::filled([2049, 23], egui::Color32::BLACK);
    files.captured(&ctx, capture.target, Ok(image));
    let mut output = ctx.run_ui(input, |ui| {
        assert!(!files.suspends_simulation());
        let dialog = files.dialog.take().unwrap();
        let Dialog::PrintPreview(ref preview) = dialog else {
            panic!("missing preview")
        };
        assert_eq!(preview.original.size, [2049, 23]);
        assert!(preview.texture.size()[0] <= 2048);
        files.respond_to_print(dialog, Response::Cancel, ui.ctx(), Language::English);
    });
    assert!(
        !output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::Screenshot(_)))
    );
    output.textures_delta.clear();
    assert!(!files.busy() && !files.present(&workspace).dirty && workspace.recent().is_empty());
    assert_eq!(workspace.project(), &start);
}
