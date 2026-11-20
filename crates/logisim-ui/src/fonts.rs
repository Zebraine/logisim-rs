use egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily};
use egui::{Align2, Color32, Painter, Pos2, Vec2};
use fontique::{Collection, GenericFamily};
use logisim_core::ArtworkFont;
use read_fonts::{FontRef, TableProvider};
use std::sync::LazyLock;

static SERIF: LazyLock<egui::FontFamily> = LazyLock::new(|| egui::FontFamily::Name("Serif".into()));
struct SystemFonts {
    available: bool,
    serif: Option<FontInsert>,
}

static SYSTEM_FONTS: LazyLock<SystemFonts> = LazyLock::new(|| {
    let mut collection = system_collection();
    let serif = collection.as_mut().and_then(resolve_serif);
    if serif.is_none() {
        eprintln!(
            "warning: no usable system serif font found; canvas Serif labels will use Proportional."
        );
    }
    SystemFonts {
        available: collection.is_some(),
        serif,
    }
});

// fontique 0.11.1 unwraps Fontconfig's NoMatch when *all* system fonts are absent.
// Avoid starting either the document resolver or eframe's glyph provider in that case.
fn system_collection() -> Option<Collection> {
    #[cfg(target_os = "linux")]
    {
        if std::process::Command::new("fc-match")
            .arg("--format=%{file}")
            .arg("serif")
            .output()
            .is_ok_and(|output| output.stdout.is_empty())
        {
            return None;
        }
        // Minimal installations can have libfontconfig without the fc-match utility.
        std::panic::catch_unwind(Collection::default).ok()
    }
    #[cfg(not(target_os = "linux"))]
    Some(Collection::default())
}

/// Whether the desktop can ask the platform for missing glyphs.
pub fn system_fonts_available() -> bool {
    SYSTEM_FONTS.available
}

fn resolve_serif(collection: &mut Collection) -> Option<FontInsert> {
    let known = [
        "Noto Serif",
        "DejaVu Serif",
        "Liberation Serif",
        "Times New Roman",
        "Times",
        "Georgia",
    ];
    let mut candidates: Vec<_> = collection.generic_families(GenericFamily::Serif).collect();
    for name in known {
        if let Some(id) = collection.family_id(name)
            && !candidates.contains(&id)
        {
            candidates.push(id);
        }
    }
    candidates.into_iter().find_map(|id| {
        let family = collection.family(id)?;
        let face = family.default_font()?;
        let bytes = face.load(None)?;
        // Fontconfig's generic list can end in sans fallbacks. Only accept a serif face.
        let font = FontRef::from_index(bytes.as_ref(), face.index()).ok()?;
        let classified = font.os2().is_ok_and(|os2| {
            matches!(os2.s_family_class() >> 8, 1..=5 | 7)
                || matches!(os2.panose_10(), [2, 2..=10, ..])
        });
        if !classified && !known.contains(&family.name()) {
            return None;
        }
        Some(FontInsert {
            name: format!("document-serif:{}", family.name()),
            data: egui::FontData::from_blob(bytes.into_raw_parts().0, face.index()),
            families: vec![InsertFontFamily {
                family: SERIF.clone(),
                priority: FontPriority::Highest,
            }],
        })
    })
}

pub(crate) fn install_serif(ctx: &egui::Context) {
    if let Some(font) = &SYSTEM_FONTS.serif {
        ctx.add_font(font.clone());
    }
}

pub(crate) struct Text<'a> {
    pub text: &'a str,
    pub font: &'a ArtworkFont,
    pub color: Color32,
    pub zoom: f32,
    pub angle: f32,
}
impl Text<'_> {
    pub fn layout(&self, p: &Painter) -> std::sync::Arc<egui::Galley> {
        let family = if self.font.family.eq_ignore_ascii_case("serif")
            && p.fonts(|fonts| fonts.definitions().families.contains_key(&*SERIF))
        {
            SERIF.clone()
        } else if self.font.family.to_ascii_lowercase().contains("mono") {
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
        p.layout_job(job)
    }
    pub fn paint(&self, p: &Painter, at: Pos2, align: Align2) {
        let galley = self.layout(p);
        let origin = align.anchor_size(at, galley.size()).min;
        let shape = egui::epaint::TextShape::new(origin, galley, self.color)
            .with_angle_and_anchor(self.angle, align);
        self.paint_shape(p, shape);
    }
    /// Paint a pre-laid-out run from an origin computed by the caller (e.g. an SVG baseline).
    pub fn paint_galley(&self, p: &Painter, origin: Pos2, galley: std::sync::Arc<egui::Galley>) {
        self.paint_shape(
            p,
            egui::epaint::TextShape::new(origin, galley, self.color).with_angle(self.angle),
        );
    }
    fn paint_shape(&self, p: &Painter, shape: egui::epaint::TextShape) {
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

#[cfg(test)]
mod tests;
