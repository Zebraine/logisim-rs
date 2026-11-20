//! Workbench metrics are logical points. Canvas coordinates and document artwork use their own units.
use egui::{Color32, Context, Id, Vec2, vec2};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AccentColors {
    pub base: Color32,
    pub hover: Color32,
    pub soft: Color32,
    pub selected: Color32,
}

impl Accent {
    pub(crate) fn colors(self) -> AccentColors {
        let [base, hover, soft, selected] = match self {
            Self::Indigo => [0x4F46E5, 0x4338CA, 0xEEF2FF, 0xE0E7FF],
            Self::Blue => [0x2563EB, 0x1D4ED8, 0xEFF6FF, 0xDBEAFE],
            Self::Teal => [0x0F766E, 0x115E59, 0xF0FDFA, 0xCCFBF1],
        }
        .map(super::rgb);
        AccentColors {
            base,
            hover,
            soft,
            selected,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SidebarWidths {
    pub min: f32,
    pub default: f32,
    pub max: f32,
}

impl SidebarWidths {
    /// Store divider preferences in Comfortable units across theme switches.
    pub fn project(self, width: f32, target: Self) -> f32 {
        if width <= self.default {
            egui::remap_clamp(width, self.min..=self.default, target.min..=target.default)
        } else {
            egui::remap_clamp(width, self.default..=self.max, target.default..=target.max)
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Metrics {
    pub menu: f32,
    pub toolbar_wide: f32,
    pub toolbar_wrapped: f32,
    pub zoom_bar: f32,
    pub icon_button: f32,
    pub row: f32,
    pub sidebar: SidebarWidths,
    pub card_margin: i8,
    pub dialog_margin: i8,
    pub settings_size: Vec2,
    pub control: f32,
    pub gap: i8,
}

impl Density {
    pub(crate) fn metrics(self) -> Metrics {
        match self {
            Self::Comfortable => Metrics {
                menu: 30.,
                toolbar_wide: 50.,
                toolbar_wrapped: 78.,
                zoom_bar: 40.,
                icon_button: 28.,
                row: 30.,
                sidebar: SidebarWidths {
                    min: 230.,
                    default: 320.,
                    max: 380.,
                },
                card_margin: 8,
                dialog_margin: 12,
                settings_size: vec2(760., 560.),
                control: 28.,
                gap: 4,
            },
            Self::Compact => Metrics {
                menu: 28.,
                toolbar_wide: 44.,
                toolbar_wrapped: 68.,
                zoom_bar: 34.,
                icon_button: 24.,
                row: 26.,
                sidebar: SidebarWidths {
                    min: 210.,
                    default: 280.,
                    max: 340.,
                },
                card_margin: 6,
                dialog_margin: 8,
                settings_size: vec2(720., 520.),
                control: 26.,
                gap: 3,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Chrome {
    density: Density,
    accent: Accent,
}

fn id() -> Id {
    Id::unique("workbench.chrome")
}

pub(crate) fn apply_chrome(ctx: &Context, density: Density, accent: Accent) {
    let chrome = Chrome { density, accent };
    let changed = ctx.data_mut(|d| {
        if d.get_temp::<Chrome>(id()) == Some(chrome) {
            return false;
        }
        d.insert_temp(id(), chrome);
        true
    });
    if changed {
        ctx.global_style_mut(|s| super::style_chrome(s, density.metrics(), accent.colors()));
    }
}

fn current(ctx: &Context) -> Chrome {
    ctx.data(|d| d.get_temp(id())).unwrap_or_default()
}

pub(crate) fn colors(ctx: &Context) -> AccentColors {
    current(ctx).accent.colors()
}
pub(crate) fn metrics(ctx: &Context) -> Metrics {
    current(ctx).density.metrics()
}
