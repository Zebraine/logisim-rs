//! Shared desktop chrome. Document artwork supplies its own colors and fonts.
use egui::{Color32, Context, FontId, Response, Stroke, StrokeKind, TextStyle, Ui, vec2};

mod chrome;
pub use chrome::{Accent, Density, ThemeMode};
pub(crate) use chrome::{Metrics, apply_chrome, colors, metrics, palette};
const fn rgb(hex: u32) -> Color32 {
    let [_, r, g, b] = hex.to_be_bytes();
    Color32::from_rgb(r, g, b)
}

mod signal;
pub use signal::SignalPalette;
pub(crate) use signal::{active_palette, signal, signal_text};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Palette {
    pub mode: ThemeMode,
    pub canvas: Color32,
    pub surface: Color32,
    pub chrome: Color32,
    pub sunken: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub ink: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
}

impl ThemeMode {
    pub(crate) const fn palette(self) -> &'static Palette {
        match self {
            Self::Light => &LIGHT,
            Self::Dark => &DARK,
        }
    }
}

const LIGHT: Palette = Palette {
    mode: ThemeMode::Light,
    canvas: rgb(0xFFFFFF),
    surface: rgb(0xFFFFFF),
    chrome: rgb(0xF6F8FB),
    sunken: rgb(0xEEF2F7),
    border: rgb(0xE2E8F0),
    border_strong: rgb(0xCBD5E1),
    ink: rgb(0x0F172A),
    muted: rgb(0x5B6B7F),
    faint: rgb(0x94A3B8),
    success: rgb(0x15803D),
    warning: rgb(0xB45309),
    danger: rgb(0xDC2626),
};

const DARK: Palette = Palette {
    mode: ThemeMode::Dark,
    canvas: rgb(0x0B1220),
    surface: rgb(0x111827),
    chrome: rgb(0x0F172A),
    sunken: rgb(0x1E293B),
    border: rgb(0x334155),
    border_strong: rgb(0x475569),
    ink: rgb(0xE2E8F0),
    muted: rgb(0x94A3B8),
    faint: rgb(0x64748B),
    success: rgb(0x4ADE80),
    warning: rgb(0xFBBF24),
    danger: rgb(0xF87171),
};

fn contrast(a: Color32, b: Color32) -> f32 {
    let luminance = |c| {
        let rgb = egui::Rgba::from(c);
        0.2126 * rgb.r() + 0.7152 * rgb.g() + 0.0722 * rgb.b()
    };
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// A one-device-pixel edge, independent of camera zoom and UI scale.
#[derive(Clone, Copy)]
pub(crate) struct ArtworkContrast {
    canvas: Color32,
    pub edge: Stroke,
}

impl ArtworkContrast {
    pub fn new(paint: &Palette, pixels_per_point: f32, enabled: bool) -> Option<Self> {
        (enabled && paint.mode == ThemeMode::Dark).then(|| Self {
            canvas: paint.canvas,
            edge: Stroke::new(
                pixels_per_point.recip(),
                paint.border_strong.lerp_to_gamma(paint.ink, 0.16),
            ),
        })
    }

    pub fn low_contrast(self, color: Color32) -> bool {
        contrast(color, self.canvas) < 3.0
    }
}

impl Palette {
    pub(crate) fn grid(&self) -> Color32 {
        match self.mode {
            ThemeMode::Light => self.border_strong,
            ThemeMode::Dark => self.border,
        }
    }

    pub(crate) fn button_shadow(&self) -> Color32 {
        match self.mode {
            ThemeMode::Light => Color32::from_gray(140),
            ThemeMode::Dark => self.border_strong,
        }
    }

    pub(crate) fn component_ink(&self, rgb: [u8; 3], alpha: u8) -> Color32 {
        let [r, g, b] = match (self.mode, rgb) {
            (ThemeMode::Dark, [51, 65, 85] | [0, 0, 0]) => {
                [self.ink.r(), self.ink.g(), self.ink.b()]
            }
            _ => rgb,
        };
        Color32::from_rgba_unmultiplied(r, g, b, alpha)
    }
}

pub(crate) const ANIMATION_SECONDS: f32 = 0.12;

/// Install the default workbench style before opening a workspace or host dialog.
pub fn configure(ctx: &Context) {
    crate::fonts::install_serif(ctx);
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
    apply_chrome(ctx, ThemeMode::Light, Density::default(), Accent::default());
}

fn style_chrome(
    style: &mut egui::Style,
    m: chrome::Metrics,
    colors: chrome::AccentColors,
    p: &Palette,
) {
    style.text_styles.extend([
        (TextStyle::Body, FontId::proportional(13.0)),
        (TextStyle::Button, FontId::proportional(13.0)),
        (TextStyle::Small, FontId::proportional(12.0)),
        (TextStyle::Heading, FontId::proportional(16.0)),
        (TextStyle::Monospace, FontId::monospace(12.0)),
    ]);
    style.spacing.window_margin = 16.into();
    style.spacing.menu_margin = 4.into();
    style.spacing.scroll.bar_width = 6.0;
    style.animation_time = ANIMATION_SECONDS;
    let v = &mut style.visuals;
    v.panel_fill = p.chrome;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0, p.border_strong);
    v.window_corner_radius = 8.into();
    v.menu_corner_radius = 8.into();
    v.window_highlight_topmost = false;
    v.faint_bg_color = p.sunken;
    v.extreme_bg_color = p.surface;
    v.text_edit_bg_color = Some(p.surface);
    if p.mode == ThemeMode::Dark {
        v.code_bg_color = p.sunken;
        v.text_cursor.stroke.color = colors.base;
        v.ime_composition.active_underline_stroke.color = colors.base;
        v.ime_composition.inactive_underline_stroke.color = p.muted;
    }
    v.weak_text_color = Some(p.muted);
    v.warn_fg_color = p.warning;
    v.error_fg_color = p.danger;
    v.disabled_alpha = 0.64;
    style.spacing.item_spacing = vec2(4., f32::from(m.gap));
    style.spacing.button_padding = vec2(f32::from(m.card_margin), f32::from(m.gap));
    style.spacing.interact_size.y = m.control;
    v.hyperlink_color = colors.base;
    v.selection.bg_fill = colors.selected;
    // egui also uses this stroke for text on selected tabs and list items.
    v.selection.stroke = Stroke::new(2.0, colors.hover);
    for (widget, fill, border, ink) in [
        (&mut v.widgets.noninteractive, p.chrome, p.border, p.ink),
        (&mut v.widgets.inactive, p.surface, p.border_strong, p.ink),
        (
            &mut v.widgets.hovered,
            colors.soft,
            p.border_strong,
            colors.hover,
        ),
        (
            &mut v.widgets.open,
            colors.selected,
            colors.base,
            colors.hover,
        ),
        (&mut v.widgets.active, colors.selected, colors.base, p.ink),
    ] {
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.bg_stroke = Stroke::new(1.0, border);
        widget.fg_stroke = Stroke::new(1.5, ink);
        widget.corner_radius = 4.into();
        widget.expansion = 0.0;
    }
    v.widgets.active.bg_stroke.width = 2.0;
}

pub(crate) fn focus_ring(ui: &Ui, response: &Response) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect,
            4,
            Stroke::new(2.0, colors(ui.ctx()).base),
            StrokeKind::Inside,
        );
    }
}

pub(crate) fn section(ui: &mut Ui, label: &str) -> Response {
    ui.label(
        egui::RichText::new(label.to_uppercase())
            .size(12.0)
            .extra_letter_spacing(0.4)
            .strong()
            .color(palette(ui.ctx()).muted),
    )
}

pub(crate) fn chip(ui: &mut Ui, label: &str) -> Response {
    egui::Frame::NONE
        .fill(palette(ui.ctx()).sunken)
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(label)
                    .size(12.0)
                    .color(palette(ui.ctx()).muted),
            )
        })
        .response
}

pub(crate) fn card<R>(ui: &mut Ui, label: &str, body: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::NONE
        .fill(palette(ui.ctx()).surface)
        .stroke(Stroke::new(1.0, palette(ui.ctx()).border))
        .corner_radius(6)
        .inner_margin(metrics(ui.ctx()).card_margin)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            section(ui, label);
            ui.add_space(2.0);
            body(ui)
        })
        .inner
}

#[cfg(test)]
mod tests;
