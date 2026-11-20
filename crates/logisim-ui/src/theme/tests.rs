use super::*;
use crate::SignalTone;

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
    let paint = palette(&ctx);
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
                contrast(signal(&ctx, tone), paint.canvas) >= 3.0,
                "{palette:?} {tone:?} wire"
            );
            for surface in [
                paint.canvas,
                paint.sunken,
                paint.chrome,
                colors(&ctx).selected,
            ] {
                assert!(
                    contrast(signal_text(&ctx, tone), surface) >= 4.5,
                    "{palette:?} {tone:?} text on {surface:?}"
                );
            }
        }
        assert_eq!(active_palette(&other), SignalPalette::Default);
    }
    for color in [paint.ink, paint.muted, colors(&ctx).base] {
        assert!(contrast(color, paint.chrome) >= 4.5);
    }
}

#[test]
fn accent_presets_update_standard_visuals_with_readable_chrome_and_independent_signals() {
    let ctx = Context::default();
    configure(&ctx);
    let other = Context::default();
    let paint = palette(&ctx);
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
        assert!(contrast(c.base, paint.surface) >= 4.5 && contrast(c.hover, paint.surface) >= 4.5);
        assert!(contrast(paint.ink, c.selected) >= 4.5 && contrast(paint.ink, c.soft) >= 4.5);
        let v = &ctx.global_style().visuals;
        assert_eq!(v.selection.bg_fill, c.selected);
        assert_eq!(v.selection.stroke, Stroke::new(2., c.hover));
        assert!(contrast(v.selection.stroke.color, v.selection.bg_fill) >= 4.5);
        assert_eq!(v.widgets.active.bg_stroke, Stroke::new(2., c.base));
        assert_eq!(v.hyperlink_color, c.base);
        assert_eq!(v.widgets.hovered.weak_bg_fill, c.soft);
        assert_eq!(
            (v.warn_fg_color, v.error_fg_color),
            (paint.warning, paint.danger)
        );
        assert_eq!(
            signal(&ctx, SignalTone::High),
            SignalPalette::ColorblindSafe.color(ThemeMode::Light, SignalTone::High)
        );
        assert_eq!(colors(&other), Accent::Indigo.colors(ThemeMode::Light));
    }
}

#[test]
fn metrics_have_the_prescribed_sizes_in_logical_points() {
    for (density, sizes, sidebar, dialog, large_dialog, settings) in [
        (
            Density::Comfortable,
            [30., 50., 78., 40., 28., 30., 8., 28.],
            [230., 320., 380.],
            vec2(660., 540.),
            vec2(800., 620.),
            vec2(760., 560.),
        ),
        (
            Density::Compact,
            [28., 44., 68., 34., 24., 26., 6., 26.],
            [210., 280., 340.],
            vec2(620., 500.),
            vec2(740., 580.),
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
        assert_eq!(
            (m.dialog, m.large_dialog, m.settings_size),
            (dialog, large_dialog, settings)
        );
        assert_eq!(ctx.global_style().spacing.interact_size.y, m.control);
    }
}

#[test]
fn both_workbench_palettes_and_dark_signal_contrast_are_context_local() {
    use SignalTone::*;
    let ctx = Context::default();
    let other = Context::default();
    configure(&other);
    for (mode, expected) in [
        (
            ThemeMode::Light,
            [
                0xFFFFFF, 0xFFFFFF, 0xF6F8FB, 0xEEF2F7, 0xE2E8F0, 0xCBD5E1, 0x0F172A, 0x5B6B7F,
                0x94A3B8, 0x15803D, 0xB45309, 0xDC2626,
            ],
        ),
        (
            ThemeMode::Dark,
            [
                0x0B1220, 0x111827, 0x0F172A, 0x1E293B, 0x334155, 0x475569, 0xE2E8F0, 0x94A3B8,
                0x64748B, 0x4ADE80, 0xFBBF24, 0xF87171,
            ],
        ),
    ] {
        apply_chrome(&ctx, mode, Density::Comfortable, Accent::Indigo);
        let p = palette(&ctx);
        assert_eq!(
            [
                p.canvas,
                p.surface,
                p.chrome,
                p.sunken,
                p.border,
                p.border_strong,
                p.ink,
                p.muted,
                p.faint,
                p.success,
                p.warning,
                p.danger
            ],
            expected.map(rgb)
        );
        assert_eq!(ctx.global_style().visuals.window_fill, p.surface);
        assert_eq!(ctx.global_style().visuals.panel_fill, p.chrome);
        assert_eq!(
            ctx.global_style().visuals.dark_mode,
            mode == ThemeMode::Dark
        );
    }
    let p = palette(&ctx);
    for (accent, expected) in [
        (Accent::Indigo, [0x818CF8, 0xA5B4FC, 0x1E1B4B, 0x312E81]),
        (Accent::Blue, [0x60A5FA, 0x93C5FD, 0x172554, 0x1E3A8A]),
        (Accent::Teal, [0x2DD4BF, 0x5EEAD4, 0x042F2E, 0x134E4A]),
    ] {
        apply_chrome(&ctx, ThemeMode::Dark, Density::Comfortable, accent);
        let c = colors(&ctx);
        assert_eq!([c.base, c.hover, c.soft, c.selected], expected.map(rgb));
        for signals in SignalPalette::ALL {
            signals.apply(&ctx);
            for tone in [Low, High, Bus, Unknown, Error, WidthMismatch] {
                assert!(contrast(signal(&ctx, tone), p.canvas) >= 3.0);
                for surface in [p.surface, p.canvas, p.sunken, c.selected] {
                    assert!(
                        contrast(signal_text(&ctx, tone), surface) >= 4.5,
                        "{accent:?} {signals:?} {tone:?}"
                    );
                }
            }
        }
    }
    assert_eq!(palette(&other).mode, ThemeMode::Light);
    assert_eq!(active_palette(&other), SignalPalette::Default);
    configure(&ctx);
    assert_eq!(palette(&ctx), palette(&other));
}
