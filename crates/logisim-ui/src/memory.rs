use crate::{
    SimulationAction, canvas::Canvas, editor::Editor, i18n::tr, simulation::SimulationView, tf,
};
use egui::{Align2, Color32, FontId, Painter, Rect, Stroke, StrokeKind, Ui};
use logisim_core::{Component, Id, Kind, MemoryInterface, MemoryRole, Point};

pub(crate) fn port_label(c: &Component, index: usize) -> &'static str {
    match c.memory_terminals()[index].role {
        MemoryRole::Data
            if c.kind == Kind::Ram && c.memory.interface != MemoryInterface::Separate =>
        {
            "Bidirectional data"
        }
        MemoryRole::Data => "Q",
        MemoryRole::Address => "Address",
        MemoryRole::Input => "D",
        MemoryRole::Select => "Chip select",
        MemoryRole::Clock => "Clock",
        MemoryRole::Load => "Load: 0 writes; otherwise reads",
        MemoryRole::Store => "Store: 0 holds data",
        MemoryRole::Clear => "Clear: reset to zero",
    }
}
pub(crate) fn fields(ui: &mut Ui, c: &mut Component) -> bool {
    if !c.kind.is_memory() {
        return false;
    }
    let before = (c.memory.address_bits, c.bits);
    let mut active = false;
    for (label, value, range) in [
        ("Address bits", &mut c.memory.address_bits, 2..=24),
        ("Data bits", &mut c.bits, 1..=32),
    ] {
        ui.label(tr(label));
        let r = ui.add(egui::DragValue::new(value).range(range));
        active |= r.has_focus() || r.dragged();
        ui.end_row();
    }
    if before != (c.memory.address_bits, c.bits) {
        c.memory.contents.resize(c.memory.address_bits, c.bits);
    }
    if c.kind == Kind::Ram {
        ui.label(tr("Data interface"));
        egui::ComboBox::from_id_salt("memory_interface")
            .width(135.0)
            .selected_text(tr(c.memory.interface.name()))
            .show_ui(ui, |ui| {
                for value in MemoryInterface::ALL {
                    ui.selectable_value(&mut c.memory.interface, value, tr(value.name()));
                }
            });
        ui.end_row();
    }
    ui.label(tr("Simplified mode"));
    ui.checkbox(&mut c.memory.simple, "");
    ui.end_row();
    ui.label(tr("Chip select active on"));
    ui.add_enabled_ui(c.memory_port(MemoryRole::Select).is_some(), |ui| {
        egui::ComboBox::from_id_salt("memory_select")
            .selected_text(tr(if c.memory.select_high {
                "High level"
            } else {
                "Low level"
            }))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut c.memory.select_high, false, tr("Low level"));
                ui.selectable_value(&mut c.memory.select_high, true, tr("High level"));
            });
    });
    ui.end_row();
    active
}
pub(crate) fn draw(
    p: &Painter,
    canvas: &Canvas,
    c: &Component,
    color: Color32,
    simulation: Option<&SimulationView>,
) {
    let at = |x, y| canvas.screen(c.position + Point::new(x, y));
    let z = canvas.camera.zoom;
    p.rect(
        Rect::from_min_max(at(-140, -40), at(0, 40)),
        3.0 * z,
        Color32::from_rgb(244, 248, 255),
        Stroke::new(1.6, color),
        StrokeKind::Inside,
    );
    let text = |x, y, align, label: String, size| {
        p.text(at(x, y), align, label, FontId::monospace(size * z), color);
    };
    text(
        -70,
        -32,
        Align2::CENTER_CENTER,
        format!("{} {} × {}", c.kind.name(), c.memory_capacity(), c.bits),
        7.0,
    );
    let contents = if c.kind == Kind::Rom {
        Some(&c.memory.contents)
    } else {
        simulation.and_then(|s| s.memories.get(&c.id))
    };
    let address = simulation
        .and_then(|s| s.ports.get(&(c.id, 1)))
        .and_then(|s| u32::from_str_radix(&s.binary, 2).ok())
        .unwrap_or(0);
    let start = (address / 4) * 4;
    for row in 0..4 {
        let a = start + row;
        if a >= c.memory_capacity() {
            break;
        }
        let y = -18 + row as i32 * 11;
        if a == address {
            p.rect_filled(
                Rect::from_min_max(at(-125, y - 5), at(-15, y + 5)),
                1.0,
                Color32::from_rgb(221, 234, 253),
            );
        }
        text(
            -120,
            y,
            Align2::LEFT_CENTER,
            format!(
                "{:0width$X}",
                a,
                width = c.memory.address_bits.div_ceil(4) as usize
            ),
            7.0,
        );
        text(
            -20,
            y,
            Align2::RIGHT_CENTER,
            format!(
                "{:0width$X}",
                contents.map_or(0, |v| v.get(a)),
                width = c.bits.div_ceil(4) as usize
            ),
            7.0,
        );
    }
    for port in c.memory_terminals() {
        use MemoryRole::*;
        let label = match port.role {
            Data => "D",
            Address => "A",
            Input => "D",
            Select => {
                if c.memory.select_high {
                    "CS"
                } else {
                    "/CS"
                }
            }
            Load => "LD",
            Store => "ST",
            Clear => "CLR",
            Clock => "",
        };
        let x = port.offset.x.clamp(-135, -5);
        let y = port.offset.y.clamp(-35, 34);
        text(x, y, Align2::CENTER_CENTER, label.into(), 5.0);
        if port.role == Clock {
            p.add(egui::Shape::line(
                vec![at(x - 4, 40), at(x, 35), at(x + 4, 40)],
                Stroke::new(1.3, color),
            ));
        }
    }
}

#[derive(Default)]
pub(crate) struct MemoryEditor {
    context: Option<(Id, Vec<Id>, bool, Id)>,
    address: u32,
    last: u32,
    value: String,
    page: u32,
}
impl MemoryEditor {
    pub fn show(&mut self, ctx: &egui::Context, editor: &mut Editor) {
        let Some((placing, id)) = editor.memory_target else {
            self.context = None;
            return;
        };
        let target = if placing {
            Some(editor.template.clone())
        } else {
            editor.circuit.component(id).cloned()
        };
        let Some(c) = target.filter(|c| c.kind.is_memory() && c.id == id) else {
            editor.memory_target = None;
            return;
        };
        editor.commit_properties();
        let context = (editor.active, editor.path.clone(), placing, id);
        if self.context.as_ref() != Some(&context) {
            self.context = Some(context);
            self.address = 0;
            self.last = 0;
            self.page = 0;
            self.value = String::new();
        }
        let capacity = c.memory_capacity();
        self.address = self.address.min(capacity - 1);
        self.last = self.last.clamp(self.address, capacity - 1);
        self.page = self.page.min((capacity - 1) / 32);
        let contents = if c.kind == Kind::Rom {
            c.memory.contents.clone()
        } else {
            editor
                .simulation
                .view
                .memories
                .get(&id)
                .cloned()
                .unwrap_or_default()
        };
        if self.value.is_empty() {
            self.value = format!("{:X}", contents.get(self.address));
        }
        let mut edit = None;
        let mut close = false;
        let modal=egui::Modal::new(egui::Id::unique("memory_contents_editor")).show(ctx,|ui| {
            ui.set_width(610.0);
            ui.heading(tr(if c.kind==Kind::Rom {"ROM contents"} else {"RAM contents"}));
            if !c.label.is_empty() {ui.label(&c.label);}
            ui.weak(tr(if c.kind==Kind::Rom {"ROM contents are saved with the circuit. Edits support Undo."} else {"RAM contents belong to this simulation instance. Reset clears them; circuit files do not save them."}));
            ui.label(tf!("{} words · {} bits per word",capacity,c.bits));
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(tr("Address (hex)"));
                let r=ui.add(egui::DragValue::new(&mut self.address).range(0..=capacity-1).hexadecimal(c.memory.address_bits.div_ceil(4) as usize,false,true));
                if r.changed() {
                    self.page=self.address/32;
                    self.last=self.address;
                    self.value=format!("{:X}",contents.get(self.address));
                }
                ui.label(tr("Value (hex)"));
                ui.add(egui::TextEdit::singleline(&mut self.value).id_salt("memory_word").desired_width(105.0).char_limit(8));
                let value=parse_word(&self.value,c.bits);
                if ui.add_enabled(value.is_some(),egui::Button::new(tr("Write word"))).clicked() {edit=Some((self.address,self.address+1,value.unwrap()));}
            });
            let value=parse_word(&self.value,c.bits);
            if value.is_none() {ui.colored_label(Color32::RED,tf!("Enter a hexadecimal value within {} bits.",c.bits));}
            ui.horizontal(|ui| {
                ui.label(tr("Through address (hex)"));
                ui.add(egui::DragValue::new(&mut self.last).range(self.address..=capacity-1).hexadecimal(c.memory.address_bits.div_ceil(4) as usize,false,true));
                if ui.add_enabled(value.is_some(),egui::Button::new(tr("Fill range"))).clicked() {edit=Some((self.address,self.last+1,value.unwrap()));}
                if ui.button(tr("Clear contents")).clicked() {edit=Some((0,capacity,0));}
            });
            ui.separator();
            egui::Grid::new("memory_words").min_col_width(105.0).striped(true).show(ui,|ui| {
                for row in 0..8 {
                    let base=self.page*32+row*4;
                    if base>=capacity {break;}
                    ui.monospace(format!("{:0width$X}",base,width=c.memory.address_bits.div_ceil(4) as usize));
                    for column in 0..4 {
                        let address=base+column;
                        if address<capacity && ui.selectable_label(self.address==address,egui::RichText::new(format!("{:0width$X}",contents.get(address),width=c.bits.div_ceil(4) as usize)).monospace()).clicked() {
                            self.address=address;self.last=address;self.value=format!("{:X}",contents.get(address));
                        }
                    }
                    ui.end_row();
                }
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(self.page>0,egui::Button::new(tr("Previous page"))).clicked() {self.page-=1;}
                ui.monospace(format!("{} / {}",self.page+1,capacity.div_ceil(32)));
                if ui.add_enabled((self.page+1)*32<capacity,egui::Button::new(tr("Next page"))).clicked() {self.page+=1;}
                if ui.button(tr("Close")).clicked() {close=true;}
            });
        });
        if let Some((start, end, value)) = edit {
            if c.kind == Kind::Rom {
                let mut changed = c;
                changed.memory.contents.fill(start, end, value);
                if placing {
                    editor.template = changed;
                } else {
                    editor.update_property(changed);
                    editor.commit_properties();
                }
            } else {
                editor
                    .simulation
                    .actions
                    .push(SimulationAction::FillMemory {
                        id,
                        start,
                        end,
                        value,
                    });
            }
        }
        if close || modal.should_close() {
            editor.memory_target = None;
            self.context = None;
        }
    }
}
fn parse_word(text: &str, bits: u8) -> Option<u32> {
    if text.is_empty() || !text.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(text, 16)
        .ok()
        .filter(|v| *v <= (u32::MAX >> (32 - bits.clamp(1, 32))))
}
