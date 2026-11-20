use crate::editor::{Editor, Tool};
use crate::{i18n::tr, tf, theme};
use egui::{Frame, RichText, ScrollArea, Ui};
use logisim_core::{
    Component, Extension, Facing, Gate, GateSize, HorizontalAlign, Kind, Point, Radix,
    SplitAppearance, VerticalAlign,
};

pub(crate) fn show(
    ui: &mut Ui,
    editor: &mut Editor,
    circuit_name: &str,
    plugins: &[logisim_core::plugin::api::presentation::Library],
    files: &mut Vec<crate::FileAction>,
) {
    let placing = matches!(editor.tool, Tool::Place(_));
    let target = if placing {
        Some(editor.template.clone())
    } else if editor.session.selection.len() == 1 {
        editor
            .session
            .selection
            .first()
            .and_then(|id| editor.session.circuit.component(*id))
            .cloned()
    } else {
        None
    };
    let plugin = target.as_ref().and_then(|c| c.plugin.as_ref());
    let library = plugin.and_then(|p| {
        plugins
            .iter()
            .find(|l| l.manifest.id == p.plugin && l.manifest.version == p.version)
    });
    let title = match (&target, plugin) {
        (_, Some(p)) => {
            crate::plugins::text(library.map(|l| &l.presentation), &p.tool.name).to_owned()
        }
        (Some(c), _) if matches!(c.kind, Kind::Subcircuit(_)) => c.interface.name.clone(),
        (Some(c), _) => tr(c.kind.name()).to_owned(),
        _ if editor.session.selection.len() == 1 => tr("Wire").to_owned(),
        _ if !editor.session.selection.is_empty() => {
            tf!("Selection: {} item(s)", editor.session.selection.len())
        }
        _ => circuit_name.to_owned(),
    };
    let source = match (&target, plugin) {
        (_, Some(p)) => format!(
            "{} {}",
            library.map_or(tr("Missing plugin"), |l| crate::plugins::text(
                Some(&l.presentation),
                &l.manifest.name
            )),
            p.version
        ),
        (Some(c), _) if matches!(c.kind, Kind::Subcircuit(_)) => tr("Subcircuit").to_owned(),
        (Some(_), _) => tr("Built-in").to_owned(),
        _ if editor.session.selection.is_empty() => tr("Circuit").to_owned(),
        _ => tr("Selection").to_owned(),
    };
    let source_id = plugin.map(|p| p.plugin.clone());
    ScrollArea::vertical()
        .id_salt("inspector-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            Frame::NONE.inner_margin(8).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.spacing_mut().text_edit_width = (ui.available_width() * 0.45).max(72.0);
                ui.spacing_mut().combo_width = ui.spacing().text_edit_width;
                theme::section(ui, tr("Inspector"));
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(&title).size(15.0).strong());
                    let chip = theme::chip(ui, &source);
                    if let Some(id) = source_id {
                        chip.on_hover_text(id);
                    } else if !editor.session.context.path.is_empty() {
                        chip.on_hover_text(tr("Instance inputs are driven by the parent circuit. Edits apply to all instances."));
                    }
                });
                if let Some(c) = target {
                    if !placing && c.kind == Kind::Plugin {
                        if crate::plugins::has_runtime(&editor.simulation, &c, plugins) {
                            theme::card(ui, tr("Simulation"), |ui| {
                                crate::plugins::runtime(ui, &mut editor.simulation, &c, plugins, files)
                            });
                        }
                    } else if !placing && editor.simulation.has_component_controls(&c) {
                        theme::card(ui, tr("Simulation"), |ui| editor.simulation.component(ui, &c));
                    }
                    component_editor(ui, editor, c, placing, plugins);
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
                                editor.session.selection = issue.targets.clone();
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
    if !editor.session.selection.is_empty() {
        editor.commit_properties();
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
        if editor.session.selection.len() == 1
            && let Some(w) = editor
                .session
                .circuit
                .wires
                .iter()
                .find(|w| editor.session.selection.contains(&w.id))
        {
            ui.label(tf!("Start: {}, {}", w.a.x, w.a.y));
            ui.label(tf!("End: {}, {}", w.b.x, w.b.y));
            ui.label(tf!(
                "Connected wires: {}",
                editor.session.circuit.connected_wires(w.id).len()
            ));
            ui.weak(tr("Click or drag the wire to branch. Ctrl-drag moves; drag blue endpoint handles to reshape."));
            if let Some(signal) = editor.simulation.view.wires.get(&w.id) {
                ui.colored_label(
                    crate::theme::signal_text(ui.ctx(), signal.tone),
                    tf!("Signal: {}", signal.text),
                );
                ui.monospace(&signal.binary);
            }
            if editor.display.highlight_net {
                ui.weak(tr(
                    "The highlight follows the connected net, including matching tunnel labels.",
                ));
            }
        }
        return;
    }
    ui.label(tf!(
        "{} components · {} wires",
        editor.session.circuit.components.len(),
        editor.session.circuit.wires.len()
    ))
    .on_hover_text(tr(if editor.tool == Tool::Wire {
        "Click two points or drag to connect. Shift changes the bend. Esc cancels."
    } else {
        "Click or drag a wire to branch. Ctrl-click selects; Ctrl-drag moves."
    }));
    let mut label = editor.session.circuit.label.clone();
    let mut active = false;
    theme::card(ui, tr("Label & typography"), |ui| {
        property_grid(ui, "circuit_label_properties", |ui| {
            ui.label(tr("Circuit label"));
            active |= ui.text_edit_singleline(&mut label.text).has_focus();
            ui.end_row();
            active |= crate::fonts::properties(ui, &mut label.font);
            ui.label(tr("Label up"));
            egui::ComboBox::from_id_salt("circuit_label_up")
                .selected_text(tr(label.up.name()))
                .show_ui(ui, |ui| {
                    for facing in Facing::ALL {
                        ui.selectable_value(&mut label.up, facing, tr(facing.name()));
                    }
                });
            ui.end_row();
            ui.label(tr("Label color"));
            ui.color_edit_button_srgb(&mut label.color);
            ui.end_row();
        });
    });
    editor.update_circuit_label(label);
    if !active && !ui.ctx().any_popup_open() {
        editor.commit_properties();
    }
    theme::card(ui, tr("Simulation"), |ui| editor.simulation.overview(ui));
}

fn component_editor(
    ui: &mut Ui,
    editor: &mut Editor,
    mut c: Component,
    placing: bool,
    plugins: &[logisim_core::plugin::api::presentation::Library],
) {
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
    if c.kind == Kind::PlaRom {
        if ui.button(tr("Edit programming table…")).clicked() {
            editor.pla_target = Some((placing, c.id));
        }
        ui.weak(tr(
            "Resizing preserves the upper-left connections; removed cells are discarded.",
        ));
    }

    let original = c.clone();
    let mut active = false;
    if c.kind.has_label() {
        active |= property_grid(ui, ("label", c.id), |ui| {
            let response = if c.kind == Kind::Text {
                ui.label(tr("Text"));
                ui.add(
                    egui::TextEdit::multiline(&mut c.label)
                        .desired_rows(3)
                        .desired_width(ui.spacing().text_edit_width)
                        .char_limit(4096)
                        .id_salt((c.id, "text")),
                )
            } else {
                ui.label(tr("Label"));
                ui.add(
                    egui::TextEdit::singleline(&mut c.label)
                        .desired_width(ui.spacing().text_edit_width)
                        .char_limit(128)
                        .id_salt((c.id, "label")),
                )
            };
            if editor.focus_text {
                response.request_focus();
                editor.focus_text = false;
            }
            ui.end_row();
            response.has_focus()
        });
    }
    theme::card(
        ui,
        tr(if c.kind == Kind::Plugin {
            "Plugin"
        } else {
            "Geometry / behavior"
        }),
        |ui| {
            active |= property_grid(ui, ("component_properties", c.id), |ui| {
                fields(ui, &mut c, plugins)
            });
        },
    );
    if c.kind.has_label() || c.kind == Kind::Plugin {
        theme::card(ui, tr("Label & typography"), |ui| {
            active |= property_grid(ui, ("font", c.id), |ui| typography(ui, &mut c));
        });
    }
    if !placing && c.kind != Kind::Plugin {
        active |= property_grid(ui, ("position", c.id), |ui| position(ui, &mut c));
    }
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

fn property_grid<R>(ui: &mut Ui, id: impl egui::AsIdSalt, body: impl FnOnce(&mut Ui) -> R) -> R {
    let width = ui.available_width() - 8.0;
    ui.spacing_mut().text_edit_width = width * 0.52;
    ui.spacing_mut().combo_width = width * 0.52;
    egui::Grid::new(id)
        .num_columns(2)
        .min_col_width(width * 0.42)
        .max_col_width((ui.available_width() - 8.0) * 0.52)
        .spacing(egui::vec2(8.0, 4.0))
        .show(ui, body)
        .inner
}

fn fields(
    ui: &mut Ui,
    c: &mut Component,
    plugins: &[logisim_core::plugin::api::presentation::Library],
) -> bool {
    let mut active = false;
    if c.kind.rotatable() {
        ui.label(tr("Facing"));
        egui::ComboBox::from_id_salt("facing")
            .selected_text(tr(c.facing.name()))
            .width(ui.spacing().combo_width)
            .show_ui(ui, |ui| {
                for facing in Facing::ALL {
                    ui.selectable_value(&mut c.facing, facing, tr(facing.name()));
                }
            });
        ui.end_row();
    }
    if c.kind == Kind::Plugin {
        active |= crate::plugins::fields(ui, c, plugins);
        return active;
    }
    if !matches!(
        c.kind,
        Kind::Text
            | Kind::Button
            | Kind::DipSwitch
            | Kind::Led
            | Kind::SevenSegment
            | Kind::RgbLed
            | Kind::DotMatrix
            | Kind::HexDisplay
            | Kind::Keyboard
            | Kind::Tty
            | Kind::Rom
            | Kind::PlaRom
            | Kind::Ram
            | Kind::Probe
            | Kind::PullResistor
            | Kind::Subcircuit(_)
            | Kind::Clock
            | Kind::FlipFlop(_)
            | Kind::Latch(_)
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
        let range = if c.kind == Kind::Joystick {
            2..=5
        } else {
            1..=32
        };
        let r = ui.add(egui::DragValue::new(&mut c.bits).range(range));
        if r.changed() && c.kind == Kind::Slider {
            c.io.slider_value = c.io.slider_value.min(c.slider_max());
        }
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
    crate::latches::fields(ui, c);
    crate::bus::fields(ui, c);
    crate::transistors::fields(ui, c);
    active |= wiring_fields(ui, c);
    active |= crate::parity::fields(ui, c);
    active |= crate::sequential::fields(ui, c);
    active |= crate::memory::fields(ui, c);
    active |= crate::pla::fields(ui, c);
    active |= crate::io::fields(ui, c);
    active |= crate::plexers::fields(ui, c);
    active |= crate::arithmetic::fields(ui, c);
    if let Kind::Gate(gate) = c.kind
        && !gate.unary()
    {
        ui.label(tr("Inputs"));
        let r =
            ui.add(egui::DragValue::new(&mut c.gate_options.inputs).range(2..=Gate::MAX_INPUTS));
        active |= r.has_focus() || r.dragged();
        ui.end_row();
        if matches!(gate, Gate::Xor | Gate::Xnor) {
            ui.label(tr("XOR mode"));
            egui::ComboBox::from_id_salt("gate_xor_mode")
                .selected_text(tr(if c.gate_options.xor_odd {
                    "Odd parity"
                } else {
                    "Exactly one"
                }))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut c.gate_options.xor_odd, false, tr("Exactly one"));
                    ui.selectable_value(&mut c.gate_options.xor_odd, true, tr("Odd parity"));
                });
            ui.end_row();
        }
        for i in 0..c.input_count() {
            ui.label(tf!("Invert input {}", i + 1));
            let mut inverted = c.gate_negated(i);
            if ui
                .push_id(("gate_inversion", i), |ui| ui.checkbox(&mut inverted, ""))
                .inner
                .changed()
            {
                c.gate_options.negated ^= 1 << i;
            }
            ui.end_row();
        }
    }
    if let Kind::Gate(gate) = c.kind
        && gate != Gate::Buffer
    {
        ui.label(tr("Gate size"));
        egui::ComboBox::from_id_salt("gate_size")
            .selected_text(tr(c.gate_options.size.name()))
            .show_ui(ui, |ui| {
                for size in [GateSize::Narrow, GateSize::Medium, GateSize::Wide] {
                    if gate != Gate::Not || size != GateSize::Medium {
                        ui.selectable_value(&mut c.gate_options.size, size, tr(size.name()));
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
        if !c.output {
            ui.label(tr("Three-state input"));
            ui.checkbox(&mut c.pin.three_state, "");
            ui.end_row();
            ui.label(tr("Pull behavior"));
            egui::ComboBox::from_id_salt("pin_pull")
                .selected_text(tr(c.pin.pull.name()))
                .show_ui(ui, |ui| {
                    for pull in logisim_core::PinPull::ALL {
                        ui.selectable_value(&mut c.pin.pull, pull, tr(pull.name()));
                    }
                });
            ui.end_row();
        }
    }
    active
}

fn typography(ui: &mut Ui, c: &mut Component) -> bool {
    let active = crate::fonts::properties(ui, &mut c.font);
    if c.kind != Kind::Tunnel {
        ui.label(tr("Color"));
        if c.kind == Kind::Text {
            let mut rgba = [c.color[0], c.color[1], c.color[2], c.color_alpha];
            if ui.color_edit_button_srgba_unmultiplied(&mut rgba).changed() {
                c.color = [rgba[0], rgba[1], rgba[2]];
                c.color_alpha = rgba[3];
            }
        } else {
            ui.color_edit_button_srgb(&mut c.color);
        }
        ui.end_row();
    }
    active
}
fn position(ui: &mut Ui, c: &mut Component) -> bool {
    let mut active = false;
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
    active
}

fn wiring_fields(ui: &mut Ui, c: &mut Component) -> bool {
    let mut active = false;
    match c.kind {
        Kind::Subcircuit(_)
        | Kind::Pin
        | Kind::Clock
        | Kind::Button
        | Kind::Switch
        | Kind::DipSwitch
        | Kind::Slider
        | Kind::Led
        | Kind::RgbLed => {
            ui.label(tr("Label location"));
            egui::ComboBox::from_id_salt("instance_label_location")
                .selected_text(tr(if c.label_center {
                    "Center"
                } else {
                    c.label_facing.name()
                }))
                .show_ui(ui, |ui| {
                    for facing in Facing::ALL {
                        if ui
                            .selectable_label(
                                !c.label_center && c.label_facing == facing,
                                tr(facing.name()),
                            )
                            .clicked()
                        {
                            c.label_center = false;
                            c.label_facing = facing;
                        }
                    }
                    if c.kind == Kind::Button
                        && ui.selectable_label(c.label_center, tr("Center")).clicked()
                    {
                        c.label_center = true;
                        c.label_facing = Facing::North;
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
