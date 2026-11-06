use crate::{
    appearance::{AppearanceEditor, Command, Target, Tool},
    editor::Editor,
};
use crate::{i18n::tr, tf};
use egui::{Frame, ScrollArea, Ui};
use logisim_core::{Appearance, ArtworkFont, Facing, Geometry, Kind, Paint, Point, TextAnchor};
use std::collections::BTreeMap;

fn point(ui: &mut Ui, p: &mut Point, grid: bool) {
    ui.horizontal(|ui| {
        ui.add(
            egui::DragValue::new(&mut p.x)
                .update_while_editing(false)
                .prefix("X ")
                .range(-1_000_000..=1_000_000)
                .speed(if grid { 10.0 } else { 1.0 }),
        );
        ui.add(
            egui::DragValue::new(&mut p.y)
                .update_while_editing(false)
                .prefix("Y ")
                .range(-1_000_000..=1_000_000)
                .speed(if grid { 10.0 } else { 1.0 }),
        );
    });
    if grid {
        *p = Point::snapped(p.x as f32, p.y as f32);
    }
}
fn paint(ui: &mut Ui, paint: &mut Paint, text: bool) {
    if !text {
        let mut enabled = paint.stroke.is_some();
        ui.checkbox(&mut enabled, tr("Outline"));
        if enabled {
            let mut color = paint.stroke.unwrap_or([0; 3]);
            ui.horizontal(|ui| {
                ui.color_edit_button_srgb(&mut color);
                ui.add(
                    egui::DragValue::new(&mut paint.width)
                        .update_while_editing(false)
                        .prefix(tr("Width "))
                        .range(1..=64),
                );
            });
            paint.stroke = Some(color);
        } else {
            paint.stroke = None;
        }
    }
    let mut enabled = paint.fill.is_some();
    ui.checkbox(
        &mut enabled,
        if text { tr("Text color") } else { tr("Fill") },
    );
    if enabled {
        let mut color = paint.fill.unwrap_or([235, 242, 255]);
        ui.color_edit_button_srgb(&mut color);
        paint.fill = Some(color);
    } else {
        paint.fill = None;
    }
}
fn font(ui: &mut Ui, font: &mut ArtworkFont, anchor: &mut TextAnchor) {
    ui.label(tr("Font family"));
    ui.text_edit_singleline(&mut font.family);
    ui.add(
        egui::DragValue::new(&mut font.size)
            .update_while_editing(false)
            .range(1..=96)
            .prefix(tr("Size ")),
    );
    ui.horizontal(|ui| {
        ui.checkbox(&mut font.bold, tr("Bold"));
        ui.checkbox(&mut font.italic, tr("Italic"));
    });
    egui::ComboBox::from_id_salt("appearance-text-align")
        .selected_text(match anchor {
            TextAnchor::Start => tr("Left"),
            TextAnchor::Middle => tr("Center"),
            TextAnchor::End => tr("Right"),
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(anchor, TextAnchor::Start, tr("Left"));
            ui.selectable_value(anchor, TextAnchor::Middle, tr("Center"));
            ui.selectable_value(anchor, TextAnchor::End, tr("Right"));
        });
}
impl AppearanceEditor {
    pub fn properties(&mut self, ui: &mut Ui, editor: &mut Editor) {
        ScrollArea::vertical().show(ui,|ui| { Frame::NONE.inner_margin(12).show(ui,|ui|{
            ui.strong(tf!("Appearance: {}", editor.project.circuit(editor.active).unwrap().name));
            ui.weak(tr("Changes update every instance. Wires keep their positions; reconnect moved ports in the circuit layout."));ui.separator();
            let mut a=Self::current(editor);let original=a.clone();
            let selection=(self.selected.len()==1).then(||*self.selected.first().unwrap());
            if self.tool!=Tool::Select {
                ui.strong(tr("Drawing style"));paint(ui,&mut self.paint,self.tool==Tool::Text);
                if self.tool==Tool::Text {font(ui,&mut self.font,&mut self.text_anchor);}
                if self.tool==Tool::Rounded{ui.add(egui::DragValue::new(&mut self.radius).update_while_editing(false).range(1..=10000).prefix(tr("Corner radius ")));}
                ui.weak(match self.tool {Tool::Polyline|Tool::Polygon=>tr("Click vertices. Double-click or Enter to finish. Escape cancels."),Tool::Curve=>tr("Drag the two endpoints, then click the control point."),Tool::Text=>tr("Click to add text, then edit its content here."),_=>tr("Drag to draw. Shift-click adds to a selection.")});
            } else if let Some(target)=selection {
                match target {
                    Target::Shape(id)=>if let Some(shape)=a.shapes.iter_mut().find(|s|s.id==id){
                        ui.strong(tr(shape.geometry.name()));
                        let is_text=matches!(shape.geometry,Geometry::Text {..});paint(ui,&mut shape.paint,is_text);
                        match &mut shape.geometry {
                            Geometry::Text {at,text,font:f,anchor}=>{
                                ui.label(tr("Text"));let response=ui.text_edit_singleline(text);
                                if self.focus_text {response.request_focus();self.focus_text=false;}
                                font(ui,f,anchor);point(ui,at,false);
                            },
                            Geometry::Rect {a,b,radius}=>{ui.label(tr("First corner"));point(ui,a,false);ui.label(tr("Opposite corner"));point(ui,b,false);ui.add(egui::DragValue::new(radius).update_while_editing(false).range(0..=10000).prefix(tr("Corner radius ")));},
                            Geometry::Ellipse {a,b}=>{ui.label(tr("First corner"));point(ui,a,false);ui.label(tr("Opposite corner"));point(ui,b,false);},
                            Geometry::Curve {a,control,b}=>{ui.label(tr("Start"));point(ui,a,false);ui.label(tr("Control"));point(ui,control,false);ui.label(tr("End"));point(ui,b,false);},
                            Geometry::Poly {points,closed}=>{
                                ui.add_enabled(points.len()>=3,egui::Checkbox::new(closed,tr("Closed polygon")));
                                for (index,at) in points.iter_mut().enumerate(){ui.push_id(index,|ui|{if ui.selectable_label(self.vertex==Some((id,index)),tf!("Vertex {}", index+1)).clicked(){self.vertex=Some((id,index));}point(ui,at,false);});}
                            }
                        }
                    },
                    Target::Port(id)=>if let Some(pin)=editor.circuit.component(id) {
                        ui.strong(if pin.label.is_empty(){tf!("Pin ({}, {})", pin.position.x, pin.position.y)}else{pin.label.clone()});
                        ui.label(tf!("{} · {} bit(s)", tr(if pin.output{"Output"}else{"Input"}), pin.bits));
                        ui.label(tf!("Internal Pin: ({}, {})", pin.position.x, pin.position.y));
                        if let Some(at)=a.ports.get_mut(&id){point(ui,at,true);}
                        ui.weak(tr("Move this port to arrange the package. Edit or remove its Pin in the circuit layout."));
                    },
                    Target::Anchor=>{
                        ui.strong(tr("Anchor"));point(ui,&mut a.anchor,true);
                        egui::ComboBox::from_id_salt("appearance-facing").selected_text(tr(a.facing.name())).show_ui(ui,|ui|{for f in Facing::ALL{ui.selectable_value(&mut a.facing,f,tr(f.name()));}});
                        ui.weak(tr("The anchor sets the placement origin and the artwork's original facing."));
                    }
                }
            } else {
                ui.label(tf!("{} shape(s) · {} port(s)", a.shapes.len(), a.ports.len()));
                ui.weak(tr("Select, drag or use arrow keys to move. Drag shape handles to reshape. Ports and anchor stay on the circuit grid."));
            }
            if a!=original && a.shapes.iter().all(|s|s.geometry.valid()) {editor.update_appearance(a);}
            if !ui.ctx().egui_wants_keyboard_input()&&!ui.input(|i|i.pointer.any_down()){editor.commit_properties();}
            if self.selected.iter().any(|t|matches!(t,Target::Shape(_))) {
                ui.separator();ui.horizontal_wrapped(|ui|{
                    for (label,cmd) in [("Raise",Command::Raise),("Lower",Command::Lower),("Front",Command::Front),("Send to Back",Command::Back),("Duplicate",Command::Duplicate),("Delete",Command::Delete)] {if ui.button(tr(label)).clicked(){self.command(editor,cmd);}}
                });
                if self.can_edit_vertex(editor,false)||self.can_edit_vertex(editor,true) {
                    ui.horizontal(|ui| {
                        if ui.add_enabled(self.can_edit_vertex(editor,false),egui::Button::new(tr("Add vertex"))).clicked() {self.command(editor,Command::AddVertex);}
                        if ui.add_enabled(self.can_edit_vertex(editor,true),egui::Button::new(tr("Remove vertex"))).clicked() {self.command(editor,Command::RemoveVertex);}
                    });
                }
            }
            ui.separator();
            egui::CollapsingHeader::new(tr("Package design")).id_salt("package-design").default_open(selection.is_none()).show(ui,|ui|{
                ui.add(egui::DragValue::new(&mut self.package_width).update_while_editing(false).range(80..=1000).speed(10).prefix(tr("Body width ")));
                ui.add(egui::DragValue::new(&mut self.package_pitch).update_while_editing(false).range(10..=100).speed(10).prefix(tr("Port spacing ")));
                self.package_width=(self.package_width/10).max(8)*10;self.package_pitch=(self.package_pitch/10).max(1)*10;
                ui.checkbox(&mut self.package_labels,tr("Include pin names"));
                if ui.button(tr("Generate package")).on_hover_text(tr("Replace the artwork with a labeled package. You can undo this change.")).clicked(){self.generate_package(editor);}
            });
            egui::CollapsingHeader::new(tr("Ports")).id_salt("appearance-ports").show(ui,|ui|{
                for pin in editor.circuit.components.clone().iter().filter(|p|p.kind==Kind::Pin) {
                    let label=if pin.label.is_empty(){tf!("Pin ({}, {})", pin.position.x, pin.position.y)}else{pin.label.clone()};
                    if ui.selectable_label(self.selected.contains(&Target::Port(pin.id)),format!("{} {} [{}]",if pin.output{"OUT"}else{"IN"},label,pin.bits)).clicked(){editor.commit_properties();self.selected=BTreeSet::from([Target::Port(pin.id)]);self.tool=Tool::Select;}
                }
                if ui.selectable_label(self.selected.contains(&Target::Anchor),tr("Anchor")).clicked(){editor.commit_properties();self.selected=BTreeSet::from([Target::Anchor]);self.tool=Tool::Select;}
            });
            if ui.add_enabled(editor.circuit.appearance.is_some(),egui::Button::new(tr("Revert to default appearance"))).clicked(){self.revert(editor);}
        });
        });
    }
    pub fn generate_package(&mut self, editor: &mut Editor) {
        self.fit_requested = true;
        self.cancel();
        editor.commit_properties();
        let before = editor.snapshot();
        let width = self.package_width.max(80);
        let pitch = self.package_pitch.max(10);
        let mut inputs: Vec<_> = editor
            .circuit
            .components
            .iter()
            .filter(|p| p.kind == Kind::Pin && !p.output)
            .collect();
        let mut outputs: Vec<_> = editor
            .circuit
            .components
            .iter()
            .filter(|p| p.kind == Kind::Pin && p.output)
            .collect();
        inputs.sort_by_key(|p| (p.position.y, p.position.x, p.id));
        outputs.sort_by_key(|p| (p.position.y, p.position.x, p.id));
        let height = 40 + (inputs.len().max(outputs.len()).max(1) as i32) * pitch;
        let mut a = Appearance {
            shapes: vec![],
            ports: BTreeMap::new(),
            anchor: Point::ZERO,
            facing: Facing::East,
        };
        a.add(
            Geometry::Rect {
                a: Point::ZERO,
                b: Point::new(width, height),
                radius: 8,
            },
            Paint {
                stroke: Some([51, 65, 85]),
                fill: Some([241, 245, 251]),
                width: 2,
            },
        );
        let text_paint = Paint {
            stroke: None,
            fill: Some([51, 65, 85]),
            width: 1,
        };
        let title = editor.project.circuit(editor.active).unwrap().name.clone();
        a.add(
            Geometry::Text {
                at: Point::new(width / 2, 20),
                text: title,
                font: ArtworkFont {
                    bold: true,
                    ..Default::default()
                },
                anchor: TextAnchor::Middle,
            },
            text_paint.clone(),
        );
        for (right, pins) in [(false, inputs), (true, outputs)] {
            for (index, pin) in pins.into_iter().enumerate() {
                let at = Point::new(if right { width } else { 0 }, 40 + index as i32 * pitch);
                a.ports.insert(pin.id, at);
                if self.package_labels {
                    let label = if pin.label.is_empty() {
                        format!("{}{}", if right { "OUT" } else { "IN" }, index + 1)
                    } else {
                        pin.label.clone()
                    };
                    a.add(
                        Geometry::Text {
                            at: Point::new(if right { width - 8 } else { 8 }, at.y + 4),
                            text: label,
                            font: ArtworkFont {
                                size: 10,
                                ..Default::default()
                            },
                            anchor: if right {
                                TextAnchor::End
                            } else {
                                TextAnchor::Start
                            },
                        },
                        text_paint.clone(),
                    );
                }
            }
        }
        editor.circuit.appearance = Some(a);
        editor.record(before);
        self.selected.clear();
        self.tool = Tool::Select;
    }
}
use std::collections::BTreeSet;
