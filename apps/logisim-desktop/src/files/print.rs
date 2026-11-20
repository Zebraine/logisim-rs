//! A print job owns the pixels shown in its preview. The OS chooses the printer.
use super::{Dialog, Files, Response, image::encode_png};
use eframe::egui::{self, ColorImage, Context};
use logisim_ui::{
    Language,
    dialog::{Dialog as Window, Dismiss},
    i18n::tr,
    tf,
};
use std::{path::Path, process::Command, sync::Arc};

mod job;
use job::Delivery;
pub(super) use job::Job;
pub(super) use job::Report;

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
                match Job::start(preview.original, ctx.clone(), language) {
                    Ok(job) => self.dialog = Some(Dialog::Printing(job)),
                    Err(error) => self.print_result(Err(error.to_string()).into()),
                }
            }
            (Dialog::Printing(job), Response::Cancel) => {
                job.cancel();
                self.dialog = Some(Dialog::Printing(job));
            }
            (Dialog::Printing(job), Response::Printed) => self.print_result(job.finish()),
            _ => unreachable!("unrelated print dialog response"),
        }
    }

    pub(super) fn print_result(&mut self, report: Report) {
        let (title, mut body) = match report.status {
            Ok(Delivery::Sent) => ("Print", tr("Sent to the default printer.").to_owned()),
            Ok(Delivery::Cancelled) => ("Print", tr("Print cancelled.").to_owned()),
            Err(error) => ("Print failed", error),
        };
        if let Some(warning) = report.cleanup_warning {
            body.push('\n');
            body.push_str(&tf!(
                "Could not clean up temporary print files: {}",
                warning
            ));
        }
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

#[cfg(test)]
mod tests;
