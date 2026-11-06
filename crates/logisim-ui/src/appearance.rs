//! Appearance gestures are previews until release, then join the project undo timeline.
use crate::{
    ACCENT, artwork,
    canvas::Canvas,
    editor::{Editor, Snapshot},
};
use crate::{i18n::tr, tf};
use egui::{Key, Modifiers, PointerButton, Rect, Response, Stroke, Ui, Vec2};
use logisim_core::{Appearance, Artwork, ArtworkFont, Geometry, Id, Paint, Point, TextAnchor};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Target {
    Shape(Id),
    Port(Id),
    Anchor,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Tool {
    #[default]
    Select,
    Text,
    Line,
    Curve,
    Polyline,
    Rect,
    Rounded,
    Ellipse,
    Polygon,
}
impl Tool {
    fn name(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Text => "Text",
            Self::Line => "Line",
            Self::Curve => "Curve",
            Self::Polyline => "Polyline",
            Self::Rect => "Rect",
            Self::Rounded => "Round",
            Self::Ellipse => "Ellipse",
            Self::Polygon => "Polygon",
        }
    }
}
pub(crate) enum Gesture {
    Move {
        start: Point,
        base: Appearance,
        before: Snapshot,
        targets: BTreeSet<Target>,
        draft: Appearance,
    },
    Handle {
        target: Target,
        index: usize,
        before: Snapshot,
        draft: Appearance,
    },
    Draw {
        start: Point,
        end: Point,
    },
    Box {
        start: Vec2,
        end: Vec2,
        previous: BTreeSet<Target>,
    },
}
#[derive(Clone, Copy)]
pub(crate) enum Command {
    Cut,
    Copy,
    Paste,
    Duplicate,
    Delete,
    All,
    Raise,
    Lower,
    Front,
    Back,
    AddVertex,
    RemoveVertex,
}
#[derive(Default)]
pub(crate) struct AppearanceEditor {
    pub selected: BTreeSet<Target>,
    pub tool: Tool,
    pub paint: Paint,
    pub font: ArtworkFont,
    pub text_anchor: TextAnchor,
    pub radius: i32,
    pub snap: bool,
    pub gesture: Option<Gesture>,
    pub points: Vec<Point>,
    pub vertex: Option<(Id, usize)>,
    pub clipboard: Vec<Artwork>,
    pub focus_text: bool,
    pub package_width: i32,
    pub package_pitch: i32,
    pub package_labels: bool,
    pub fit_requested: bool,
    initialized: bool,
    publish_clipboard: bool,
}
impl AppearanceEditor {
    pub fn current(editor: &Editor) -> Appearance {
        let mut definition = editor.project.circuit(editor.active).unwrap().clone();
        definition.circuit = editor.circuit.clone();
        definition.editable_appearance()
    }
    pub fn reset_context(&mut self) {
        self.cancel();
        self.selected.clear();
        self.vertex = None;
        self.tool = Tool::Select;
    }
    pub fn cancel(&mut self) {
        self.gesture = None;
        self.points.clear();
    }
    pub fn revert(&mut self, editor: &mut Editor) {
        self.fit_requested = true;
        self.cancel();
        editor.commit_properties();
        let before = editor.snapshot();
        editor.circuit.appearance = None;
        editor.record(before);
        self.selected.clear();
        self.vertex = None;
    }
    fn apply(&mut self, editor: &mut Editor, before: Snapshot, appearance: Appearance) {
        if appearance == Self::current(editor) {
            return;
        }
        editor.circuit.appearance = Some(appearance);
        editor.record(before);
    }
    pub fn toolbar(&mut self, ui: &mut Ui, editor: &mut Editor) {
        if !self.initialized {
            self.snap = true;
            self.radius = 10;
            self.package_width = 140;
            self.package_pitch = 20;
            self.package_labels = true;
            self.initialized = true;
        }
        ui.horizontal(|ui| {
            for tool in [
                Tool::Select,
                Tool::Text,
                Tool::Line,
                Tool::Curve,
                Tool::Polyline,
                Tool::Rect,
                Tool::Rounded,
                Tool::Ellipse,
                Tool::Polygon,
            ] {
                if ui
                    .selectable_label(self.tool == tool, tr(tool.name()))
                    .clicked()
                {
                    self.cancel();
                    editor.commit_properties();
                    self.selected.clear();
                    self.tool = tool;
                }
            }
            ui.separator();
            ui.checkbox(&mut self.snap, tr("Snap"));
            if ui.button(tr("Circuit layout")).clicked() {
                self.cancel();
                editor.commit_properties();
                editor.appearance_mode = false;
            }
            ui.weak(if editor.circuit.appearance.is_some() {
                tr("Custom appearance")
            } else {
                tr("Default appearance")
            });
        });
    }
    fn point(&self, world: Vec2, grid: bool) -> Point {
        if grid || self.snap {
            Point::snapped(world.x, world.y)
        } else {
            Point::new(
                world.x.round().clamp(-1e6, 1e6) as i32,
                world.y.round().clamp(-1e6, 1e6) as i32,
            )
        }
    }
    fn targets(a: &Appearance) -> BTreeSet<Target> {
        a.shapes
            .iter()
            .map(|s| Target::Shape(s.id))
            .chain(a.ports.keys().map(|id| Target::Port(*id)))
            .chain([Target::Anchor])
            .collect()
    }
    fn hit(a: &Appearance, world: Vec2, tolerance: f32) -> Option<Target> {
        if (artwork::vector(a.anchor) - world).length() <= tolerance + 4.0 {
            return Some(Target::Anchor);
        }
        if let Some((id, _)) = a
            .ports
            .iter()
            .find(|(_, p)| (artwork::vector(**p) - world).length() <= tolerance + 4.0)
        {
            return Some(Target::Port(*id));
        }
        a.shapes
            .iter()
            .rev()
            .find(|s| artwork::hit(s, world, tolerance))
            .map(|s| Target::Shape(s.id))
    }
    fn handle(&self, a: &Appearance, world: Vec2, tolerance: f32) -> Option<(Target, usize)> {
        if self.selected.len() != 1 {
            return None;
        }
        let target = *self.selected.first()?;
        if let Target::Shape(id) = target {
            let shape = a.shapes.iter().find(|s| s.id == id)?;
            shape
                .geometry
                .handles()
                .iter()
                .position(|p| (artwork::vector(*p) - world).length() <= tolerance + 3.0)
                .map(|i| (target, i))
        } else {
            let at = match target {
                Target::Port(id) => *a.ports.get(&id)?,
                Target::Anchor => a.anchor,
                _ => return None,
            };
            ((artwork::vector(at) - world).length() <= tolerance + 4.0).then_some((target, 0))
        }
    }
    pub fn command(&mut self, editor: &mut Editor, command: Command) {
        self.cancel();
        editor.commit_properties();
        let before = editor.snapshot();
        let mut a = Self::current(editor);
        let selected = self.selected.clone();
        let is_selected = |s: &Artwork| selected.contains(&Target::Shape(s.id));
        match command {
            Command::Copy | Command::Cut => {
                self.publish_clipboard = true;
                self.clipboard = a
                    .shapes
                    .iter()
                    .filter(|s| is_selected(s))
                    .cloned()
                    .collect();
                if matches!(command, Command::Copy) {
                    return;
                }
                a.shapes.retain(|s| !is_selected(s));
                self.selected.retain(|t| !matches!(t, Target::Shape(_)));
            }
            Command::Paste | Command::Duplicate => {
                let shapes = if matches!(command, Command::Duplicate) {
                    a.shapes
                        .iter()
                        .filter(|s| is_selected(s))
                        .cloned()
                        .collect()
                } else {
                    self.clipboard.clone()
                };
                self.selected.clear();
                for mut shape in shapes {
                    shape.geometry.translate(Point::new(10, 10));
                    if let Some(id) = a.add(shape.geometry, shape.paint) {
                        self.selected.insert(Target::Shape(id));
                    }
                }
            }
            Command::Delete => {
                a.shapes.retain(|s| !is_selected(s));
                self.selected.retain(|t| !matches!(t, Target::Shape(_)));
            }
            Command::All => {
                self.selected = Self::targets(&a);
                return;
            }
            Command::Raise => {
                for i in (0..a.shapes.len().saturating_sub(1)).rev() {
                    if is_selected(&a.shapes[i]) && !is_selected(&a.shapes[i + 1]) {
                        a.shapes.swap(i, i + 1);
                    }
                }
            }
            Command::Lower => {
                for i in 1..a.shapes.len() {
                    if is_selected(&a.shapes[i]) && !is_selected(&a.shapes[i - 1]) {
                        a.shapes.swap(i, i - 1);
                    }
                }
            }
            Command::Front | Command::Back => {
                a.shapes
                    .sort_by_key(|s| is_selected(s) ^ matches!(command, Command::Back));
            }
            Command::AddVertex | Command::RemoveVertex => {
                if let Some((id, index)) = self.vertex
                    && let Some(shape) = a.shapes.iter_mut().find(|s| s.id == id)
                    && let Geometry::Poly { points, closed } = &mut shape.geometry
                    && index < points.len()
                {
                    if matches!(command, Command::AddVertex) && points.len() < 256 {
                        let next = points[(index + 1) % points.len()];
                        let at = points[index];
                        points.insert(
                            index + 1,
                            Point::new((at.x + next.x) / 2, (at.y + next.y) / 2),
                        );
                        self.vertex = Some((id, index + 1));
                    } else if points.len() > if *closed { 3 } else { 2 } {
                        points.remove(index);
                        self.vertex = None;
                    }
                }
            }
        }
        if a != Self::current(editor) {
            self.apply(editor, before, a);
        }
    }
    fn publish(&mut self, ui: &Ui) {
        if std::mem::take(&mut self.publish_clipboard) {
            ui.ctx()
                .copy_text("logisim-rs appearance (paste in the same window)".into());
        }
    }
    pub fn can_edit_vertex(&self, editor: &Editor, remove: bool) -> bool {
        let Some((id, index)) = self.vertex else {
            return false;
        };
        Self::current(editor).shapes.iter().any(|s|s.id==id && matches!(&s.geometry,Geometry::Poly {points,closed} if index<points.len() && if remove {points.len()>if *closed {3}else{2}}else{points.len()<256}))
    }
    pub fn edit_menu(&mut self, ui: &mut Ui, editor: &mut Editor) {
        if ui
            .add_enabled(editor.can_undo(), egui::Button::new(tr("Undo · Ctrl+Z")))
            .clicked()
        {
            self.cancel();
            editor.undo();
            self.selected.clear();
            ui.close();
        }
        if ui
            .add_enabled(
                editor.can_redo(),
                egui::Button::new(tr("Redo · Ctrl+Shift+Z")),
            )
            .clicked()
        {
            self.cancel();
            editor.redo();
            self.selected.clear();
            ui.close();
        }
        ui.separator();
        let selected = self.selected.iter().any(|t| matches!(t, Target::Shape(_)));
        for (label, command, enabled) in [
            ("Cut · Ctrl+X", Command::Cut, selected),
            ("Copy · Ctrl+C", Command::Copy, selected),
            ("Paste · Ctrl+V", Command::Paste, !self.clipboard.is_empty()),
            ("Duplicate · Ctrl+D", Command::Duplicate, selected),
            ("Delete", Command::Delete, selected),
            ("Select All · Ctrl+A", Command::All, true),
            ("Raise · Ctrl+Up", Command::Raise, selected),
            ("Lower · Ctrl+Down", Command::Lower, selected),
            ("Bring to Front", Command::Front, selected),
            ("Send to Back", Command::Back, selected),
            (
                "Add Vertex",
                Command::AddVertex,
                self.can_edit_vertex(editor, false),
            ),
            (
                "Remove Vertex",
                Command::RemoveVertex,
                self.can_edit_vertex(editor, true),
            ),
        ] {
            if ui
                .add_enabled(enabled, egui::Button::new(tr(label)))
                .clicked()
            {
                self.command(editor, command);
                ui.close();
            }
        }
        self.publish(ui);
    }
    fn insert(&mut self, editor: &mut Editor, geometry: Geometry) {
        editor.commit_properties();
        let before = editor.snapshot();
        let mut a = Self::current(editor);
        let mut paint = self.paint.clone();
        if matches!(geometry, Geometry::Text { .. }) {
            paint.stroke = None;
            paint.fill = Some(self.paint.fill.unwrap_or([0; 3]));
        }
        if let Some(id) = a.add(geometry, paint) {
            self.selected = BTreeSet::from([Target::Shape(id)]);
            self.apply(editor, before, a);
        } else {
            editor.project_error =
                Some("The shape is empty, invalid, or exceeds the appearance limit.".into());
        }
    }
    fn finish_points(&mut self, editor: &mut Editor) {
        let mut points = std::mem::take(&mut self.points);
        points.dedup();
        let closed = self.tool == Tool::Polygon;
        if closed && points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        if points.len() >= if closed { 3 } else { 2 } {
            self.insert(editor, Geometry::Poly { points, closed });
        }
    }
    fn draw_geometry(&self, start: Point, end: Point) -> Option<Geometry> {
        Some(match self.tool {
            Tool::Line => Geometry::Poly {
                points: vec![start, end],
                closed: false,
            },
            Tool::Rect | Tool::Rounded => Geometry::Rect {
                a: start,
                b: end,
                radius: if self.tool == Tool::Rounded {
                    self.radius
                } else {
                    0
                },
            },
            Tool::Ellipse => Geometry::Ellipse { a: start, b: end },
            _ => return None,
        })
    }
    fn shortcuts(&mut self, ui: &mut Ui, editor: &mut Editor) {
        let shift = Modifiers::COMMAND.plus(Modifiers::SHIFT);
        if ui.input_mut(|i| i.consume_key(shift, Key::Z)) {
            self.cancel();
            editor.redo();
            self.selected.clear();
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Z)) {
            self.cancel();
            editor.undo();
            self.selected.clear();
        }
        for (modifiers, key, command) in [
            (shift, Key::ArrowUp, Command::Front),
            (shift, Key::ArrowDown, Command::Back),
            (Modifiers::COMMAND, Key::ArrowUp, Command::Raise),
            (Modifiers::COMMAND, Key::ArrowDown, Command::Lower),
            (Modifiers::COMMAND, Key::D, Command::Duplicate),
            (Modifiers::COMMAND, Key::A, Command::All),
            (Modifiers::NONE, Key::Delete, Command::Delete),
        ] {
            if ui.input_mut(|i| i.consume_key(modifiers, key)) {
                self.command(editor, command);
            }
        }
        // Consume native clipboard events and key fallbacks once per action.
        let events = ui.input(|i| i.events.clone());
        for (key, command, native) in [
            (
                Key::C,
                Command::Copy,
                events.iter().any(|e| matches!(e, egui::Event::Copy)),
            ),
            (
                Key::X,
                Command::Cut,
                events.iter().any(|e| matches!(e, egui::Event::Cut)),
            ),
            (
                Key::V,
                Command::Paste,
                events.iter().any(|e| matches!(e, egui::Event::Paste(_))),
            ),
        ] {
            let pressed = ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, key));
            if native || pressed {
                self.command(editor, command);
            }
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.cancel();
            self.selected.clear();
            self.tool = Tool::Select;
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
            && matches!(self.tool, Tool::Polyline | Tool::Polygon)
        {
            self.finish_points(editor);
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Backspace)) {
            self.points.pop();
        }
        let mut delta = Point::ZERO;
        for (key, p) in [
            (Key::ArrowLeft, Point::new(-1, 0)),
            (Key::ArrowRight, Point::new(1, 0)),
            (Key::ArrowUp, Point::new(0, -1)),
            (Key::ArrowDown, Point::new(0, 1)),
        ] {
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, key)) {
                delta = delta + p;
            }
        }
        if delta != Point::ZERO && !self.selected.is_empty() {
            let step = if self.snap || self.selected.iter().any(|t| !matches!(t, Target::Shape(_)))
            {
                10
            } else {
                1
            };
            let delta = Point::new(delta.x * step, delta.y * step);
            editor.commit_properties();
            let before = editor.snapshot();
            let mut a = Self::current(editor);
            Self::translate(&mut a, &self.selected, delta);
            self.apply(editor, before, a);
        }
    }
    fn translate(a: &mut Appearance, targets: &BTreeSet<Target>, delta: Point) {
        for target in targets {
            match target {
                Target::Shape(id) => {
                    if let Some(s) = a.shapes.iter_mut().find(|s| s.id == *id) {
                        s.geometry.translate(delta);
                    }
                }
                Target::Port(id) => {
                    if let Some(at) = a.ports.get_mut(id) {
                        *at = *at + delta;
                    }
                }
                Target::Anchor => a.anchor = a.anchor + delta,
            }
        }
    }
    pub fn show(
        &mut self,
        ui: &mut Ui,
        editor: &mut Editor,
        canvas: &mut Canvas,
        response: &Response,
    ) {
        let a = Self::current(editor);
        self.selected.retain(|t| Self::targets(&a).contains(t));
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let world = pointer.map(|p| canvas.world(p));
        let interactive = ui.is_enabled() && !ui.ctx().any_popup_open();
        if interactive
            && (response.has_focus()
                || (response.hovered() && !ui.ctx().egui_wants_keyboard_input()))
        {
            self.shortcuts(ui, editor);
        }
        if !editor.appearance_mode {
            return;
        }
        let space = ui.input(|i| i.key_down(Key::Space));
        if space {
            self.gesture = None;
        }
        if interactive && !space {
            if response.drag_started_by(PointerButton::Primary)
                && let Some(origin) = ui.input(|i| i.pointer.press_origin())
            {
                editor.commit_properties();
                let a = Self::current(editor);
                let at = canvas.world(origin);
                let tolerance = 6.0 / canvas.camera.zoom;
                if self.tool == Tool::Select {
                    if let Some((target, index)) = self.handle(&a, at, tolerance) {
                        if let Target::Shape(id) = target {
                            self.vertex = Some((id, index));
                        }
                        self.gesture = Some(Gesture::Handle {
                            target,
                            index,
                            before: editor.snapshot(),
                            draft: a,
                        });
                    } else if let Some(target) = Self::hit(&a, at, tolerance) {
                        if !self.selected.contains(&target) {
                            if !ui.input(|i| i.modifiers.shift) {
                                self.selected.clear();
                            }
                            self.selected.insert(target);
                        }
                        let grid = self.selected.iter().any(|t| !matches!(t, Target::Shape(_)));
                        self.gesture = Some(Gesture::Move {
                            start: self.point(at, grid),
                            base: a.clone(),
                            before: editor.snapshot(),
                            targets: self.selected.clone(),
                            draft: a,
                        });
                    } else {
                        self.gesture = Some(Gesture::Box {
                            start: at,
                            end: at,
                            previous: if ui.input(|i| i.modifiers.shift) {
                                self.selected.clone()
                            } else {
                                BTreeSet::new()
                            },
                        });
                    }
                } else if matches!(
                    self.tool,
                    Tool::Rect | Tool::Rounded | Tool::Ellipse | Tool::Line | Tool::Curve
                ) {
                    self.gesture = Some(Gesture::Draw {
                        start: self.point(at, false),
                        end: self.point(at, false),
                    });
                }
            }
            if response.dragged_by(PointerButton::Primary)
                && let Some(world) = world
                && let Some(mut gesture) = self.gesture.take()
            {
                match &mut gesture {
                    Gesture::Move {
                        start,
                        base,
                        targets,
                        draft,
                        ..
                    } => {
                        let grid = targets.iter().any(|t| !matches!(t, Target::Shape(_)));
                        let delta = self.point(world, grid) - *start;
                        *draft = base.clone();
                        Self::translate(draft, targets, delta);
                    }
                    Gesture::Handle {
                        target,
                        index,
                        draft,
                        ..
                    } => {
                        let at = self.point(world, !matches!(target, Target::Shape(_)));
                        match target {
                            Target::Shape(id) => {
                                if let Some(s) = draft.shapes.iter_mut().find(|s| s.id == *id) {
                                    s.geometry.set_handle(*index, at);
                                }
                            }
                            Target::Port(id) => {
                                draft.ports.insert(*id, at);
                            }
                            Target::Anchor => draft.anchor = at,
                        }
                    }
                    Gesture::Draw { end, .. } => *end = self.point(world, false),
                    Gesture::Box { end, .. } => *end = world,
                }
                self.gesture = Some(gesture);
            }
            if response.drag_stopped_by(PointerButton::Primary)
                && let Some(gesture) = self.gesture.take()
            {
                match gesture {
                    Gesture::Move { before, draft, .. } | Gesture::Handle { before, draft, .. } => {
                        if draft.shapes.iter().all(|s| s.geometry.valid()) {
                            self.apply(editor, before, draft);
                        }
                    }
                    Gesture::Draw { start, end } if start != end => {
                        if self.tool == Tool::Curve {
                            self.points = vec![start, end];
                        } else if let Some(g) = self.draw_geometry(start, end) {
                            self.insert(editor, g);
                        }
                    }
                    Gesture::Box {
                        start,
                        end,
                        previous,
                    } => {
                        self.selected = previous;
                        let rect = Rect::from_two_pos(start.to_pos2(), end.to_pos2());
                        let a = Self::current(editor);
                        for s in &a.shapes {
                            let (a, b) = s.geometry.bounds();
                            if rect.contains_rect(Rect::from_two_pos(
                                artwork::vector(a).to_pos2(),
                                artwork::vector(b).to_pos2(),
                            )) {
                                self.selected.insert(Target::Shape(s.id));
                            }
                        }
                        for (id, at) in &a.ports {
                            if rect.contains(artwork::vector(*at).to_pos2()) {
                                self.selected.insert(Target::Port(*id));
                            }
                        }
                        if rect.contains(artwork::vector(a.anchor).to_pos2()) {
                            self.selected.insert(Target::Anchor);
                        }
                    }
                    _ => {}
                }
            }
            if response.clicked_by(PointerButton::Primary)
                && let Some(world) = world
            {
                editor.commit_properties();
                let at = self.point(world, false);
                match self.tool {
                    Tool::Select => {
                        let a = Self::current(editor);
                        let tolerance = 6.0 / canvas.camera.zoom;
                        if let Some((Target::Shape(id), index)) = self.handle(&a, world, tolerance)
                        {
                            self.vertex = Some((id, index));
                        } else {
                            self.vertex = None;
                            let hit = Self::hit(&a, world, tolerance);
                            if !ui.input(|i| i.modifiers.shift) {
                                self.selected.clear();
                            }
                            if let Some(target) = hit
                                && !self.selected.insert(target)
                            {
                                self.selected.remove(&target);
                            }
                        }
                    }
                    Tool::Text => {
                        self.insert(
                            editor,
                            Geometry::Text {
                                at,
                                text: "Text".into(),
                                font: self.font.clone(),
                                anchor: self.text_anchor,
                            },
                        );
                        self.tool = Tool::Select;
                        self.focus_text = true;
                    }
                    Tool::Polyline | Tool::Polygon => {
                        if self.points.last() != Some(&at) {
                            self.points.push(at);
                        }
                        if response.double_clicked() {
                            self.finish_points(editor);
                        }
                    }
                    Tool::Curve if self.points.len() == 2 => {
                        let a = self.points[0];
                        let b = self.points[1];
                        self.points.clear();
                        self.insert(editor, Geometry::Curve { a, control: at, b });
                    }
                    _ => {}
                }
            }
        }
        let preview = match &self.gesture {
            Some(Gesture::Move { draft, .. } | Gesture::Handle { draft, .. }) => draft.clone(),
            _ => Self::current(editor),
        };
        let painter = ui.painter_at(canvas.rect);
        for shape in &preview.shapes {
            artwork::paint(&painter, canvas, shape, |p| p, 0.0);
        }
        for (id, at) in &preview.ports {
            let output = editor.circuit.component(*id).is_some_and(|p| p.output);
            let center = canvas.screen(*at);
            let radius = if output { 5.0 } else { 4.0 } * canvas.camera.zoom;
            let stroke = Stroke::new(1.5, ACCENT);
            if output {
                painter.circle_stroke(center, radius, stroke);
            } else {
                painter.rect_stroke(
                    Rect::from_center_size(center, Vec2::splat(radius * 2.0)),
                    0.0,
                    stroke,
                    egui::StrokeKind::Middle,
                );
            }
            painter.circle_filled(center, 2.0, ACCENT);
        }
        let center = canvas.screen(preview.anchor);
        let green = egui::Color32::from_rgb(34, 139, 70);
        painter.circle_stroke(center, 4.0 * canvas.camera.zoom, Stroke::new(1.7, green));
        painter.line_segment(
            [
                center,
                canvas.screen(preview.anchor + preview.facing.transform(Point::new(14, 0))),
            ],
            Stroke::new(1.7, green),
        );
        for target in &self.selected {
            let handles = match target {
                Target::Shape(id) => preview
                    .shapes
                    .iter()
                    .find(|s| s.id == *id)
                    .map(|s| s.geometry.handles())
                    .unwrap_or_default(),
                Target::Port(id) => preview.ports.get(id).copied().into_iter().collect(),
                Target::Anchor => vec![preview.anchor],
            };
            for (i, at) in handles.into_iter().enumerate() {
                let rect = Rect::from_center_size(canvas.screen(at), Vec2::splat(7.0));
                painter.rect_filled(
                    rect,
                    1.0,
                    if self.vertex
                        == match target {
                            Target::Shape(id) => Some((*id, i)),
                            _ => None,
                        }
                    {
                        ACCENT
                    } else {
                        egui::Color32::WHITE
                    },
                );
                painter.rect_stroke(
                    rect,
                    1.0,
                    Stroke::new(1.0, ACCENT),
                    egui::StrokeKind::Middle,
                );
            }
        }
        if let Some(Gesture::Box { start, end, .. }) = &self.gesture {
            painter.rect_stroke(
                Rect::from_two_pos(canvas.screen_vec(*start), canvas.screen_vec(*end)),
                0.0,
                Stroke::new(1.0, ACCENT),
                egui::StrokeKind::Middle,
            );
        }
        let drawing = match &self.gesture {
            Some(Gesture::Draw { start, end }) => self.draw_geometry(*start, *end),
            _ => None,
        }
        .or_else(|| {
            let at = self.point(world?, false);
            if self.tool == Tool::Curve && self.points.len() == 2 {
                Some(Geometry::Curve {
                    a: self.points[0],
                    control: at,
                    b: self.points[1],
                })
            } else if !self.points.is_empty() {
                let mut points = self.points.clone();
                points.push(at);
                Some(Geometry::Poly {
                    points,
                    closed: self.tool == Tool::Polygon,
                })
            } else {
                None
            }
        });
        if let Some(geometry) = drawing {
            artwork::paint(
                &painter,
                canvas,
                &Artwork {
                    id: 0,
                    geometry,
                    paint: self.paint.clone(),
                },
                |p| p,
                0.0,
            );
        }
        response.context_menu(|ui| self.edit_menu(ui, editor));
        self.publish(ui);
        if response.hovered()
            && let Some(world) = world
            && let Some(Target::Port(id)) = Self::hit(&preview, world, 4.0 / canvas.camera.zoom)
            && let Some(pin) = editor.circuit.component(id)
        {
            response.clone().on_hover_text(tf!(
                "{} · {} · {} bit(s) · Pin ({}, {})",
                pin.label,
                tr(if pin.output { "Output" } else { "Input" }),
                pin.bits,
                pin.position.x,
                pin.position.y
            ));
        }
    }
}
