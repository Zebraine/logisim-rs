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

use super::{ThemeMode, rgb};

impl SignalPalette {
    pub(crate) fn color(self, mode: ThemeMode, tone: SignalTone) -> Color32 {
        let [low, high, bus, unknown, error, mismatch] = match (mode, self) {
            (ThemeMode::Light, Self::Default) => {
                [0x0B6B3A, 0x16A34A, 0x475569, 0x7C3AED, 0xDC2626, 0xB45309]
            }
            (ThemeMode::Light, Self::HighContrast) => {
                [0x052E16, 0x15803D, 0x1F2937, 0x5B21B6, 0xB91C1C, 0x92400E]
            }
            (ThemeMode::Light, Self::ColorblindSafe) => {
                [0x1D4ED8, 0xC2410C, 0x334155, 0x6D28D9, 0xB91C1C, 0xA16207]
            }
            (ThemeMode::Dark, Self::Default) => {
                [0x299568, 0x4ADE80, 0x94A3B8, 0xC084FC, 0xF87171, 0xFBBF24]
            }
            (ThemeMode::Dark, Self::HighContrast) => {
                [0x34A277, 0xBBF7D0, 0xCBD5E1, 0xD8B4FE, 0xFDA4AF, 0xFDE68A]
            }
            (ThemeMode::Dark, Self::ColorblindSafe) => {
                [0x60A5FA, 0xFDBA74, 0xA5B4C7, 0xC4B5FD, 0xFB7185, 0xFACC15]
            }
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

    pub(crate) fn text_color(self, mode: ThemeMode, tone: SignalTone) -> Color32 {
        if mode == ThemeMode::Dark {
            // Memory values also appear on the selected-cell accent fill.
            return match (self, tone) {
                (Self::Default | Self::HighContrast, SignalTone::Low) => rgb(0x6EE7B7),
                (Self::ColorblindSafe, SignalTone::Low) => rgb(0x93C5FD),
                (_, SignalTone::Bus) => rgb(0xCBD5E1),
                (Self::Default, SignalTone::Unknown) => rgb(0xD8B4FE),
                (_, SignalTone::Error) => rgb(0xFDA4AF),
                _ => self.color(mode, tone),
            };
        }
        // Value text can sit on white, inset surfaces or a selected memory word.
        match (self, tone) {
            (Self::Default, SignalTone::High) => rgb(0x126F35),
            (Self::HighContrast, SignalTone::High) => rgb(0x095A2A),
            (Self::Default, SignalTone::Error) => rgb(0xC42121),
            (Self::Default, SignalTone::WidthMismatch) => rgb(0x9D4608),
            (Self::ColorblindSafe, SignalTone::High) => rgb(0xAF390B),
            (Self::ColorblindSafe, SignalTone::WidthMismatch) => rgb(0x895307),
            _ => self.color(mode, tone),
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
    active_palette(ctx).color(super::palette(ctx).mode, tone)
}

pub(crate) fn signal_text(ctx: &Context, tone: SignalTone) -> Color32 {
    active_palette(ctx).text_color(super::palette(ctx).mode, tone)
}
