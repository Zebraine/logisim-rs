//! Stable setting identities and their bindings, shared by search, controls and reset.
use super::*;
use egui::Ui;

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq)]
pub(super) enum Category {
    #[default]
    General,
    Appearance,
    Editor,
    Simulation,
    Generation,
    Advanced,
}
impl Category {
    pub const ALL: [Self; 6] = [
        Self::General,
        Self::Appearance,
        Self::Editor,
        Self::Simulation,
        Self::Generation,
        Self::Advanced,
    ];
    pub fn label(self) -> &'static str {
        tr(match self {
            Self::General => "General",
            Self::Appearance => "Appearance",
            Self::Editor => "Editor",
            Self::Simulation => "Simulation",
            Self::Generation => "Circuit generation",
            Self::Advanced => "Advanced",
        })
    }
}
pub(super) enum Control {
    Language,
    Scale,
    Toggle(fn(&mut Settings) -> &mut bool),
    Zoom,
    Mode,
    Frequency,
}
pub(super) struct Entry {
    pub key: &'static str,
    pub category: Category,
    pub label: &'static str,
    pub help: &'static str,
    pub control: Control,
}
macro_rules! registry {
    ($(($key:literal,$category:ident,$label:literal,$help:literal,$control:expr)),+ $(,)?)=>{
        pub(super) const REGISTRY:&[Entry]=&[$(Entry{key:$key,category:Category::$category,label:$label,help:$help,control:$control}),+];
    }
}
registry![
    (
        "general.language",
        General,
        "Language",
        "Choose the language used by the workbench.",
        Control::Language
    ),
    (
        "appearance.ui_scale",
        Appearance,
        "Interface scale",
        "Resize text and controls throughout the app.",
        Control::Scale
    ),
    (
        "appearance.reduce_motion",
        Appearance,
        "Reduce motion",
        "Turn off interface animations.",
        Control::Toggle(|s| &mut s.reduce_motion)
    ),
    (
        "appearance.show_telemetry",
        Appearance,
        "Show telemetry",
        "Show FPS and clock rates in the toolbar.",
        Control::Toggle(|s| &mut s.show_telemetry)
    ),
    (
        "editor.show_grid",
        Editor,
        "Show grid",
        "Display the dot grid on the canvas.",
        Control::Toggle(|s| &mut s.show_grid)
    ),
    (
        "editor.default_zoom",
        Editor,
        "Initial canvas zoom",
        "Set the zoom when opening a circuit or entering a subcircuit.",
        Control::Zoom
    ),
    (
        "simulation.mode",
        Simulation,
        "Simulation mode",
        "Choose how the simulator propagates signals.",
        Control::Mode
    ),
    (
        "simulation.tick_hz",
        Simulation,
        "Tick frequency",
        "Set the automatic clock rate in Hz.",
        Control::Frequency
    ),
    (
        "generation.truth_table",
        Generation,
        "Enable truth table generation",
        "Show truth tables in circuit generation.",
        Control::Toggle(|s| &mut s.truth_table)
    ),
];
impl Entry {
    pub fn matches(&self, query: &str) -> bool {
        [tr(self.label), tr(self.help)]
            .iter()
            .any(|s| s.to_lowercase().contains(query))
    }
    pub fn visible(&self, category: Category, query: &str) -> bool {
        if query.is_empty() {
            self.category == category
        } else {
            self.matches(query)
        }
    }
    pub fn copy(&self, target: &mut Settings, mut source: Settings) {
        match self.control {
            Control::Language => target.language = source.language,
            Control::Scale => target.ui_scale = source.ui_scale,
            Control::Toggle(field) => *field(target) = *field(&mut source),
            Control::Zoom => target.default_zoom = source.default_zoom,
            Control::Mode => target.simulation_mode = source.simulation_mode,
            Control::Frequency => target.tick_hz = source.tick_hz,
        }
    }
    pub fn changed(&self, current: Settings, baseline: Settings) -> bool {
        let mut one = baseline;
        self.copy(&mut one, current);
        baseline != one
    }
    pub fn show(&self, ui: &mut Ui, values: &mut Settings, draft: &mut Option<u32>) {
        match self.control {
            Control::Language => choice(ui, &mut values.language, Language::ALL, |v| {
                v.name().to_owned()
            }),
            Control::Scale => choice(ui, &mut values.ui_scale, UiScale::ALL, |v| {
                format!("{}%", v.percent())
            }),
            Control::Zoom => choice(
                ui,
                &mut values.default_zoom,
                [DefaultZoom::Fit, DefaultZoom::Actual],
                |v| match v {
                    DefaultZoom::Fit => tr("Fit to canvas").to_owned(),
                    DefaultZoom::Actual => "100%".into(),
                },
            ),
            Control::Mode => choice(ui, &mut values.simulation_mode, SimulationMode::ALL, |v| {
                v.name().to_owned()
            }),
            Control::Toggle(field) => {
                let current = field(values);
                let label = tr(if *current { "On" } else { "Off" });
                let response = ui.checkbox(current, label);
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::Role::CheckBox,
                        ui.is_enabled(),
                        *current,
                        tr(self.label),
                    )
                });
                theme::focus_ring(ui, &response);
            }
            Control::Frequency => {
                let mut hz = draft.unwrap_or(values.tick_hz.get());
                let r = ui.add(
                    egui::DragValue::new(&mut hz)
                        .range(1..=crate::MAX_TICK_HZ)
                        .suffix(" Hz")
                        .update_while_editing(false),
                );
                if r.changed() || draft.is_some() {
                    if r.dragged() {
                        *draft = Some(hz);
                    } else {
                        values.tick_hz = TickHz::new(hz).expect("bounded frequency control");
                        *draft = None;
                    }
                }
                theme::focus_ring(ui, &r);
            }
        }
    }
}
fn choice<T: Copy + PartialEq, const N: usize>(
    ui: &mut Ui,
    current: &mut T,
    options: [T; N],
    label: impl Fn(T) -> String,
) {
    let r = egui::ComboBox::from_id_salt("choice")
        .width(ui.available_width().clamp(88.0, 156.0))
        .selected_text(label(*current))
        .show_ui(ui, |ui| {
            for value in options {
                ui.selectable_value(current, value, label(value));
            }
        })
        .response;
    theme::focus_ring(ui, &r);
}
