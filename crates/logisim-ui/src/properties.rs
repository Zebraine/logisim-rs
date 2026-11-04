use crate::editor::{Editor, Tool};
use egui::{Frame, RichText, ScrollArea, Ui};
use logisim_core::{Component, Facing, Kind, Point};

pub(crate) fn show(ui: &mut Ui, editor: &mut Editor) {
    let placing = matches!(editor.tool, Tool::Place(_));
    let target = if placing {
        Some(editor.template.clone())
    } else if editor.selection.len() == 1 {
        editor
            .selection
            .first()
            .and_then(|id| editor.circuit.component(*id))
            .cloned()
    } else {
        None
    };
    let title = if placing {
        format!("Tool: {}", editor.tool.name())
    } else if let Some(c) = &target {
        format!("Selection: {}", c.kind.name())
    } else if !editor.selection.is_empty() {
        format!("Selection: {} item(s)", editor.selection.len())
    } else {
        "Circuit: main".into()
    };
    Frame::NONE.inner_margin(12).show(ui, |ui| {
        ui.label(RichText::new(title).strong());
    });
    ui.separator();
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            Frame::NONE.inner_margin(12).show(ui, |ui| {
                if let Some(c) = target {
                    if !placing {
                        editor.simulation.component(ui, &c);
                        ui.separator();
                    }
                    component_editor(ui, editor, c, placing);
                } else {
                    selection_info(ui, editor);
                }
                if !editor.simulation.view.issues.is_empty() {
                    ui.separator();
                    egui::CollapsingHeader::new(format!(
                        "Simulation issues ({})",
                        editor.simulation.view.issues.len()
                    ))
                    .default_open(true)
                    .show(ui, |ui| {
                        for issue in editor.simulation.view.issues.iter().take(32) {
                            if ui
                                .button(&issue.message)
                                .on_hover_text("Select affected components and wires")
                                .clicked()
                            {
                                editor.selection = issue.targets.clone();
                            }
                        }
                        if editor.simulation.view.issues.len() > 32 {
                            ui.weak("Showing the first 32 issues.");
                        }
                    });
                }
            });
        });
}

fn selection_info(ui: &mut Ui, editor: &mut Editor) {
    editor.commit_properties();
    if !editor.selection.is_empty() {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(editor.can_rotate(), egui::Button::new("Rotate"))
                .clicked()
            {
                editor.rotate();
            }
            if ui.button("Delete").clicked() {
                editor.delete();
            }
        });
        if editor.selection.len() == 1
            && let Some(w) = editor
                .circuit
                .wires
                .iter()
                .find(|w| editor.selection.contains(&w.id))
        {
            ui.label(format!("Start: {}, {}", w.a.x, w.a.y));
            ui.label(format!("End: {}, {}", w.b.x, w.b.y));
            ui.label(format!(
                "Connected wires: {}",
                editor.circuit.connected_wires(w.id).len()
            ));
            ui.weak("Drag the blue endpoint handles to reshape this wire.");
            if let Some(signal) = editor.simulation.view.wires.get(&w.id) {
                ui.colored_label(signal.tone.color(), format!("Signal: {}", signal.text));
                ui.monospace(&signal.binary);
            }
            ui.weak("The highlight follows the connected net, including matching tunnel labels.");
        }
        return;
    }
    ui.label(format!(
        "{} components · {} wires",
        editor.circuit.components.len(),
        editor.circuit.wires.len()
    ));
    ui.add_space(10.0);
    if editor.tool == Tool::Wire {
        ui.weak("Click two points or drag to connect. Shift changes the bend. Esc cancels.");
    } else {
        ui.weak("Choose a tool to place a component. Drag a terminal to connect.");
        ui.weak("Drag empty space to select. Shift adds to the selection.");
    }
    ui.add_space(10.0);
    editor.simulation.overview(ui);
    ui.add_space(8.0);
    ui.weak("In-memory circuit. File saving and clocks are not available yet.");
}

fn component_editor(ui: &mut Ui, editor: &mut Editor, mut c: Component, placing: bool) {
    let original = c.clone();
    let mut active = false;
    let response = if c.kind == Kind::Text {
        ui.label("Text");
        ui.add(
            egui::TextEdit::multiline(&mut c.label)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .char_limit(4096)
                .id_salt((c.id, "text")),
        )
    } else {
        ui.label("Label");
        ui.add(
            egui::TextEdit::singleline(&mut c.label)
                .desired_width(f32::INFINITY)
                .char_limit(128)
                .id_salt((c.id, "label")),
        )
    };
    if editor.focus_text {
        response.request_focus();
        editor.focus_text = false;
    }
    active |= response.has_focus();
    ui.add_space(8.0);
    egui::Grid::new(("component_properties", c.id))
        .num_columns(2)
        .spacing(egui::vec2(12.0, 9.0))
        .show(ui, |ui| {
            active |= fields(ui, &mut c, placing);
        });
    c.position = Point::snapped(c.position.x as f32, c.position.y as f32);
    ui.add_space(12.0);
    if c.kind != Kind::Text && ui.button("Rotate clockwise · R").clicked() {
        c.facing = c.facing.next();
    }
    if c != original {
        if placing {
            editor.template = c;
        } else {
            editor.update_property(c);
        }
    }
    if !active && !ui.ctx().any_popup_open() {
        editor.commit_properties();
    }
    if !placing {
        ui.horizontal(|ui| {
            if ui.button("Duplicate").clicked() {
                editor.duplicate();
            }
            if ui.button("Delete").clicked() {
                editor.delete();
            }
        });
    }
}

fn fields(ui: &mut Ui, c: &mut Component, placing: bool) -> bool {
    let mut active = false;
    if c.kind != Kind::Text {
        ui.label("Facing");
        egui::ComboBox::from_id_salt("facing")
            .selected_text(c.facing.name())
            .width(110.0)
            .show_ui(ui, |ui| {
                for facing in Facing::ALL {
                    ui.selectable_value(&mut c.facing, facing, facing.name());
                }
            });
        ui.end_row();
        ui.label("Data bits");
        let r = ui.add(egui::DragValue::new(&mut c.bits).range(1..=32));
        active |= r.has_focus() || r.dragged();
        ui.end_row();
    }
    if let Kind::Gate(gate) = c.kind
        && !gate.unary()
    {
        ui.label("Inputs");
        let r = ui.add(egui::DragValue::new(&mut c.inputs).range(2..=8));
        active |= r.has_focus() || r.dragged();
        ui.end_row();
    }
    if c.kind == Kind::Pin {
        ui.label("Output pin");
        ui.checkbox(&mut c.output, "");
        ui.end_row();
    }
    ui.label("Font size");
    let r = ui.add(egui::DragValue::new(&mut c.font_size).range(8..=48));
    active |= r.has_focus() || r.dragged();
    ui.end_row();
    ui.label("Color");
    ui.color_edit_button_srgb(&mut c.color);
    ui.end_row();
    if !placing {
        for (label, value) in [("X", &mut c.position.x), ("Y", &mut c.position.y)] {
            ui.label(label);
            let r = ui.add(
                egui::DragValue::new(value)
                    .speed(10.0)
                    .range(-1_000_000..=1_000_000),
            );
            active |= r.has_focus() || r.dragged();
            ui.end_row();
        }
    }
    active
}
