//! Shared desktop chrome. Document artwork supplies its own colors and fonts.
use egui::{Color32, Context, FontId, Response, Stroke, StrokeKind, TextStyle, Ui, vec2};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Accent {
    #[default]
    Indigo,
    Blue,
    Teal,
}

mod signal;
pub use signal::SignalPalette;
pub(crate) use signal::{active_palette, signal, signal_text};

pub(crate) const CANVAS: Color32 = Color32::WHITE;
pub(crate) const SURFACE: Color32 = Color32::WHITE;
pub(crate) const CHROME: Color32 = Color32::from_rgb(246, 248, 251);
pub(crate) const SUNKEN: Color32 = Color32::from_rgb(238, 242, 247);
pub(crate) const BORDER: Color32 = Color32::from_rgb(226, 232, 240);
pub(crate) const BORDER_STRONG: Color32 = Color32::from_rgb(203, 213, 225);
pub(crate) const INK: Color32 = Color32::from_rgb(15, 23, 42);
pub(crate) const MUTED: Color32 = Color32::from_rgb(91, 107, 127);
pub(crate) const FAINT: Color32 = Color32::from_rgb(148, 163, 184);
pub(crate) const ACCENT: Color32 = Color32::from_rgb(79, 70, 229);
pub(crate) const ACCENT_HOVER: Color32 = Color32::from_rgb(67, 56, 202);
pub(crate) const ACCENT_SOFT: Color32 = Color32::from_rgb(238, 242, 255);
pub(crate) const SELECTED: Color32 = Color32::from_rgb(224, 231, 255);
pub(crate) const SUCCESS: Color32 = Color32::from_rgb(21, 128, 61);
pub(crate) const WARNING: Color32 = Color32::from_rgb(180, 83, 9);
pub(crate) const DANGER: Color32 = Color32::from_rgb(220, 38, 38);

pub(crate) const ANIMATION_SECONDS: f32 = 0.12;

/// Configure the shared light theme before the workspace or a host dialog opens.
pub fn configure(ctx: &Context) {
    ctx.set_theme(egui::Theme::Light);
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
    let mut style = (*ctx.global_style()).clone();
    style.text_styles.extend([
        (TextStyle::Body, FontId::proportional(13.0)),
        (TextStyle::Button, FontId::proportional(13.0)),
        (TextStyle::Small, FontId::proportional(12.0)),
        (TextStyle::Heading, FontId::proportional(16.0)),
        (TextStyle::Monospace, FontId::monospace(12.0)),
    ]);
    style.spacing.item_spacing = vec2(4.0, 4.0);
    style.spacing.button_padding = vec2(8.0, 4.0);
    style.spacing.interact_size.y = 28.0;
    style.spacing.window_margin = 16.into();
    style.spacing.menu_margin = 4.into();
    style.spacing.scroll.bar_width = 6.0;
    style.animation_time = ANIMATION_SECONDS;
    let v = &mut style.visuals;
    v.panel_fill = CHROME;
    v.window_fill = SURFACE;
    v.window_stroke = Stroke::new(1.0, BORDER_STRONG);
    v.window_corner_radius = 8.into();
    v.menu_corner_radius = 8.into();
    v.window_highlight_topmost = false;
    v.faint_bg_color = SUNKEN;
    v.extreme_bg_color = SURFACE;
    v.text_edit_bg_color = Some(SURFACE);
    v.weak_text_color = Some(MUTED);
    v.hyperlink_color = ACCENT;
    v.warn_fg_color = WARNING;
    v.error_fg_color = DANGER;
    v.selection.bg_fill = SELECTED;
    v.selection.stroke = Stroke::new(2.0, ACCENT);
    v.disabled_alpha = 0.64;
    for (widget, fill, border, ink) in [
        (&mut v.widgets.noninteractive, CHROME, BORDER, INK),
        (&mut v.widgets.inactive, SURFACE, BORDER_STRONG, INK),
        (
            &mut v.widgets.hovered,
            ACCENT_SOFT,
            BORDER_STRONG,
            ACCENT_HOVER,
        ),
        (&mut v.widgets.open, SELECTED, ACCENT, ACCENT),
        (&mut v.widgets.active, SELECTED, ACCENT, INK),
    ] {
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.bg_stroke = Stroke::new(1.0, border);
        widget.fg_stroke = Stroke::new(1.5, ink);
        widget.corner_radius = 4.into();
        widget.expansion = 0.0;
    }
    v.widgets.active.bg_stroke.width = 2.0;
    ctx.set_global_style(style);
}

pub(crate) fn focus_ring(ui: &Ui, response: &Response) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect,
            4,
            Stroke::new(2.0, ACCENT),
            StrokeKind::Inside,
        );
    }
}

pub(crate) fn section(ui: &mut Ui, label: &str) -> Response {
    ui.label(
        egui::RichText::new(label.to_uppercase())
            .size(11.0)
            .extra_letter_spacing(0.4)
            .strong()
            .color(MUTED),
    )
}

pub(crate) fn chip(ui: &mut Ui, label: &str) -> Response {
    egui::Frame::NONE
        .fill(SUNKEN)
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(label).size(12.0).color(MUTED))
        })
        .response
}

pub(crate) fn card<R>(ui: &mut Ui, label: &str, body: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::NONE
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(6)
        .inner_margin(8)
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
