//! A print job owns the pixels shown in its preview. The OS chooses the printer.
use super::{Dialog, Files, Response, image::encode_png};
use eframe::egui::{self, ColorImage, Context};
use logisim_ui::{
    Language,
    dialog::{Dialog as Window, Dismiss},
    i18n::{tr, with_language},
    tf,
};
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

pub(super) struct Preview {
    original: Arc<ColorImage>,
    texture: egui::TextureHandle,
}

impl Preview {
    pub(super) fn new(ctx: &Context, image: ColorImage) -> Self {
        let original = Arc::new(image);
        let max_side = ctx.input(|i| i.max_texture_side);
        let step = original.width().max(original.height()).div_ceil(max_side);
        let display = if step == 1 {
            Arc::clone(&original)
        } else {
            let size = original.size.map(|n| n.div_ceil(step));
            let pixels = original
                .pixels
                .chunks_exact(original.width())
                .step_by(step)
                .flat_map(|row| row.iter().step_by(step).copied())
                .collect();
            Arc::new(ColorImage::new(size, pixels))
        };
        let texture = ctx.load_texture("print-preview", display, egui::TextureOptions::LINEAR);
        Self { original, texture }
    }

    pub(super) fn show(&self, ctx: &Context, response: &mut Response) -> Dismiss {
        Window::standard("print-preview", ctx).show(ctx, tr("Print Preview"), |dialog| {
            let height = dialog.body_height();
            dialog.body(|ui| {
                let area = egui::vec2(ui.available_width(), height);
                let original =
                    egui::vec2(self.original.width() as f32, self.original.height() as f32);
                let scale = (area.x / original.x).min(area.y / original.y);
                ui.allocate_ui_with_layout(
                    area,
                    egui::Layout::centered_and_justified(egui::Direction::TopDown),
                    |ui| {
                        ui.add(egui::Image::new(&self.texture).fit_to_exact_size(original * scale));
                    },
                );
            });
            dialog.footer("Cancel", |ui| {
                if ui.button(tr("Print")).clicked() {
                    *response = Response::Print;
                }
            });
        })
    }
}

impl Files {
    pub(super) fn respond_to_print(
        &mut self,
        dialog: Dialog,
        response: Response,
        ctx: &Context,
        language: Language,
    ) {
        match (dialog, response) {
            (dialog, Response::Wait) => self.dialog = Some(dialog),
            (Dialog::PrintPreview(_), Response::Cancel) => {}
            (Dialog::PrintPreview(preview), Response::Print) => {
                let wake = ctx.clone();
                match std::thread::Builder::new()
                    .name("print-canvas".into())
                    .spawn(move || {
                        let result = submit(&std::env::temp_dir(), &preview.original, |path| {
                            with_language(language, || run(printer_command(path, cfg!(windows))))
                        });
                        wake.request_repaint();
                        result
                    }) {
                    Ok(job) => self.dialog = Some(Dialog::Printing(job)),
                    Err(error) => self.print_result(Err(error.to_string())),
                }
            }
            (Dialog::Printing(job), Response::Printed) => {
                self.print_result(
                    job.join()
                        .unwrap_or_else(|_| Err(tr("The print worker stopped.").into())),
                );
            }
            _ => unreachable!("unrelated print dialog response"),
        }
    }

    pub(super) fn print_result(&mut self, result: Result<(), String>) {
        let (title, body) = match result {
            Ok(()) => ("Print", tr("Sent to the default printer.").to_owned()),
            Err(error) => ("Print failed", error),
        };
        self.dialog = Some(Dialog::ImageMessage { title, body });
    }
}

fn printer_command(path: &Path, windows: bool) -> Command {
    if windows {
        let mut command = Command::new("powershell.exe");
        // Pass the path as data. -Wait keeps the PNG alive while its print handler reads it.
        command.env("LOGISIM_PRINT_FILE", path).args(["-NoProfile", "-NonInteractive", "-Command",
            "$ErrorActionPreference = 'Stop'; $p = Start-Process -FilePath $env:LOGISIM_PRINT_FILE -Verb Print -Wait -PassThru; exit $p.ExitCode"]);
        command
    } else {
        let mut command = Command::new("lp");
        command.arg(path);
        command
    }
}

fn run(mut command: Command) -> Result<(), String> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command.output().map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            tf!("The system print command {} was not found.", program)
        } else {
            format!("{program}: {error}")
        }
    })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{program} ({})\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn temporary_png(directory: &Path) -> io::Result<(PathBuf, File)> {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    loop {
        let id = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = directory.join(format!("logisim-print-{}-{id}.png", std::process::id()));
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
}

fn submit(
    directory: &Path,
    image: &ColorImage,
    send: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    let (path, file) =
        temporary_png(directory).map_err(|e| format!("{}: {e}", directory.display()))?;
    let result = encode_png(file, image).and_then(|()| send(&path));
    if let Err(error) = fs::remove_file(&path) {
        let cleanup = format!("{}: {error}", path.display());
        return Err(match result {
            Ok(()) => cleanup,
            Err(cause) => format!("{cause}\n{cleanup}"),
        });
    }
    result
}

#[cfg(test)]
mod tests;
