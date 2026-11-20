use super::*;
use crate::background::Cancellation;
use crate::tests::support::Temp;
use job::{run, submit};
use std::{ffi::OsStr, fs, time::Duration};

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
    for status in [Ok(Delivery::Sent), Err("printer offline".to_owned())] {
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
            status
                .as_ref()
                .map(|_| Delivery::Sent)
                .map_err(Clone::clone)
        });
        assert_eq!(result.status, status);
        assert!(result.cleanup_warning.is_none());
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 0);
    }
    assert_ne!(paths[0], paths[1]);
    assert!(
        submit(&dir.path("absent"), &image, |_| panic!("unexpected job"))
            .status
            .is_err()
    );
    let err = run(
        Command::new(dir.path("missing-printer-command")),
        &dir.path("canvas.png"),
        &Cancellation::default(),
        Duration::from_secs(1),
    )
    .unwrap_err();
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

#[test]
fn cleanup_warning_keeps_a_successful_submission_in_the_success_dialog() {
    let dir = Temp::new();
    let image = ColorImage::filled([1, 1], egui::Color32::WHITE);
    let report = submit(&dir.0, &image, |path| {
        // Simulate loss of access to the original spool path after the OS accepted it.
        fs::rename(path.parent().unwrap(), dir.path("moved")).unwrap();
        Ok(Delivery::Sent)
    });
    assert_eq!(report.status, Ok(Delivery::Sent));
    assert!(report.cleanup_warning.is_some());
    let mut files = Files::default();
    files.print_result(report);
    let Some(Dialog::ImageMessage { title, body }) = files.dialog else {
        panic!("missing print outcome")
    };
    assert_eq!(title, "Print");
    assert!(body.contains("Sent to the default printer.") && body.contains("clean up"));
}

#[cfg(unix)]
#[test]
fn command_polling_reports_success_delayed_success_and_noisy_failures() {
    let dir = Temp::new();
    let image = ColorImage::filled([1, 1], egui::Color32::WHITE);
    for (script, succeeds) in [
        ("exit 0", true),
        ("sleep 0.12", true),
        (
            "printf 'offline\\n' >&2; head -c 1048576 /dev/zero >&2; exit 3",
            false,
        ),
    ] {
        let report = submit(&dir.0, &image, |path| {
            let mut command = Command::new("sh");
            command.args(["-c", script]);
            run(
                command,
                path,
                &Cancellation::default(),
                Duration::from_secs(5),
            )
        });
        if succeeds {
            assert_eq!(report.status, Ok(Delivery::Sent));
        } else {
            let error = report.status.unwrap_err();
            assert!(error.contains("sh") && error.contains('3') && error.contains("offline"));
            assert!(error.len() < 8300);
        }
        assert!(report.cleanup_warning.is_none());
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 0);
    }
}

#[cfg(unix)]
#[test]
fn cancellation_and_timeout_reap_the_print_command_and_remove_its_spool() {
    use std::{thread, time::Instant};
    for cancel in [false, true] {
        let dir = Temp::new();
        let pid_path = dir.path("pid");
        let flag = Cancellation::default();
        let timeout = if cancel {
            Duration::from_secs(5)
        } else {
            Duration::from_millis(300)
        };
        let report = thread::scope(|scope| {
            let worker = scope.spawn(|| {
                let image = ColorImage::filled([2, 2], egui::Color32::BLACK);
                submit(&dir.0, &image, |path| {
                    let mut cmd = Command::new("sh");
                    cmd.args([
                        "-c",
                        "echo $$ > \"$1\"; echo waiting >&2; exec sleep 10",
                        "print",
                    ])
                    .arg(&pid_path);
                    run(cmd, path, &flag, timeout)
                })
            });
            if cancel {
                let start = Instant::now();
                while !pid_path.is_file() && start.elapsed() < Duration::from_secs(3) {
                    thread::sleep(Duration::from_millis(10));
                }
                flag.cancel();
            }
            worker.join().unwrap()
        });
        if cancel {
            assert_eq!(report.status, Ok(Delivery::Cancelled));
        } else {
            let error = report.status.unwrap_err();
            assert!(
                error.contains("sh timed out") && error.contains("waiting"),
                "{error}"
            );
        }
        assert!(report.cleanup_warning.is_none());
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
        let pid = fs::read_to_string(&pid_path)
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();
        #[cfg(target_os = "linux")]
        assert!(
            !Path::new("/proc").join(pid.to_string()).exists(),
            "child {pid} was not reaped"
        );
        #[cfg(not(target_os = "linux"))]
        assert!(pid > 0);
    }
}
