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
    for (palette, colors) in SignalPalette::ALL.into_iter().zip(expected) {
        crate::Settings {
            signal_palette: palette,
            ..Default::default()
        }
        .apply_appearance(&ctx);
        for (tone, hex) in [Low, High, Bus, Unknown, Error, WidthMismatch]
            .into_iter()
            .zip(colors)
        {
            let [_, r, g, b] = u32::to_be_bytes(hex);
            assert_eq!(signal(&ctx, tone), Color32::from_rgb(r, g, b));
            assert!(
                contrast(signal(&ctx, tone), CANVAS) >= 3.0,
                "{palette:?} {tone:?} wire"
            );
            for surface in [CANVAS, SUNKEN, CHROME, SELECTED] {
                assert!(
                    contrast(signal_text(&ctx, tone), surface) >= 4.5,
                    "{palette:?} {tone:?} text on {surface:?}"
                );
            }
        }
        assert_eq!(active_palette(&other), SignalPalette::Default);
    }
    for color in [INK, MUTED, ACCENT] {
        assert!(contrast(color, CHROME) >= 4.5);
    }
}
