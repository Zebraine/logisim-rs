use egui::{Align2, Color32, Painter, Pos2, Vec2};
use logisim_core::ArtworkFont;

pub(crate) struct Text<'a> {
    pub text: &'a str,
    pub font: &'a ArtworkFont,
    pub color: Color32,
    pub zoom: f32,
    pub angle: f32,
}
impl Text<'_> {
    pub fn paint(&self, p: &Painter, at: Pos2, align: Align2) {
        let family = if self.font.family.to_ascii_lowercase().contains("mono") {
            egui::FontFamily::Monospace
        } else {
            egui::FontFamily::Proportional
        };
        let mut job = egui::text::LayoutJob::default();
        job.append(
            self.text,
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::new(f32::from(self.font.size) * self.zoom, family),
                color: self.color,
                italics: self.font.italic,
                ..Default::default()
            },
        );
        let galley = p.layout_job(job);
        let origin = align.anchor_size(at, galley.size()).min;
        let shape = egui::epaint::TextShape::new(origin, galley, self.color)
            .with_angle_and_anchor(self.angle, align);
        p.add(shape.clone());
        if self.font.bold {
            let mut second = shape;
            second.pos +=
                egui::emath::Rot2::from_angle(self.angle) * Vec2::new(self.zoom * 0.5, 0.0);
            p.add(second);
        }
    }
}

/// Grid rows shared by component and circuit properties. Returns active text/drag editing.
pub(crate) fn properties(ui: &mut egui::Ui, font: &mut ArtworkFont) -> bool {
    use crate::i18n::tr;
    ui.label(tr("Font family"));
    let family = ui.text_edit_singleline(&mut font.family);
    ui.end_row();
    ui.label(tr("Font size"));
    let size = ui.add(egui::DragValue::new(&mut font.size).range(1..=255));
    ui.end_row();
    ui.label(tr("Style"));
    ui.horizontal(|ui| {
        ui.checkbox(&mut font.bold, tr("Bold"));
        ui.checkbox(&mut font.italic, tr("Italic"));
    });
    ui.end_row();
    family.has_focus() || size.has_focus() || size.dragged()
}
