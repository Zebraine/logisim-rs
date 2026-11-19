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
    for tone in [
        SignalTone::Low,
        SignalTone::High,
        SignalTone::Bus,
        SignalTone::Unknown,
        SignalTone::Error,
        SignalTone::WidthMismatch,
    ] {
        assert!(contrast(tone.color(), CANVAS) >= 3.0, "{tone:?} wire");
        assert!(contrast(tone.text_color(), CANVAS) >= 4.5, "{tone:?} text");
    }
    for color in [INK, MUTED, ACCENT] {
        assert!(contrast(color, CHROME) >= 4.5);
    }
}
