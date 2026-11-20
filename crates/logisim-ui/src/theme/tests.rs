use super::*;
use crate::SignalTone;

fn contrast(a: Color32, b: Color32) -> f32 {
    let luminance = |c| {
        let rgb = egui::Rgba::from(c);
        0.2126 * rgb.r() + 0.7152 * rgb.g() + 0.0722 * rgb.b()
    };
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[test]
fn wires_and_signal_labels_contrast_with_canvas() {
    use SignalTone::*;
    let expected = [
        [0x0B6B3A, 0x16A34A, 0x475569, 0x7C3AED, 0xDC2626, 0xB45309],
        [0x052E16, 0x15803D, 0x1F2937, 0x5B21B6, 0xB91C1C, 0x92400E],
        [0x1D4ED8, 0xC2410C, 0x334155, 0x6D28D9, 0xB91C1C, 0xA16207],
    ];
    let ctx = Context::default();
    let other = Context::default();
    for (palette, expected_colors) in SignalPalette::ALL.into_iter().zip(expected) {
        crate::Settings {
            signal_palette: palette,
            ..Default::default()
        }
        .apply_appearance(&ctx);
        for (tone, hex) in [Low, High, Bus, Unknown, Error, WidthMismatch]
            .into_iter()
            .zip(expected_colors)
        {
            let [_, r, g, b] = u32::to_be_bytes(hex);
            assert_eq!(signal(&ctx, tone), Color32::from_rgb(r, g, b));
            assert!(
                contrast(signal(&ctx, tone), CANVAS) >= 3.0,
                "{palette:?} {tone:?} wire"
            );
            for surface in [CANVAS, SUNKEN, CHROME, colors(&ctx).selected] {
                assert!(
                    contrast(signal_text(&ctx, tone), surface) >= 4.5,
                    "{palette:?} {tone:?} text on {surface:?}"
                );
            }
        }
        assert_eq!(active_palette(&other), SignalPalette::Default);
    }
    for color in [INK, MUTED, colors(&ctx).base] {
        assert!(contrast(color, CHROME) >= 4.5);
    }
}

#[test]
fn accent_presets_update_standard_visuals_with_readable_chrome_and_independent_signals() {
    let ctx = Context::default();
    configure(&ctx);
    let other = Context::default();
    for (accent, expected) in [
        (Accent::Indigo, [0x4F46E5, 0x4338CA, 0xEEF2FF, 0xE0E7FF]),
        (Accent::Blue, [0x2563EB, 0x1D4ED8, 0xEFF6FF, 0xDBEAFE]),
        (Accent::Teal, [0x0F766E, 0x115E59, 0xF0FDFA, 0xCCFBF1]),
    ] {
        crate::Settings {
            accent,
            signal_palette: SignalPalette::ColorblindSafe,
            ..Default::default()
        }
        .apply_appearance(&ctx);
        let c = colors(&ctx);
        assert_eq!([c.base, c.hover, c.soft, c.selected], expected.map(rgb));
        assert!(contrast(c.base, SURFACE) >= 4.5 && contrast(c.hover, SURFACE) >= 4.5);
        assert!(contrast(INK, c.selected) >= 4.5 && contrast(INK, c.soft) >= 4.5);
        let v = &ctx.global_style().visuals;
        assert_eq!(v.selection.bg_fill, c.selected);
        assert_eq!(v.selection.stroke, Stroke::new(2., c.base));
        assert_eq!(v.widgets.active.bg_stroke, v.selection.stroke);
        assert_eq!(v.hyperlink_color, c.base);
        assert_eq!(v.widgets.hovered.weak_bg_fill, c.soft);
        assert_eq!((v.warn_fg_color, v.error_fg_color), (WARNING, DANGER));
        assert_eq!(
            signal(&ctx, SignalTone::High),
            SignalPalette::ColorblindSafe.color(SignalTone::High)
        );
        assert_eq!(colors(&other), Accent::Indigo.colors());
    }
}

#[test]
fn metrics_have_the_prescribed_sizes_in_logical_points() {
    for (density, sizes, sidebar, dialog) in [
        (
            Density::Comfortable,
            [30., 50., 78., 40., 28., 30., 8., 28.],
            [230., 320., 380.],
            vec2(760., 560.),
        ),
        (
            Density::Compact,
            [28., 44., 68., 34., 24., 26., 6., 26.],
            [210., 280., 340.],
            vec2(720., 520.),
        ),
    ] {
        let ctx = Context::default();
        crate::Settings {
            density,
            ui_scale: crate::UiScale::P150,
            ..Default::default()
        }
        .apply_appearance(&ctx);
        let m = metrics(&ctx);
        assert_eq!(
            [
                m.menu,
                m.toolbar_wide,
                m.toolbar_wrapped,
                m.zoom_bar,
                m.icon_button,
                m.row,
                f32::from(m.card_margin),
                m.control
            ],
            sizes
        );
        assert_eq!([m.sidebar.min, m.sidebar.default, m.sidebar.max], sidebar);
        assert_eq!(m.settings_size, dialog);
        assert_eq!(ctx.global_style().spacing.interact_size.y, m.control);
    }
}
