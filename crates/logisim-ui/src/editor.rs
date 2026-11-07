//! Editing transactions and gestures. Simulation is deliberately separate.
use crate::{i18n::tr, tf};

use crate::{
    canvas::Canvas,
    drawing,
    simulation::{SimulationAction, SimulationUi},
};

use egui::{Key, Modifiers, PointerButton, Pos2, Rect, Response, Ui, Vec2};
use logisim_core::{Circuit, Component, GRID, Gate, Id, Kind, Point, Project};
use std::collections::BTreeSet;

const CLIPBOARD_MARKER: &str = "logisim-rs selection (paste into the same window)";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Tool {
    #[default]
    Select,
    Wire,
    Poke,
    Place(Kind),
}
impl Tool {
    pub fn name(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Wire => "Wire",
            Self::Poke => "Poke",
            Self::Place(kind) => kind.name(),
        }
    }
}
#[derive(Clone)]
pub(crate) struct Snapshot {
    pub appearance_mode: bool,
    pub project: Project,
    pub active: Id,
    pub root: Id,
    pub path: Vec<Id>,
    pub circuit: Circuit,
    pub selection: BTreeSet<Id>,
}
struct Edit {
    before: Snapshot,
    after: Snapshot,
}
enum Gesture {
    Slider {
        id: Id,
        before: Snapshot,
    },
    Move {
        start: Point,
        before: Snapshot,
    },
    Box {
        start: Pos2,
        current: Pos2,
        previous: BTreeSet<Id>,
    },
    Wire {
        start: Point,
        end: Point,
        horizontal: bool,
    },
    Endpoint {
        before: Snapshot,
        fixed: Point,
        id: Id,
        horizontal: bool,
    },
}

pub(crate) struct Editor {
    pub appearance_mode: bool,
    pub project: Project,
    pub active: Id,
    pub root: Id,
    pub path: Vec<Id>,
    pub open_instance: Option<Id>,
    pub project_error: Option<String>,
    pub circuit: Circuit,
    pub selection: BTreeSet<Id>,
    pub tool: Tool,
    pub template: Component,
    pub memory_target: Option<(bool, Id)>,
    pub focus_text: bool,
    pub simulation: SimulationUi,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    clipboard: Option<Circuit>,
    publish_clipboard: bool,
    paste_count: i32,
    gesture: Option<Gesture>,
    wire_start: Option<Point>,
    property_before: Option<Snapshot>,
}
impl Default for Editor {
    fn default() -> Self {
        Self {
            appearance_mode: false,
            project: Project::default(),
            active: 1,
            root: 1,
            path: vec![],
            open_instance: None,
            project_error: None,
            circuit: Circuit::default(),
            selection: BTreeSet::new(),
            tool: Tool::Select,
            template: Component::new(Kind::Gate(Gate::And), Point::ZERO),
            memory_target: None,
            focus_text: false,
            simulation: SimulationUi::default(),
            undo: vec![],
            redo: vec![],
            clipboard: None,
            publish_clipboard: false,
            paste_count: 0,
            gesture: None,
            wire_start: None,
            property_before: None,
        }
    }
}
impl Editor {
    pub fn snapshot(&self) -> Snapshot {
        let mut project = self.project.clone();
        if let Some(definition) = project.circuit_mut(self.active) {
            definition.circuit = self.circuit.clone();
        }
        Snapshot {
            appearance_mode: self.appearance_mode,
            project,
            active: self.active,
            root: self.root,
            path: self.path.clone(),
            circuit: self.circuit.clone(),
            selection: self.selection.clone(),
        }
    }
    fn restore(&mut self, mut state: Snapshot) {
        self.appearance_mode = state.appearance_mode;
        state.project.preserve_allocations(&self.project);
        self.project = state.project;
        self.active = state.active;
        self.root = state.root;
        self.path = state.path;
        self.circuit = state.circuit;
        self.selection = state.selection;
        if let Tool::Place(Kind::Subcircuit(id)) = self.tool
            && !self.project.can_instantiate(self.active, id)
        {
            self.tool = Tool::Select;
        }
    }
    pub fn record(&mut self, before: Snapshot) {
        self.sync_project();
        for definition in &mut self.project.circuits {
            if before.project.circuit(definition.id).map(|d| &d.circuit)
                != Some(&definition.circuit)
            {
                let mapped = definition.circuit.normalize_wires();
                if definition.id == self.active {
                    self.selection = self
                        .selection
                        .iter()
                        .flat_map(|id| {
                            mapped.get(id).cloned().unwrap_or_else(|| {
                                if definition.circuit.component(*id).is_some() {
                                    BTreeSet::from([*id])
                                } else {
                                    BTreeSet::new()
                                }
                            })
                        })
                        .collect();
                }
            }
        }
        self.circuit = self.project.circuit(self.active).unwrap().circuit.clone();
        if before.project != self.project || before.circuit != self.circuit {
            self.undo.push(Edit {
                before,
                after: self.snapshot(),
            });
            if self.undo.len() > 100 {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
    }
    pub fn commit_properties(&mut self) {
        if let Some(before) = self.property_before.take() {
            self.record(before);
        }
    }
    pub fn update_appearance(&mut self, appearance: logisim_core::Appearance) {
        if self.circuit.appearance.as_ref() == Some(&appearance) {
            return;
        }
        if self.property_before.is_none() {
            self.property_before = Some(self.snapshot());
        }
        self.circuit.appearance = Some(appearance);
    }
    pub fn update_property(&mut self, component: Component) {
        if self.circuit.component(component.id) == Some(&component) {
            return;
        }
        if self.property_before.is_none() {
            self.property_before = Some(self.snapshot());
        }
        self.circuit.replace_component(component);
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.property_before.is_some()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty() && self.property_before.is_none()
    }
    pub fn can_paste(&self) -> bool {
        self.clipboard
            .as_ref()
            .is_some_and(|c| self.allowed_components(c))
    }
    fn allowed_components(&self, circuit: &Circuit) -> bool {
        circuit.components.iter().all(|c| !matches!(c.kind,Kind::Subcircuit(id) if !self.project.can_instantiate(self.active,id)))
    }
    pub fn can_rotate(&self) -> bool {
        if let Tool::Place(kind) = self.tool {
            kind.rotatable()
        } else {
            self.circuit
                .components
                .iter()
                .any(|c| self.selection.contains(&c.id) && c.kind.rotatable())
        }
    }
    pub fn undo(&mut self) {
        self.cancel_gesture();
        self.commit_properties();
        if let Some(edit) = self.undo.pop() {
            self.restore(edit.before.clone());
            self.redo.push(edit);
        }
    }
    pub fn redo(&mut self) {
        self.cancel_gesture();
        self.commit_properties();
        if let Some(edit) = self.redo.pop() {
            self.restore(edit.after.clone());
            self.undo.push(edit);
        }
    }
    pub fn cancel_slider(&mut self) {
        if matches!(self.gesture, Some(Gesture::Slider { .. })) {
            self.cancel_gesture();
        }
    }
    pub fn cancel_gesture(&mut self) {
        if let Some(
            Gesture::Move { before, .. }
            | Gesture::Endpoint { before, .. }
            | Gesture::Slider { before, .. },
        ) = self.gesture.take()
        {
            self.restore(before);
        }
        self.wire_start = None;
    }
    pub fn set_tool(&mut self, tool: Tool) {
        self.simulation.release_button();
        self.cancel_gesture();
        self.commit_properties();
        self.tool = tool;
        if let Tool::Place(kind) = tool {
            if self.template.kind != kind {
                self.template = if let Kind::Subcircuit(id) = kind {
                    self.project
                        .instance(id, Point::ZERO)
                        .unwrap_or_else(|| Component::new(kind, Point::ZERO))
                } else {
                    Component::new(kind, Point::ZERO)
                };
            }
            self.selection.clear();
        }
    }
    pub fn delete(&mut self) {
        self.cancel_gesture();
        self.commit_properties();
        let before = self.snapshot();
        self.circuit.remove(&self.selection);
        self.selection.clear();
        self.record(before);
    }
    pub fn select_all(&mut self) {
        self.cancel_gesture();
        self.selection = self.circuit.ids();
        self.tool = Tool::Select;
    }
    fn selected_copy(&self) -> Circuit {
        let mut clip = self.circuit.clone();
        clip.components.retain(|c| self.selection.contains(&c.id));
        clip.wires.retain(|w| self.selection.contains(&w.id));
        clip
    }
    pub fn copy(&mut self) {
        self.commit_properties();
        if !self.selection.is_empty() {
            self.clipboard = Some(self.selected_copy());
            self.publish_clipboard = true;
            self.paste_count = 0;
        }
    }
    pub fn cut(&mut self) {
        self.copy();
        self.delete();
    }
    fn paste_circuit(&mut self, clip: Circuit, offset: Point) {
        if !self.allowed_components(&clip) {
            self.project_error =
                Some("Pasting would create a missing or recursive subcircuit.".into());
            return;
        }
        self.cancel_gesture();
        self.commit_properties();
        let before = self.snapshot();
        self.selection.clear();
        for mut c in clip.components {
            c.position = c.position + offset;
            self.selection.insert(self.circuit.add_component(c));
        }
        for w in clip.wires {
            if let Some(id) = self.circuit.add_wire(w.a + offset, w.b + offset) {
                self.selection.insert(id);
            }
        }
        self.tool = Tool::Select;
        self.record(before);
    }
    pub fn paste(&mut self) {
        if let Some(clip) = self.clipboard.clone() {
            self.paste_count += 1;
            self.paste_circuit(
                clip,
                Point::new(self.paste_count * GRID, self.paste_count * GRID),
            );
        }
    }
    pub fn duplicate(&mut self) {
        if !self.selection.is_empty() {
            self.paste_circuit(self.selected_copy(), Point::new(GRID, GRID));
        }
    }
    pub fn rotate(&mut self) {
        if !self.can_rotate() {
            return;
        }
        if let Tool::Place(_) = self.tool {
            self.template.facing = self.template.facing.next();
            return;
        }
        self.cancel_gesture();
        self.commit_properties();
        let before = self.snapshot();
        let changes: Vec<_> = self
            .circuit
            .components
            .iter()
            .filter(|c| self.selection.contains(&c.id) && c.kind.rotatable())
            .cloned()
            .collect();
        for mut c in changes {
            c.facing = c.facing.next();
            self.circuit.replace_component(c);
        }
        self.record(before);
    }
    fn nudge(&mut self, delta: Point) {
        self.commit_properties();
        let before = self.snapshot();
        self.circuit.move_selection(&self.selection, delta);
        self.record(before);
    }
    pub fn place(&mut self, at: Point) {
        if let Kind::Subcircuit(id) = self.template.kind
            && !self.project.can_instantiate(self.active, id)
        {
            self.project_error =
                Some("A circuit cannot contain itself, directly or indirectly.".into());
            return;
        }
        self.commit_properties();
        let before = self.snapshot();
        let mut c = self.template.clone();
        c.position = at;
        self.focus_text = c.kind == Kind::Text;
        self.selection = BTreeSet::from([self.circuit.add_component(c)]);
        self.tool = Tool::Select;
        self.record(before);
    }
    fn add_route(&mut self, a: Point, b: Point, horizontal: bool) {
        let before = self.snapshot();
        self.selection = self
            .circuit
            .add_route(a, b, horizontal)
            .into_iter()
            .collect();
        self.record(before);
    }
    fn port_at(&self, world: Vec2, tolerance: f32) -> Option<Point> {
        self.circuit
            .components
            .iter()
            .flat_map(Component::ports)
            .filter(|p| (Vec2::new(p.x as f32, p.y as f32) - world).length() <= tolerance)
            .min_by(|a, b| {
                let dist = |p: &Point| (Vec2::new(p.x as f32, p.y as f32) - world).length_sq();
                dist(a).total_cmp(&dist(b))
            })
    }
    fn wire_at(&self, world: Vec2, tolerance: f32) -> Option<Id> {
        self.circuit
            .wires
            .iter()
            .rev()
            .find(|w| drawing::wire_distance(**w, world) <= tolerance)
            .map(|w| w.id)
    }
    fn hit(&self, world: Vec2, tolerance: f32) -> Option<Id> {
        self.circuit
            .components
            .iter()
            .rev()
            .find(|c| {
                drawing::bounds(c)
                    .expand(tolerance * 0.3)
                    .contains(world.to_pos2())
            })
            .map(|c| c.id)
            .or_else(|| self.wire_at(world, tolerance))
    }
    fn snap(&self, world: Vec2, tolerance: f32) -> Point {
        self.port_at(world, tolerance)
            .unwrap_or(Point::snapped(world.x, world.y))
    }
    fn connection_point(&self, world: Vec2, tolerance: f32) -> Point {
        if let Some(port) = self.port_at(world, tolerance) {
            return port;
        }
        let grid = Point::snapped(world.x, world.y);
        if let Some(id) = self.wire_at(world, tolerance)
            && let Some(w) = self.circuit.wires.iter().find(|w| w.id == id)
        {
            // Imported wires need not lie on the grid. Keep the anchor on the actual wire.
            if w.horizontal() {
                return Point::new(grid.x.clamp(w.a.x.min(w.b.x), w.a.x.max(w.b.x)), w.a.y);
            }
            return Point::new(w.a.x, grid.y.clamp(w.a.y.min(w.b.y), w.a.y.max(w.b.y)));
        }
        grid
    }

    pub fn show(&mut self, ui: &mut Ui, canvas: &mut Canvas, response: &Response) {
        let keyboard = ui.is_enabled()
            && !ui.ctx().any_popup_open()
            && (response.has_focus()
                || (response.hovered() && !ui.ctx().egui_wants_keyboard_input()));
        if keyboard {
            self.shortcuts(ui);
        }
        let space = ui.input(|i| i.key_down(Key::Space));
        if space {
            self.cancel_gesture();
        }
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let tolerance = 7.0 / canvas.camera.zoom;
        if space {
            self.simulation.release_button();
        }
        if ui.is_enabled() && !space && !ui.ctx().any_popup_open() {
            if self.tool == Tool::Poke
                && response.hovered()
                && ui.input(|i| i.focused && i.pointer.primary_pressed())
                && let Some(pos) = pointer
                && let Some(id) = self.hit(canvas.world(pos), tolerance)
                && self
                    .circuit
                    .component(id)
                    .is_some_and(|c| c.kind == Kind::Button)
            {
                self.selection.clear();
                self.selection.insert(id);
                self.simulation.press_button(id);
            }
            if response.drag_started_by(PointerButton::Primary) {
                self.commit_properties();
                if let Some(start) = ui.input(|i| i.pointer.press_origin()) {
                    let world = canvas.world(start);
                    let snap = self.snap(world, tolerance);
                    let hit = self.hit(world, tolerance);
                    let ctrl = ui.input(|i| i.modifiers.ctrl);
                    let hit_wire = hit.is_some_and(|id| self.circuit.component(id).is_none());
                    let selected_endpoint = self
                        .circuit
                        .wires
                        .iter()
                        .filter(|w| self.selection.contains(&w.id))
                        .find_map(|w| {
                            if (canvas.screen(w.a) - start).length() <= 7.0 {
                                Some((*w, w.b))
                            } else if (canvas.screen(w.b) - start).length() <= 7.0 {
                                Some((*w, w.a))
                            } else {
                                None
                            }
                        });
                    if self.tool == Tool::Poke {
                        if let Some(id) = hit
                            && let Some(c) = self.circuit.component(id)
                            && c.kind == Kind::Slider
                            && (crate::io::slider_knob(c) - world).length() <= tolerance
                        {
                            self.selection.clear();
                            self.selection.insert(id);
                            self.gesture = Some(Gesture::Slider {
                                id,
                                before: self.snapshot(),
                            });
                        }
                    } else if self.tool == Tool::Wire
                        || (self.tool == Tool::Select
                            && !ctrl
                            && (self.wire_start.is_some()
                                || self.port_at(world, tolerance).is_some()
                                || (hit_wire && selected_endpoint.is_none())))
                    {
                        let start = self
                            .wire_start
                            .take()
                            .unwrap_or_else(|| self.connection_point(world, tolerance));
                        self.gesture = Some(Gesture::Wire {
                            start,
                            end: start,
                            horizontal: true,
                        });
                    } else if self.tool == Tool::Select {
                        self.wire_start = None;
                        if let Some((wire, fixed)) = selected_endpoint.filter(|_| !ctrl) {
                            self.gesture = Some(Gesture::Endpoint {
                                before: self.snapshot(),
                                fixed,
                                id: wire.id,
                                horizontal: wire.horizontal(),
                            });
                        } else if let Some(id) = hit {
                            if !self.selection.contains(&id) {
                                if !ui.input(|i| i.modifiers.shift) {
                                    self.selection.clear();
                                }
                                self.selection.insert(id);
                            }
                            self.gesture = Some(Gesture::Move {
                                start: snap,
                                before: self.snapshot(),
                            });
                        } else {
                            self.gesture = Some(Gesture::Box {
                                start: world.to_pos2(),
                                current: world.to_pos2(),
                                previous: if ui.input(|i| i.modifiers.shift) {
                                    self.selection.clone()
                                } else {
                                    BTreeSet::new()
                                },
                            });
                        }
                    }
                }
            }
            if let Some(pos) = pointer {
                let world = canvas.world(pos);
                let snap = self.connection_point(world, tolerance);
                if let Some(gesture) = &mut self.gesture {
                    match gesture {
                        Gesture::Slider { id, .. } => {
                            if let Some(c) =
                                self.circuit.components.iter_mut().find(|c| c.id == *id)
                            {
                                crate::io::move_slider(c, world);
                            }
                        }
                        Gesture::Move { start, before } => {
                            self.circuit = before.circuit.clone();
                            // Movement snaps to the grid, never to the component being moved.
                            self.circuit.move_selection(
                                &before.selection,
                                Point::snapped(world.x, world.y) - *start,
                            );
                        }
                        Gesture::Box {
                            start,
                            current,
                            previous,
                        } => {
                            *current = world.to_pos2();
                            let rect = Rect::from_two_pos(*start, *current);
                            self.selection = previous.clone();
                            self.selection.extend(
                                self.circuit
                                    .components
                                    .iter()
                                    .filter(|c| rect.intersects(drawing::bounds(c)))
                                    .map(|c| c.id),
                            );
                            self.selection.extend(
                                self.circuit
                                    .wires
                                    .iter()
                                    .filter(|w| {
                                        rect.intersects(
                                            Rect::from_two_pos(
                                                drawing::pos(w.a),
                                                drawing::pos(w.b),
                                            )
                                            .expand(1.0),
                                        )
                                    })
                                    .map(|w| w.id),
                            );
                        }
                        Gesture::Wire {
                            end, horizontal, ..
                        } => {
                            *end = snap;
                            *horizontal = !ui.input(|i| i.modifiers.shift);
                        }
                        Gesture::Endpoint {
                            before,
                            fixed,
                            id,
                            horizontal,
                        } => {
                            self.circuit = before.circuit.clone();
                            self.circuit.wires.retain(|w| w.id != *id);
                            self.selection = self
                                .circuit
                                .add_route(*fixed, snap, *horizontal)
                                .into_iter()
                                .collect();
                        }
                    }
                }
            }
            if response.drag_stopped_by(PointerButton::Primary) {
                match self.gesture.take() {
                    Some(
                        Gesture::Move { before, .. }
                        | Gesture::Endpoint { before, .. }
                        | Gesture::Slider { before, .. },
                    ) => self.record(before),
                    Some(Gesture::Wire {
                        start,
                        end,
                        horizontal,
                    }) if pointer.is_some_and(|p| canvas.rect.contains(p)) => {
                        self.add_route(start, end, horizontal)
                    }
                    _ => {}
                }
            }
            if response.clicked_by(PointerButton::Primary)
                && let Some(pos) = pointer
            {
                let world = canvas.world(pos);
                let snap = self.snap(world, tolerance);
                match self.tool {
                    Tool::Poke => {
                        self.selection.clear();
                        if let Some(id) = self.hit(world, tolerance) {
                            self.selection.insert(id);
                            if self.simulation.view.clocks.contains(&id) {
                                self.simulation
                                    .actions
                                    .push(SimulationAction::PokeClock { id });
                            } else if self.circuit.component(id).is_some_and(|c| {
                                matches!(c.kind, Kind::FlipFlop(_) | Kind::Latch(_))
                            }) {
                                self.simulation
                                    .actions
                                    .push(SimulationAction::ToggleStored { id });
                            }
                            if self
                                .circuit
                                .component(id)
                                .is_some_and(|c| c.kind == Kind::DipSwitch)
                            {
                                if let Some(bit) = self
                                    .circuit
                                    .component(id)
                                    .and_then(|c| crate::io::dip_bit(c, world))
                                {
                                    self.simulation
                                        .actions
                                        .push(SimulationAction::ToggleInput { id, bit });
                                }
                            } else if let Some(input) = self.simulation.view.inputs.get(&id) {
                                if input.bits == 1 {
                                    self.simulation
                                        .actions
                                        .push(SimulationAction::ToggleInput { id, bit: 0 });
                                } else {
                                    self.simulation.focus_value = true;
                                }
                            }
                        }
                    }
                    Tool::Place(_) => self.place(snap),
                    Tool::Wire => {
                        let snap = self.connection_point(world, tolerance);
                        if let Some(start) = self.wire_start.take() {
                            self.add_route(start, snap, !ui.input(|i| i.modifiers.shift));
                        } else {
                            self.wire_start = Some(snap);
                        }
                    }
                    Tool::Select => {
                        let modifiers = ui.input(|i| i.modifiers);
                        let hit = self.hit(world, tolerance);
                        let wire = hit.is_some_and(|id| self.circuit.component(id).is_none());
                        if !modifiers.ctrl && self.wire_start.is_some() {
                            let start = self.wire_start.take().unwrap();
                            self.add_route(
                                start,
                                self.connection_point(world, tolerance),
                                !modifiers.shift,
                            );
                        } else if !modifiers.ctrl && !modifiers.shift && wire {
                            self.commit_properties();
                            self.wire_start = Some(self.connection_point(world, tolerance));
                            self.selection.clear();
                        } else {
                            self.wire_start = None;
                            if !modifiers.shift {
                                self.selection.clear();
                            }
                            if let Some(id) = hit {
                                if !self.selection.insert(id) {
                                    self.selection.remove(&id);
                                }
                                if response.double_clicked() && self.circuit.component(id).is_some()
                                {
                                    if matches!(
                                        self.circuit.component(id).map(|c| c.kind),
                                        Some(Kind::Subcircuit(_))
                                    ) {
                                        self.open_instance = Some(id);
                                    } else {
                                        self.focus_text = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if response.hovered()
            && let Some(pointer) = pointer
        {
            let world = canvas.world(pointer);
            for c in self.circuit.components.iter().filter(|c| {
                c.kind.is_io()
                    || c.kind.is_controlled()
                    || c.kind.is_transistor()
                    || c.kind == Kind::PullResistor
                    || c.kind.is_memory()
                    || c.kind.is_sequential()
                    || matches!(
                        c.kind,
                        Kind::Plexer(_) | Kind::Arithmetic(_) | Kind::Parity(_)
                    )
            }) {
                for (index, point) in c.ports().iter().enumerate() {
                    if (drawing::pos(*point).to_vec2() - world).length() <= tolerance {
                        response.clone().on_hover_ui(|ui| {
                            ui.label(tf!(
                                "{} · {} bit(s)",
                                if matches!(c.kind, Kind::Plexer(_)) {
                                    crate::plexers::port_label(c, c.plexer_terminals()[index].role)
                                } else if matches!(c.kind, Kind::Arithmetic(_)) {
                                    crate::arithmetic::port_label(c, index)
                                } else if matches!(c.kind, Kind::Parity(_)) {
                                    crate::parity::port_label(c, index)
                                } else if c.kind.is_transistor() {
                                    tr(crate::transistors::port_label(c, index)).to_owned()
                                } else if c.kind.is_controlled() || c.kind == Kind::PullResistor {
                                    tr(crate::bus::port_label(c, index)).to_owned()
                                } else if c.kind.is_io() {
                                    tr(crate::io::port_label(c, index)).to_owned()
                                } else if matches!(c.kind, Kind::Latch(_)) {
                                    tr(crate::latches::port_label(c, index)).to_owned()
                                } else if c.kind.is_memory() {
                                    tr(crate::memory::port_label(c, index)).to_owned()
                                } else {
                                    tr(crate::sequential::port_label(c, index)).to_owned()
                                },
                                c.port_width(index).unwrap_or_else(|| self
                                    .simulation
                                    .view
                                    .ports
                                    .get(&(c.id, index))
                                    .map_or(0, |s| s.binary.len() as u8))
                            ));
                            if let Some(signal) = self.simulation.view.ports.get(&(c.id, index)) {
                                ui.monospace(&signal.binary);
                            }
                        });
                    }
                }
            }
            for c in self
                .circuit
                .components
                .iter()
                .filter(|c| matches!(c.kind, Kind::Subcircuit(_)))
            {
                for (index, (point, port)) in c.ports().iter().zip(&c.interface.ports).enumerate() {
                    if (drawing::pos(*point).to_vec2() - world).length() <= tolerance {
                        response.clone().on_hover_ui(|ui| {
                            ui.label(tf!(
                                "{} · {} · {} bit",
                                if port.label.is_empty() {
                                    tf!("Pin #{}", port.pin)
                                } else {
                                    port.label.clone()
                                },
                                tr(if port.output { "Output" } else { "Input" }),
                                port.bits
                            ));
                            if let Some(signal) = self.simulation.view.ports.get(&(c.id, index)) {
                                ui.monospace(&signal.binary);
                            }
                        });
                    }
                }
            }
        }
        if response.hovered()
            && let Some(pos) = pointer
            && let Some(id) = self.hit(canvas.world(pos), tolerance)
            && let Some(signal) = self
                .simulation
                .view
                .wires
                .get(&id)
                .or_else(|| self.simulation.view.ports.get(&(id, 0)))
        {
            response.clone().on_hover_ui(|ui| {
                ui.colored_label(signal.tone.color(), tf!("Signal: {}", signal.text));
                ui.monospace(&signal.binary);
                ui.weak(tf!("{} bit", signal.binary.len()));
            });
        }
        response.context_menu(|ui| {
            let instance = pointer
                .and_then(|p| self.hit(canvas.world(p), tolerance))
                .or_else(|| self.selection.first().copied());
            if let Some(id) = instance
                && let Some(c) = self.circuit.component(id)
                && matches!(c.kind, Kind::Subcircuit(_))
                && ui.button(tf!("View {}", c.interface.name)).clicked()
            {
                self.open_instance = Some(id);
                ui.close();
            }

            if ui
                .add_enabled(self.can_rotate(), egui::Button::new(tr("Rotate · R")))
                .clicked()
            {
                self.rotate();
                ui.close();
            }
            if ui
                .add_enabled(
                    !self.selection.is_empty(),
                    egui::Button::new(tr("Duplicate · Ctrl+D")),
                )
                .clicked()
            {
                self.duplicate();
                ui.close();
            }
            if ui
                .add_enabled(!self.selection.is_empty(), egui::Button::new(tr("Delete")))
                .clicked()
            {
                self.delete();
                ui.close();
            }
            ui.separator();
            if ui.button(tr("Reset canvas")).clicked() {
                canvas.reset();
                ui.close();
            }
            if ui.button(tr("Actual size · 100%")).clicked() {
                canvas.set_zoom(1.0);
                ui.close();
            }
            ui.checkbox(&mut canvas.show_grid, tr("Show grid"));
        });
        if self.publish_clipboard {
            ui.ctx().copy_text(CLIPBOARD_MARKER.to_owned());
            self.publish_clipboard = false;
        }
        let painter = ui.painter_at(canvas.rect);
        drawing::circuit(
            &painter,
            canvas,
            &self.circuit,
            &self.selection,
            &self.simulation.view,
        );
        if let Some(Gesture::Box { start, current, .. }) = &self.gesture {
            let rect = Rect::from_two_pos(
                canvas.screen_vec(start.to_vec2()),
                canvas.screen_vec(current.to_vec2()),
            );
            painter.rect_filled(rect, 0, crate::ACCENT.gamma_multiply(0.08));
            painter.rect_stroke(
                rect,
                0,
                egui::Stroke::new(1.0, crate::ACCENT),
                egui::StrokeKind::Inside,
            );
        }
        let preview_wire = match &self.gesture {
            Some(Gesture::Wire {
                start,
                end,
                horizontal,
            }) => Some((*start, *end, *horizontal)),
            _ => pointer
                .filter(|p| response.hovered() && canvas.rect.contains(*p))
                .and_then(|p| {
                    self.wire_start.map(|a| {
                        (
                            a,
                            self.connection_point(canvas.world(p), tolerance),
                            !ui.input(|i| i.modifiers.shift),
                        )
                    })
                }),
        };
        if let Some((a, b, horizontal)) = preview_wire {
            for pair in Circuit::route(a, b, horizontal).windows(2) {
                painter.line_segment(
                    [canvas.screen(pair[0]), canvas.screen(pair[1])],
                    egui::Stroke::new(2.0, crate::ACCENT),
                );
            }
        }
        if response.hovered()
            && !space
            && !ui.ctx().any_popup_open()
            && let Some(pos) = pointer
        {
            let world = canvas.world(pos);
            if let Tool::Place(_) = self.tool {
                let mut ghost = self.template.clone();
                ghost.position = self.snap(world, tolerance);
                drawing::component(&painter, canvas, &ghost, false, true, None);
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            } else if self.tool == Tool::Poke {
                if self.hit(world, tolerance).is_some() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            } else if self.tool == Tool::Select
                && ui.input(|i| i.modifiers.ctrl)
                && self.hit(world, tolerance).is_some()
            {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            } else if self.port_at(world, tolerance).is_some()
                || self.wire_at(world, tolerance).is_some()
            {
                let port = self.connection_point(world, tolerance);
                painter.circle_stroke(
                    canvas.screen(port),
                    6.0,
                    egui::Stroke::new(1.5, crate::ACCENT),
                );
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            } else if self.tool == Tool::Wire {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            }
        }
    }
    fn shortcuts(&mut self, ui: &mut Ui) {
        self.simulation.shortcuts(ui, &self.selection);
        let consume = |key, modifiers| ui.input_mut(|i| i.consume_key(modifiers, key));
        if consume(Key::Escape, Modifiers::NONE) {
            self.cancel_gesture();
            self.tool = Tool::Select;
            return;
        }
        if consume(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)
            || consume(Key::Y, Modifiers::COMMAND)
        {
            self.redo();
        } else if consume(Key::Z, Modifiers::COMMAND) {
            self.undo();
        }
        let (copy, cut, paste) = ui.input(|i| {
            (
                i.events.iter().any(|e| matches!(e, egui::Event::Copy)),
                i.events.iter().any(|e| matches!(e, egui::Event::Cut)),
                i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Paste(text) if text == CLIPBOARD_MARKER)),
            )
        });
        if copy || consume(Key::C, Modifiers::COMMAND) {
            self.copy();
        }
        if cut || consume(Key::X, Modifiers::COMMAND) {
            self.cut();
        }
        if paste || consume(Key::V, Modifiers::COMMAND) {
            self.paste();
        }
        if consume(Key::D, Modifiers::COMMAND) {
            self.duplicate();
        }
        if consume(Key::A, Modifiers::COMMAND) {
            self.select_all();
        }
        if consume(Key::Delete, Modifiers::NONE) || consume(Key::Backspace, Modifiers::NONE) {
            self.delete();
        }
        if !ui.input(|i| i.modifiers.command || i.modifiers.alt) {
            if consume(Key::R, Modifiers::NONE) {
                self.rotate();
            }
            if consume(Key::P, Modifiers::NONE) {
                self.set_tool(Tool::Poke);
            }
            if consume(Key::V, Modifiers::NONE) {
                self.set_tool(Tool::Select);
            }
            if consume(Key::W, Modifiers::NONE) {
                self.set_tool(Tool::Wire);
            }
            if consume(Key::T, Modifiers::NONE) {
                self.set_tool(Tool::Place(Kind::Text));
            }
            if consume(Key::U, Modifiers::NONE) {
                self.set_tool(Tool::Place(Kind::Tunnel));
            }
            for (key, delta) in [
                (Key::ArrowLeft, Point::new(-GRID, 0)),
                (Key::ArrowRight, Point::new(GRID, 0)),
                (Key::ArrowUp, Point::new(0, -GRID)),
                (Key::ArrowDown, Point::new(0, GRID)),
            ] {
                if consume(key, Modifiers::NONE) {
                    self.nudge(delta);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_restore_geometry_and_selection_through_undo_redo() {
        let mut e = Editor::default();
        e.place(Point::new(200, 100));
        let placed = e.circuit.clone();
        let id = *e.selection.first().unwrap();
        e.nudge(Point::new(40, 20));
        e.rotate();
        e.delete();
        assert!(e.circuit.components.is_empty());
        e.undo();
        e.undo();
        e.undo();
        assert_eq!(e.circuit, placed);
        assert!(e.selection.contains(&id));
        e.redo();
        assert_eq!(
            e.circuit.component(id).unwrap().position,
            Point::new(240, 120)
        );
    }
    #[test]
    fn repeated_paste_assigns_unique_ids_and_leaves_original_untouched() {
        let mut e = Editor::default();
        e.place(Point::new(100, 100));
        e.copy();
        e.paste();
        e.paste();
        assert_eq!(e.circuit.components.len(), 3);
        assert_eq!(e.circuit.ids().len(), 3);
        assert_eq!(e.circuit.components[0].position, Point::new(100, 100));
        e.undo();
        assert_eq!(e.circuit.components.len(), 2);
        e.delete();
        assert!(!e.can_redo());
    }
    #[test]
    fn property_typing_is_one_undo_transaction() {
        let mut e = Editor {
            template: Component::new(Kind::Tunnel, Point::ZERO),
            ..Editor::default()
        };
        e.place(Point::new(100, 100));
        for label in ["D", "DA", "DAT", "DATA"] {
            let mut c = e.circuit.components[0].clone();
            c.label = label.into();
            e.update_property(c);
        }
        e.commit_properties();
        e.undo();
        assert_eq!(e.circuit.components[0].label, "tunnel");
        e.redo();
        assert_eq!(e.circuit.components[0].label, "DATA");
    }
}
