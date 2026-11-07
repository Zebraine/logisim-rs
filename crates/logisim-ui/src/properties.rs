use crate::editor::{Editor, Tool};
use crate::{i18n::tr, tf};
use egui::{Frame, RichText, ScrollArea, Ui};
use logisim_core::{
    Component, Extension, Facing, Gate, GateSize, HorizontalAlign, Kind, Point, Radix,
    SplitAppearance, VerticalAlign,
};

pub(crate) fn show(ui: &mut Ui, editor: &mut Editor, circuit_name: &str) {
    let placing = matches!(editor.tool, Tool::Place(_));
    if !editor.path.is_empty() {
        ui.weak(tr(
            "Instance inputs are driven by the parent circuit. Edits apply to all instances.",
        ));
    }
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
        tf!("Tool: {}", tr(editor.tool.name()))
    } else if let Some(c) = &target {
        tf!("Selection: {}", tr(c.kind.name()))
    } else if !editor.selection.is_empty() {
        tf!("Selection: {} item(s)", editor.selection.len())
    } else {
        tf!("Circuit: {}", circuit_name)
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
                    egui::CollapsingHeader::new(tf!(
                        "Simulation issues ({})",
                        editor.simulation.view.issues.len()
                    ))
                    .id_salt("simulation-issues")
                    .default_open(true)
                    .show(ui, |ui| {
                        for issue in editor.simulation.view.issues.iter().take(32) {
                            if ui
                                .button(&issue.message)
                                .on_hover_text(tr("Select affected components and wires"))
                                .clicked()
                            {
                                editor.selection = issue.targets.clone();
                            }
                        }
                        if editor.simulation.view.issues.len() > 32 {
                            ui.weak(tr("Showing the first 32 issues."));
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
                .add_enabled(editor.can_rotate(), egui::Button::new(tr("Rotate")))
                .clicked()
            {
                editor.rotate();
            }
            if ui.button(tr("Delete")).clicked() {
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
            ui.label(tf!("Start: {}, {}", w.a.x, w.a.y));
            ui.label(tf!("End: {}, {}", w.b.x, w.b.y));
            ui.label(tf!(
                "Connected wires: {}",
                editor.circuit.connected_wires(w.id).len()
            ));
            ui.weak(tr("Click or drag the wire to branch. Ctrl-drag moves; drag blue endpoint handles to reshape."));
            if let Some(signal) = editor.simulation.view.wires.get(&w.id) {
                ui.colored_label(signal.tone.color(), tf!("Signal: {}", signal.text));
                ui.monospace(&signal.binary);
            }
            ui.weak(tr(
                "The highlight follows the connected net, including matching tunnel labels.",
            ));
        }
        return;
    }
    ui.label(tf!(
        "{} components · {} wires",
        editor.circuit.components.len(),
        editor.circuit.wires.len()
    ));
    ui.add_space(10.0);
    if editor.tool == Tool::Wire {
        ui.weak(tr(
            "Click two points or drag to connect. Shift changes the bend. Esc cancels.",
        ));
    } else {
        ui.weak(tr(
            "Click or drag a wire to branch. Ctrl-click selects; Ctrl-drag moves.",
        ));
        ui.weak(tr(
            "Drag empty space to select. Shift adds to the selection.",
        ));
    }
    ui.add_space(10.0);
    editor.simulation.overview(ui);
    ui.add_space(8.0);
    ui.weak(tr("Use File to open or save the supported .circ subset."));
}

fn component_editor(ui: &mut Ui, editor: &mut Editor, mut c: Component, placing: bool) {
    if matches!(c.kind, Kind::Subcircuit(_)) {
        ui.strong(tf!("Circuit: {}", c.interface.name));
        if !placing && ui.button(tr("View instance")).clicked() {
            editor.open_instance = Some(c.id);
        }
        ui.weak(tf!(
            "{} interface pins · edit Pin components in the circuit definition",
            c.interface.ports.len()
        ));
    }
    if c.kind.is_memory()
        && (!placing || c.kind == Kind::Rom)
        && ui.button(tr("Edit contents…")).clicked()
    {
        editor.memory_target = Some((placing, c.id));
    }
    let original = c.clone();
    let mut active = false;
    if c.kind.has_label() {
        let response = if c.kind == Kind::Text {
            ui.label(tr("Text"));
            ui.add(
                egui::TextEdit::multiline(&mut c.label)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY)
                    .char_limit(4096)
                    .id_salt((c.id, "text")),
            )
        } else {
            ui.label(tr("Label"));
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
    }
    ui.add_space(8.0);
    egui::Grid::new(("component_properties", c.id))
        .num_columns(2)
        .spacing(egui::vec2(12.0, 9.0))
        .show(ui, |ui| {
            active |= fields(ui, &mut c, placing);
        });
    if c.position != original.position && c.kind != Kind::Text {
        c.position = Point::snapped(c.position.x as f32, c.position.y as f32);
    }
    ui.add_space(12.0);
    if c.kind.rotatable() && ui.button(tr("Rotate clockwise · R")).clicked() {
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
            if ui.button(tr("Duplicate")).clicked() {
                editor.duplicate();
            }
            if ui.button(tr("Delete")).clicked() {
                editor.delete();
            }
        });
    }
}

fn fields(ui: &mut Ui, c: &mut Component, placing: bool) -> bool {
    let mut active = false;
    if c.kind.rotatable() {
        ui.label(tr("Facing"));
        egui::ComboBox::from_id_salt("facing")
            .selected_text(tr(c.facing.name()))
            .width(110.0)
            .show_ui(ui, |ui| {
                for facing in Facing::ALL {
                    ui.selectable_value(&mut c.facing, facing, tr(facing.name()));
                }
            });
        ui.end_row();
    }
    if !matches!(
        c.kind,
        Kind::Text
            | Kind::Button
            | Kind::Led
            | Kind::SevenSegment
            | Kind::HexDisplay
            | Kind::Rom
            | Kind::Ram
            | Kind::Probe
            | Kind::Subcircuit(_)
            | Kind::Clock
            | Kind::FlipFlop(_)
            | Kind::Plexer(
                logisim_core::Plexer::Decoder
                    | logisim_core::Plexer::PriorityEncoder
                    | logisim_core::Plexer::DisplayDecoder
            )
    ) {
        ui.label(if c.kind == Kind::BitExtender {
            tr("Output bits")
        } else if c.kind == Kind::ShiftRegister {
            tr("Number of stages")
        } else {
            tr("Data bits")
        });
        let r = ui.add(egui::DragValue::new(&mut c.bits).range(1..=32));
        if r.changed() && c.kind == Kind::Counter {
            c.sequential.counter_max = u32::MAX >> (32 - c.bits);
        }
        if r.changed() && c.kind == Kind::Splitter {
            c.splitter.distribute(c.bits, c.splitter.descending);
        }
        if r.changed() && c.kind == Kind::Constant && c.bits < 32 {
            c.constant &= (1 << c.bits) - 1;
        }
        active |= r.has_focus() || r.dragged();
        ui.end_row();
    }
    active |= wiring_fields(ui, c);
    active |= crate::sequential::fields(ui, c);
    active |= crate::memory::fields(ui, c);
    crate::io::fields(ui, c);
    active |= crate::plexers::fields(ui, c);
    active |= crate::arithmetic::fields(ui, c);
    if let Kind::Gate(gate) = c.kind
        && !gate.unary()
    {
        ui.label(tr("Inputs"));
        let r = ui.add(egui::DragValue::new(&mut c.inputs).range(2..=8));
        active |= r.has_focus() || r.dragged();
        ui.end_row();
    }
    if let Kind::Gate(gate) = c.kind
        && gate != Gate::Buffer
    {
        ui.label(tr("Gate size"));
        egui::ComboBox::from_id_salt("gate_size")
            .selected_text(tr(c.gate_size.name()))
            .show_ui(ui, |ui| {
                for size in [GateSize::Narrow, GateSize::Medium, GateSize::Wide] {
                    if gate != Gate::Not || size != GateSize::Medium {
                        ui.selectable_value(&mut c.gate_size, size, tr(size.name()));
                    }
                }
            });
        ui.end_row();
    }
    if c.kind == Kind::Text {
        ui.label(tr("Horizontal"));
        egui::ComboBox::from_id_salt("text_halign")
            .selected_text(tr(&format!("{:?}", c.text_horizontal)))
            .show_ui(ui, |ui| {
                for a in [
                    HorizontalAlign::Left,
                    HorizontalAlign::Center,
                    HorizontalAlign::Right,
                ] {
                    ui.selectable_value(&mut c.text_horizontal, a, tr(&format!("{a:?}")));
                }
            });
        ui.end_row();
        ui.label(tr("Vertical"));
        egui::ComboBox::from_id_salt("text_valign")
            .selected_text(tr(&format!("{:?}", c.text_vertical)))
            .show_ui(ui, |ui| {
                for a in [
                    VerticalAlign::Top,
                    VerticalAlign::Center,
                    VerticalAlign::Baseline,
                    VerticalAlign::Bottom,
                ] {
                    ui.selectable_value(&mut c.text_vertical, a, tr(&format!("{a:?}")));
                }
            });
        ui.end_row();
    }
    if c.kind == Kind::Pin {
        ui.label(tr("Output pin"));
        ui.checkbox(&mut c.output, "");
        ui.end_row();
    }
    if c.kind.has_label() {
        ui.label(tr("Font size"));
        let r = ui.add(egui::DragValue::new(&mut c.font_size).range(8..=48));
        active |= r.has_focus() || r.dragged();
        ui.end_row();
        if c.kind != Kind::Tunnel {
            ui.label(tr("Color"));
            ui.color_edit_button_srgb(&mut c.color);
            ui.end_row();
        }
    }
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

fn wiring_fields(ui: &mut Ui, c: &mut Component) -> bool {
    let mut active = false;
    match c.kind {
        Kind::Subcircuit(_) | Kind::Clock | Kind::Button | Kind::Led => {
            ui.label(tr("Label location"));
            egui::ComboBox::from_id_salt("instance_label_location")
                .selected_text(tr(c.label_facing.name()))
                .show_ui(ui, |ui| {
                    for facing in Facing::ALL {
                        ui.selectable_value(&mut c.label_facing, facing, tr(facing.name()));
                    }
                });
            ui.end_row();
        }
        Kind::Constant => {
            ui.label(tr("Value (hex)"));
            let max = u32::MAX >> (32 - c.bits.clamp(1, 32));
            let r = ui.add(
                egui::DragValue::new(&mut c.constant)
                    .hexadecimal(usize::from(c.bits.div_ceil(4)), false, true)
                    .range(0..=max),
            );
            active |= r.has_focus() || r.dragged();
            ui.end_row();
        }
        Kind::Probe => {
            ui.label(tr("Radix"));
            egui::ComboBox::from_id_salt("probe_radix")
                .selected_text(tr(c.radix.name()))
                .show_ui(ui, |ui| {
                    for r in Radix::ALL {
                        ui.selectable_value(&mut c.radix, r, tr(r.name()));
                    }
                });
            ui.end_row();
            ui.label(tr("Label location"));
            egui::ComboBox::from_id_salt("probe_label")
                .selected_text(tr(c.label_facing.name()))
                .show_ui(ui, |ui| {
                    for f in Facing::ALL {
                        ui.selectable_value(&mut c.label_facing, f, tr(f.name()));
                    }
                });
            ui.end_row();
        }
        Kind::BitExtender => {
            ui.label(tr("Input bits"));
            let r = ui.add(egui::DragValue::new(&mut c.input_bits).range(1..=32));
            active |= r.has_focus() || r.dragged();
            ui.end_row();
            ui.label(tr("Extension"));
            egui::ComboBox::from_id_salt("extension")
                .selected_text(tr(c.extension.name()))
                .show_ui(ui, |ui| {
                    for e in Extension::ALL {
                        ui.selectable_value(&mut c.extension, e, tr(e.name()));
                    }
                });
            ui.end_row();
        }
        Kind::Splitter => {
            ui.label(tr("Fan out"));
            let r = ui.add(egui::DragValue::new(&mut c.splitter.fanout).range(1..=32));
            if r.changed() {
                c.splitter.distribute(c.bits, c.splitter.descending);
            }
            active |= r.has_focus() || r.dragged();
            ui.end_row();
            ui.label(tr("Appearance"));
            egui::ComboBox::from_id_salt("split_appearance")
                .selected_text(tr(c.splitter.appearance.name()))
                .show_ui(ui, |ui| {
                    for a in SplitAppearance::ALL {
                        ui.selectable_value(&mut c.splitter.appearance, a, tr(a.name()));
                    }
                });
            ui.end_row();
            ui.label(tr("Distribute"));
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(!c.splitter.descending, tr("Ascending"))
                    .clicked()
                {
                    c.splitter.distribute(c.bits, false);
                }
                if ui
                    .selectable_label(c.splitter.descending, tr("Descending"))
                    .clicked()
                {
                    c.splitter.distribute(c.bits, true);
                }
            });
            ui.end_row();
            for bit in 0..c.bits {
                ui.label(tf!("Bit {}", bit));
                let branch = &mut c.splitter.mapping[usize::from(bit)];
                egui::ComboBox::from_id_salt(("split_bit", bit))
                    .selected_text(
                        branch.map_or_else(|| tr("None").into(), |b| tf!("Branch {}", b)),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(branch, None, tr("None"));
                        for b in 0..c.splitter.fanout {
                            ui.selectable_value(branch, Some(b), tf!("Branch {}", b));
                        }
                    });
                ui.end_row();
            }
        }
        _ => {}
    }
    active
}
