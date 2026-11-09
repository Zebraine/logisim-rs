//! Generic rendering and property/action controls for plugin-owned definitions.
use crate::{
    SimulationAction, Workspace,
    canvas::Canvas,
    editor::Tool,
    i18n::tr,
    simulation::{SimulationUi, SimulationView},
};
use egui::{Align2, Color32, Painter, Rect, Stroke, StrokeKind, Ui};
use logisim_core::{
    Component, Kind, Point,
    plugin::api::{Manifest, PropertyKind, Shape},
};

impl Workspace {
    /// Dependency edits are part of the document and participate in Undo.
    pub fn set_plugin_sources(&mut self, sources: Vec<logisim_core::plugin::PluginSource>) {
        self.finish_edit();
        if self.editor.session.project.plugin_sources != sources {
            let before = self.editor.snapshot();
            self.editor.session.project.plugin_sources = sources;
            self.editor.record(before);
        }
    }
    pub fn set_plugin_catalogue(&mut self, manifests: Vec<Manifest>) {
        self.plugins = manifests;
    }
    pub(crate) fn plugin_groups(&mut self, ui: &mut Ui) {
        for manifest in &self.plugins {
            egui::CollapsingHeader::new(&manifest.name)
                .id_salt(("plugin-library", &manifest.id, &manifest.version))
                .default_open(true)
                .show(ui, |ui| {
                    for component in manifest.components() {
                        let selected = self.editor.tool == Tool::Place(Kind::Plugin)
                            && self.editor.template.plugin.as_ref().is_some_and(|p| {
                                p.plugin == component.plugin
                                    && p.version == component.version
                                    && p.tool.id == component.tool.id
                            });
                        ui.push_id(&component.tool.id, |ui| {
                            if ui
                                .selectable_label(selected, &component.tool.name)
                                .clicked()
                            {
                                self.editor.session.context.appearance_mode = false;
                                self.editor.set_tool(Tool::Place(Kind::Plugin));
                                self.editor.template =
                                    Component::from_plugin(component.clone(), Point::ZERO);
                            }
                        });
                    }
                });
        }
    }
}
pub(crate) fn fields(ui: &mut Ui, c: &mut Component, manifests: &[Manifest]) -> bool {
    let Some(plugin) = c.plugin.as_mut() else {
        return false;
    };
    if let Some(manifest) = manifests
        .iter()
        .find(|m| m.id == plugin.plugin && m.version == plugin.version)
    {
        ui.label(tr("Component"));
        let mut selected = plugin.tool.id.clone();
        egui::ComboBox::from_id_salt("plugin-tool")
            .selected_text(&plugin.tool.name)
            .show_ui(ui, |ui| {
                for tool in &manifest.tools {
                    ui.selectable_value(&mut selected, tool.id.clone(), &tool.name);
                }
            });
        ui.end_row();
        if selected != plugin.tool.id {
            let tool = manifest
                .tools
                .iter()
                .find(|t| t.id == selected)
                .expect("catalogue tool")
                .clone();
            plugin.properties = tool
                .properties
                .iter()
                .map(|property| {
                    let value = plugin
                        .properties
                        .get(&property.id)
                        .filter(|v| property.accepts(v))
                        .unwrap_or(&property.default)
                        .clone();
                    (property.id.clone(), value)
                })
                .collect();
            plugin.tool = tool;
        }
    }
    ui.label(tr("Plugin"));
    ui.label(format!("{} {}", plugin.plugin, plugin.version));
    ui.end_row();
    let mut active = false;
    for property in &plugin.tool.properties {
        ui.label(&property.label);
        let value = plugin
            .properties
            .get_mut(&property.id)
            .expect("validated plugin property");
        ui.push_id(
            (&plugin.plugin, &plugin.tool.id, &property.id),
            |ui| match &property.kind {
                PropertyKind::Boolean => {
                    let mut b = value == "true";
                    if ui.checkbox(&mut b, "").changed() {
                        *value = b.to_string();
                    }
                }
                PropertyKind::Integer { min, max } => {
                    let mut n = value.parse::<i64>().expect("validated integer");
                    let r = ui.add(egui::DragValue::new(&mut n).range(*min..=*max));
                    if r.changed() {
                        *value = n.to_string();
                    }
                    active |= r.has_focus() || r.dragged();
                }
                PropertyKind::Choice { values } => {
                    egui::ComboBox::from_id_salt("value")
                        .selected_text(value.as_str())
                        .show_ui(ui, |ui| {
                            for option in values {
                                ui.selectable_value(value, option.clone(), option);
                            }
                        });
                }
            },
        );
        ui.end_row();
    }
    active
}
pub(crate) fn runtime(ui: &mut Ui, sim: &mut SimulationUi, c: &Component) {
    let Some(plugin) = &c.plugin else {
        return;
    };
    if let Some(error) = sim.view.plugin_errors.get(&c.id) {
        ui.colored_label(Color32::DARK_RED, error);
        ui.weak(tr(
            "Load the matching plugin, or Reset after correcting the problem.",
        ));
    }
    for (i, port) in plugin.tool.ports.iter().enumerate() {
        if let Some(value) = sim.view.ports.get(&(c.id, i)) {
            ui.label(format!("{}: {}", port.label, value.text))
                .on_hover_text(&value.binary);
        }
    }
    for action in &plugin.tool.actions {
        ui.push_id(("plugin-action", c.id, &action.id), |ui| {
            let argument = sim
                .plugin_arguments
                .entry((c.id, action.id.clone()))
                .or_default();
            if let Some(label) = &action.argument {
                ui.label(label);
                ui.add(egui::TextEdit::singleline(argument).char_limit(1024));
            }
            if ui
                .add_enabled(
                    !sim.view.plugin_errors.contains_key(&c.id),
                    egui::Button::new(&action.label),
                )
                .clicked()
            {
                sim.actions.push(SimulationAction::PluginAction {
                    id: c.id,
                    action: action.id.clone(),
                    argument: argument.clone(),
                });
            }
        });
    }
}
pub(crate) fn draw(
    p: &Painter,
    canvas: &Canvas,
    c: &Component,
    view: Option<&SimulationView>,
    ghost: bool,
) {
    let Some(plugin) = &c.plugin else {
        return;
    };
    let at = |p: [i32; 2]| canvas.screen(c.position + c.facing.transform(Point::new(p[0], p[1])));
    let bounds = Rect::from_two_pos(at(plugin.tool.bounds[0]), at(plugin.tool.bounds[1]));
    let p = p.with_clip_rect(p.clip_rect().intersect(bounds.expand(2.0)));
    let color = |rgba: [u8; 4]| {
        let c = Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
        if ghost { c.gamma_multiply(0.45) } else { c }
    };
    let dynamic = view
        .and_then(|v| v.plugin_graphics.get(&c.id))
        .map_or(&[][..], Vec::as_slice);
    for shape in plugin.tool.graphics.iter().chain(dynamic) {
        match shape {
            Shape::Rect {
                min,
                max,
                fill,
                stroke,
            } => {
                p.rect(
                    Rect::from_two_pos(at(*min), at(*max)),
                    0,
                    color(*fill),
                    Stroke::new(canvas.camera.zoom, color(*stroke)),
                    StrokeKind::Inside,
                );
            }
            Shape::Ellipse {
                min,
                max,
                fill,
                stroke,
            } => {
                let r = Rect::from_two_pos(at(*min), at(*max));
                p.add(egui::Shape::ellipse_filled(
                    r.center(),
                    r.size() / 2.0,
                    color(*fill),
                ));
                p.add(egui::Shape::ellipse_stroke(
                    r.center(),
                    r.size() / 2.0,
                    Stroke::new(canvas.camera.zoom, color(*stroke)),
                ));
            }
            Shape::Line { a, b, color: rgba } => {
                p.line_segment(
                    [at(*a), at(*b)],
                    Stroke::new(canvas.camera.zoom, color(*rgba)),
                );
            }
            Shape::Text {
                at: position,
                text,
                size,
                color: rgba,
            } => {
                let font = logisim_core::ArtworkFont {
                    family: "monospace".into(),
                    size: *size,
                    ..Default::default()
                };
                let angle = match c.facing {
                    logisim_core::Facing::East => 0.0,
                    logisim_core::Facing::South => std::f32::consts::FRAC_PI_2,
                    logisim_core::Facing::West => std::f32::consts::PI,
                    logisim_core::Facing::North => -std::f32::consts::FRAC_PI_2,
                };
                crate::fonts::Text {
                    text,
                    font: &font,
                    color: color(*rgba),
                    zoom: canvas.camera.zoom,
                    angle,
                }
                .paint(&p, at(*position), Align2::LEFT_TOP);
            }
        }
    }
    if view.is_some_and(|v| v.plugin_errors.contains_key(&c.id)) {
        p.rect_stroke(
            bounds,
            0,
            Stroke::new(2.0, Color32::RED),
            StrokeKind::Inside,
        );
    }
}
