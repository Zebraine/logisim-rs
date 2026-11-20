//! Read back a clean viewport frame after the picker/confirmation has disappeared.
use super::image::ImageTarget;
use eframe::egui::{ColorImage, Context, Rect};
use std::{
    sync::{
        Arc,
        mpsc::{self, Receiver, TryRecvError},
    },
    time::{Duration, Instant},
};

const READBACK_TIMEOUT: Duration = Duration::from_secs(5);
const UNAVAILABLE: &str = "The renderer could not capture the canvas.";
const RESIZED: &str = "The viewport changed during capture. Export the image again.";

#[derive(Clone, Copy, PartialEq)]
struct Geometry {
    viewport: Rect,
    canvas: Rect,
    pixels_per_point: f32,
}

impl Geometry {
    fn crop(self, image: &ColorImage) -> Result<ColorImage, String> {
        let scale = self.pixels_per_point;
        if !self.viewport.is_finite()
            || !self.canvas.is_finite()
            || !scale.is_finite()
            || scale <= 0.
        {
            return Err(UNAVAILABLE.into());
        }
        let size = self.viewport.size() * scale;
        if (size.x - image.width() as f32).abs() > 1. || (size.y - image.height() as f32).abs() > 1.
        {
            return Err(RESIZED.into());
        }
        let rect = self.canvas.intersect(self.viewport);
        if !rect.is_positive() {
            return Err(UNAVAILABLE.into());
        }
        // Match the renderer's physical scissor edges, measured from the viewport origin.
        let from = (rect.min - self.viewport.min) * scale;
        let to = (rect.max - self.viewport.min) * scale;
        let x = (from.x.round() as usize).min(image.width());
        let y = (from.y.round() as usize).min(image.height());
        let right = (to.x.round() as usize).min(image.width());
        let bottom = (to.y.round() as usize).min(image.height());
        if x >= right || y >= bottom {
            return Err(UNAVAILABLE.into());
        }
        Ok(image.region_by_pixels([x, y], [right - x, bottom - y]))
    }
}

struct Readback {
    geometry: Geometry,
    pixels: Receiver<Arc<ColorImage>>,
    started: Instant,
}
pub(super) struct Capture {
    pub(super) target: ImageTarget,
    readback: Option<Readback>,
}

impl Capture {
    pub(super) fn new(target: ImageTarget) -> Self {
        Self {
            target,
            readback: None,
        }
    }

    pub(super) fn poll(&mut self, ctx: &Context, canvas: Rect) -> Option<Result<(), String>> {
        let geometry = Geometry {
            canvas,
            viewport: ctx.input(|i| i.viewport_rect()),
            pixels_per_point: ctx.pixels_per_point(),
        };
        let Some(readback) = &self.readback else {
            let (sender, pixels) = mpsc::channel();
            let wake = ctx.clone();
            ctx.request_screenshot(move |image| {
                let _ = sender.send(image);
                wake.request_repaint();
            });
            self.readback = Some(Readback {
                geometry,
                pixels,
                started: Instant::now(),
            });
            ctx.request_repaint_after(READBACK_TIMEOUT);
            return None;
        };
        if readback.geometry != geometry {
            return Some(Err(RESIZED.into()));
        }
        match readback.pixels.try_recv() {
            Ok(frame) => Some(geometry.crop(&frame).and_then(|image| {
                let rgba: Vec<_> = image
                    .pixels
                    .iter()
                    .flat_map(|p| p.to_srgba_unmultiplied())
                    .collect();
                self.target
                    .write(image.width() as u32, image.height() as u32, &rgba)
            })),
            Err(TryRecvError::Empty) if readback.started.elapsed() < READBACK_TIMEOUT => None,
            Err(_) => Some(Err(UNAVAILABLE.into())),
        }
    }
}

#[cfg(test)]
mod tests;
