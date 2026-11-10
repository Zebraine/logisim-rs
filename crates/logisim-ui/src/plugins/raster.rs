//! Full-frame textures are reused across snapshots; no uploads for unchanged images.
use crate::canvas::Canvas;
use egui::{Color32, Painter, TextureHandle, TextureOptions, epaint::Mesh};
use logisim_core::{Component, Id, Point, plugin::api::raster::Image};
use std::{collections::BTreeMap, sync::Arc};
#[cfg(test)]
mod tests;
#[derive(Clone)]
struct Entry {
    image: Arc<Image>,
    texture: TextureHandle,
    touched: u64,
}
#[derive(Clone, Default)]
struct Cache {
    entries: BTreeMap<Id, Entry>,
    frame: u64,
}

fn texture(ctx: &egui::Context, id: Id, image: &Arc<Image>) -> TextureHandle {
    let key = egui::Id::unique("plugin-raster-textures");
    let frame = ctx.cumulative_frame_nr();
    let entry = ctx.data_mut(|data| {
        let cache = data.get_temp_mut_or_default::<Cache>(key);
        if cache.frame != frame {
            cache
                .entries
                .retain(|_, e| e.touched.saturating_add(1) >= frame);
            cache.frame = frame;
        }
        cache.entries.remove(&id)
    });
    let texture = match entry {
        Some(mut old) => {
            if !Arc::ptr_eq(&old.image, image) {
                old.texture.set(
                    egui::ColorImage::from_rgba_unmultiplied(
                        [image.width as usize, image.height as usize],
                        &image.pixels,
                    ),
                    TextureOptions::NEAREST,
                );
            }
            old.texture
        }
        None => ctx.load_texture(
            "plugin raster",
            egui::ColorImage::from_rgba_unmultiplied(
                [image.width as usize, image.height as usize],
                &image.pixels,
            ),
            TextureOptions::NEAREST,
        ),
    };
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<Cache>(key).entries.insert(
            id,
            Entry {
                image: Arc::clone(image),
                texture: texture.clone(),
                touched: frame,
            },
        );
    });
    texture
}
pub(super) fn draw(p: &Painter, canvas: &Canvas, c: &Component, image: &Arc<Image>, ghost: bool) {
    if !p.is_visible() {
        return;
    }
    let texture = texture(p.ctx(), c.id, image);
    let mut mesh = Mesh::with_texture(texture.id());
    let [[x0, y0], [x1, y1]] = image.bounds;
    let tint = if ghost {
        Color32::WHITE.gamma_multiply(0.45)
    } else {
        Color32::WHITE
    };
    for (xy, uv) in [
        ([x0, y0], [0., 0.]),
        ([x1, y0], [1., 0.]),
        ([x1, y1], [1., 1.]),
        ([x0, y1], [0., 1.]),
    ] {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: canvas.screen(c.position + c.facing.transform(Point::new(xy[0], xy[1]))),
            uv: uv.into(),
            color: tint,
        });
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(mesh);
}
