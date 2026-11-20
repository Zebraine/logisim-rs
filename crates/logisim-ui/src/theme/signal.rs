//! Signal ink is resolved per egui context; document artwork keeps its own colors.
use crate::SignalTone;
use egui::{Color32, Context, Id};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SignalPalette {
    #[default]
    Default,
    HighContrast,
    ColorblindSafe,
}

impl SignalPalette {
    pub const ALL: [Self; 3] = [Self::Default, Self::HighContrast, Self::ColorblindSafe];

    pub(crate) fn name(self) -> &'static str {
        crate::i18n::tr(match self {
            Self::Default => "Default",
            Self::HighContrast => "High contrast",
            Self::ColorblindSafe => "Colorblind-safe",
        })
    }
}

use super::rgb;

impl SignalPalette {
    pub(crate) fn color(self, tone: SignalTone) -> Color32 {
        let [low, high, bus, unknown, error, mismatch] = match self {
            Self::Default => [0x0B6B3A, 0x16A34A, 0x475569, 0x7C3AED, 0xDC2626, 0xB45309],
            Self::HighContrast => [0x052E16, 0x15803D, 0x1F2937, 0x5B21B6, 0xB91C1C, 0x92400E],
            Self::ColorblindSafe => [0x1D4ED8, 0xC2410C, 0x334155, 0x6D28D9, 0xB91C1C, 0xA16207],
        };
        rgb(match tone {
            SignalTone::Low => low,
            SignalTone::High => high,
            SignalTone::Bus => bus,
            SignalTone::Unknown => unknown,
            SignalTone::Error => error,
            SignalTone::WidthMismatch => mismatch,
        })
    }

    pub(crate) fn text_color(self, tone: SignalTone) -> Color32 {
        // Value text can sit on white, inset surfaces or a selected memory word.
        match (self, tone) {
            (Self::Default, SignalTone::High) => rgb(0x126F35),
            (Self::HighContrast, SignalTone::High) => rgb(0x095A2A),
            (Self::Default, SignalTone::Error) => rgb(0xC42121),
            (Self::Default, SignalTone::WidthMismatch) => rgb(0x9D4608),
            (Self::ColorblindSafe, SignalTone::High) => rgb(0xAF390B),
            (Self::ColorblindSafe, SignalTone::WidthMismatch) => rgb(0x895307),
            _ => self.color(tone),
        }
    }

    pub(crate) fn apply(self, ctx: &Context) {
        ctx.data_mut(|d| *d.get_temp_mut_or_default::<Self>(palette_id()) = self);
    }
}

fn palette_id() -> Id {
    Id::unique("appearance.signal_palette")
}

pub(crate) fn active_palette(ctx: &Context) -> SignalPalette {
    ctx.data(|d| d.get_temp(palette_id())).unwrap_or_default()
}

pub(crate) fn signal(ctx: &Context, tone: SignalTone) -> Color32 {
    active_palette(ctx).color(tone)
}

pub(crate) fn signal_text(ctx: &Context, tone: SignalTone) -> Color32 {
    active_palette(ctx).text_color(tone)
}
