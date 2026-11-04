//! Editor layout based on runtime observation, with an independent canvas viewport.

mod canvas;
mod icons;

use canvas::{Canvas, MAX_ZOOM, MIN_ZOOM};
use egui::{Color32, Frame, Margin, Panel, RichText, ScrollArea, Stroke, Ui, vec2};
use icons::Icon;

const INK: Color32 = Color32::from_rgb(51, 65, 85);
const ACCENT: Color32 = Color32::from_rgb(37, 99, 235);
const BORDER: Color32 = Color32::from_rgb(220, 225, 233);
const CHROME: Color32 = Color32::from_rgb(246, 248, 251);

/// Apply the application's light visual style once, when creating the window.
pub fn configure(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Light);
    ctx.options_mut(|options| options.zoom_with_keyboard = false);
    let mut style = (*ctx.global_style()).clone();
    style.visuals = egui::Visuals::light();
    style.visuals.override_text_color = Some(INK);
    style.visuals.panel_fill = CHROME;
    style.visuals.selection.bg_fill = Color32::from_rgb(225, 235, 255);
    style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    style.spacing.item_spacing = vec2(6.0, 5.0);
    style.spacing.button_padding = vec2(8.0, 4.0);
    style.spacing.interact_size.y = 24.0;
    style.animation_time = 0.12;
    ctx.set_global_style(style);
}

/// Presentation state only. Circuit editing and simulation are not implemented here.
#[derive(Default)]
pub struct Workspace {
    canvas: Canvas,
    about_open: bool,
}

impl Workspace {
    pub fn show(&mut self, ui: &mut Ui) {
        Panel::top("menus")
            .exact_size(29.0)
            .frame(
                Frame::NONE
                    .fill(CHROME)
                    .inner_margin(Margin::symmetric(8, 2)),
            )
            .show(ui, |ui| self.menus(ui));

        Panel::top("tools")
            .exact_size(42.0)
            .frame(
                Frame::NONE
                    .fill(CHROME)
                    .inner_margin(Margin::symmetric(8, 5)),
            )
            .show(ui, |ui| self.toolbar(ui));

        let available_width = ui.available_width();
        Panel::left("inspector")
            .default_size((available_width * 0.25).clamp(230.0, 360.0))
            .size_range(230.0..=(available_width * 0.5).max(230.0))
            .frame(Frame::NONE.fill(CHROME))
            .show(ui, |ui| self.sidebar(ui));

        egui::CentralPanel::default()
            .frame(Frame::NONE)
            .show(ui, |ui| self.canvas.show(ui));

        egui::Window::new("About logisim-rs")
            .open(&mut self.about_open)
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.heading("Logisim rs");
                ui.label("A cleanroom digital circuit workspace.");
                ui.separator();
                ui.label("Canvas navigation is available.");
                ui.label("Circuit editing, file operations and simulation are not yet available.");
            });
    }

    fn menus(&mut self, ui: &mut Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                disabled_actions(ui, &["New", "Open…", "Open Recent"]);
                ui.separator();
                disabled_actions(ui, &["Save", "Save As…", "Export Image…", "Print…"]);
                ui.separator();
                if ui.button("Exit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Edit", |ui| {
                disabled_actions(ui, &["Undo", "Redo"]);
                ui.separator();
                disabled_actions(ui, &["Cut", "Copy", "Paste", "Delete", "Select All"]);
            });
            ui.menu_button("Project", |ui| {
                disabled_actions(ui, &["Add Circuit…", "Load Library…", "Project Options…"]);
            });
            ui.menu_button("Simulate", |ui| {
                disabled_actions(ui, &["Run", "Step", "Reset", "Tick Once"]);
            });
            ui.menu_button("Window", |ui| {
                if ui.button("Minimize").clicked() {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    ui.close();
                }
                if ui.button("Maximize / Restore").clicked() {
                    let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                    ui.close();
                }
                if ui.button("Close").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
                ui.separator();
                ui.add_enabled(false, egui::Button::new("Untitled").selected(true));
            });
            ui.menu_button("Help", |ui| {
                if ui.button("About logisim-rs").clicked() {
                    self.about_open = true;
                    ui.close();
                }
            });
        });
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            icons::button(ui, Icon::Poke, false, false, "Poke — requires simulation");
            icons::button(ui, Icon::Select, true, true, "Select");
            ui.separator();
            icons::button(ui, Icon::Text, false, false, "Text — not yet available");
            ui.separator();
            for (icon, name) in [
                (Icon::Pin, "Pin"),
                (Icon::Probe, "Probe"),
                (Icon::Not, "NOT Gate"),
                (Icon::And, "AND Gate"),
                (Icon::Or, "OR Gate"),
            ] {
                icons::button(ui, icon, false, false, name);
            }
            if ui.available_width() > 350.0 {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("Wheel to zoom   ·   Middle-drag to pan")
                            .size(11.0)
                            .weak(),
                    );
                });
            }
        });
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        Panel::bottom("zoom")
            .exact_size(37.0)
            .frame(
                Frame::NONE
                    .fill(CHROME)
                    .inner_margin(Margin::symmetric(8, 3)),
            )
            .show(ui, |ui| self.zoom_controls(ui));

        let height = ui.available_height();
        Panel::top("project")
            .resizable(true)
            .default_size(height * 0.5)
            .size_range(125.0..=(height - 125.0).max(125.0))
            .frame(Frame::NONE.fill(Color32::WHITE))
            .show(ui, |ui| {
                Panel::top("project_actions")
                    .exact_size(36.0)
                    .frame(
                        Frame::NONE
                            .fill(CHROME)
                            .inner_margin(Margin::symmetric(8, 3)),
                    )
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (icon, name) in [
                                (Icon::Plus, "Add circuit"),
                                (Icon::Up, "Move up"),
                                (Icon::Down, "Move down"),
                                (Icon::Trash, "Remove circuit"),
                            ] {
                                icons::button(ui, icon, false, false, name);
                            }
                        });
                    });
                ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(6.0);
                        egui::CollapsingHeader::new(RichText::new("Untitled").strong())
                            .default_open(true)
                            .show(ui, |ui| {
                                let _ =
                                    ui.selectable_label(true, RichText::new("main").color(ACCENT));
                                for group in [
                                    "Wiring",
                                    "Gates",
                                    "TTL",
                                    "Plexers",
                                    "Arithmetic",
                                    "Memory",
                                    "Input/Output",
                                    "Base",
                                ] {
                                    egui::CollapsingHeader::new(group).show(ui, |ui| {
                                        let tools: &[&str] = match group {
                                            "Wiring" => &[
                                                "Splitter",
                                                "Pin",
                                                "Probe",
                                                "Tunnel",
                                                "Pull Resistor",
                                                "Clock",
                                                "Programmable Generator",
                                                "Constant",
                                                "Power",
                                                "Ground",
                                                "Transistor",
                                                "Transmission Gate",
                                                "Bit Extender",
                                            ],
                                            "Gates" => &["NOT Gate", "AND Gate", "OR Gate"],
                                            _ => &[],
                                        };
                                        if tools.is_empty() {
                                            ui.weak("No tools available yet");
                                        }
                                        for name in tools {
                                            ui.add_enabled(
                                                false,
                                                egui::Button::new(*name).frame(false),
                                            );
                                        }
                                    });
                                }
                            });
                    });
            });

        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(CHROME))
            .show(ui, |ui| {
                Frame::NONE
                    .inner_margin(Margin::symmetric(12, 9))
                    .show(ui, |ui| {
                        ui.label(RichText::new("Circuit: main").strong());
                    });
                ui.separator();
                ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        Frame::NONE.inner_margin(12).show(ui, |ui| {
                            egui::Grid::new("circuit_attributes")
                                .num_columns(2)
                                .spacing(vec2(16.0, 12.0))
                                .show(ui, |ui| {
                                    for (key, value) in [
                                        ("Circuit Name", "main"),
                                        ("Shared Label", "—"),
                                        ("Label Facing", "East"),
                                        ("Label Font", "Default"),
                                        ("Label Color", "#000000"),
                                    ] {
                                        ui.label(RichText::new(key).weak());
                                        ui.label(value);
                                        ui.end_row();
                                    }
                                });
                        });
                    });
            });
    }

    fn zoom_controls(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if ui.small_button("−").on_hover_text("Zoom out").clicked() {
                self.canvas.set_zoom(self.canvas.camera.zoom / 1.1);
            }
            let mut percent = self.canvas.camera.zoom * 100.0;
            if ui
                .add(
                    egui::DragValue::new(&mut percent)
                        .range(MIN_ZOOM * 100.0..=MAX_ZOOM * 100.0)
                        .suffix("%")
                        .speed(1.0),
                )
                .on_hover_text("Drag or type a zoom percentage")
                .changed()
            {
                self.canvas.set_zoom(percent / 100.0);
            }
            if ui.small_button("+").on_hover_text("Zoom in").clicked() {
                self.canvas.set_zoom(self.canvas.camera.zoom * 1.1);
            }
            ui.separator();
            if icons::button(ui, Icon::Home, false, true, "Reset canvas · Ctrl+0").clicked() {
                self.canvas.reset();
            }
            ui.checkbox(&mut self.canvas.show_grid, "Grid");
        });
    }
}

fn disabled_actions(ui: &mut Ui, labels: &[&str]) {
    for label in labels {
        ui.add_enabled(false, egui::Button::new(*label));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{
        Context, Event, Modifiers, MouseWheelSource, MouseWheelUnit, PointerButton, Pos2, RawInput,
        Rect, TouchPhase, Vec2, pos2,
    };

    fn frame(ctx: &Context, app: &mut Workspace, size: Vec2, events: Vec<Event>) {
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                events,
                ..Default::default()
            },
            |ui| app.show(ui),
        );
        // These tests exercise layout and input without a renderer.
        output.textures_delta.clear();
    }

    fn wheel() -> Event {
        Event::MouseWheel {
            unit: MouseWheelUnit::Line,
            delta: vec2(0.0, 1.0),
            phase: TouchPhase::Move,
            source: MouseWheelSource::Unknown,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn canvas_keeps_the_right_hand_layout_at_both_window_sizes() {
        for size in [vec2(1200.0, 820.0), vec2(720.0, 480.0)] {
            let ctx = Context::default();
            configure(&ctx);
            let mut app = Workspace::default();
            frame(&ctx, &mut app, size, vec![]);
            let canvas = app.canvas.rect;
            assert!(canvas.left() >= 230.0 && canvas.left() < size.x * 0.5);
            assert!(canvas.top() >= 65.0 && canvas.top() < 90.0);
            assert_eq!(canvas.right(), size.x);
            assert_eq!(
                canvas.bottom(),
                size.y,
                "No full-width bottom bar may shrink the canvas"
            );
            assert!(canvas.width() > 450.0);
        }
    }

    #[test]
    fn wheel_zooms_only_canvas_and_keeps_the_pointer_anchor() {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        let size = vec2(1200.0, 820.0);
        let pointer = pos2(620.0, 310.0);
        frame(&ctx, &mut app, size, vec![Event::PointerMoved(pointer)]);
        frame(&ctx, &mut app, size, vec![Event::PointerMoved(pointer)]);
        let world = app.canvas.camera.world_at(pointer - app.canvas.rect.min);
        frame(&ctx, &mut app, size, vec![wheel()]);
        assert!((app.canvas.camera.zoom - 1.1).abs() < 0.001);
        assert!(
            (app.canvas.camera.world_at(pointer - app.canvas.rect.min) - world).length() < 0.001
        );
        let zoom = app.canvas.camera.zoom;
        frame(
            &ctx,
            &mut app,
            size,
            vec![Event::PointerMoved(pos2(80.0, 160.0)), wheel()],
        );
        assert_eq!(
            app.canvas.camera.zoom, zoom,
            "Scrolling the tree must not zoom the canvas"
        );
    }

    #[test]
    fn reset_shortcut_works_after_the_canvas_takes_focus() {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        let size = vec2(1200.0, 820.0);
        let pos = pos2(620.0, 310.0);
        frame(&ctx, &mut app, size, vec![Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut app,
                size,
                vec![Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                }],
            );
        }
        app.canvas.camera.pan(vec2(120.0, -70.0));
        app.canvas.set_zoom(2.0);
        frame(
            &ctx,
            &mut app,
            size,
            vec![Event::Key {
                key: egui::Key::Num0,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            }],
        );
        assert_eq!(app.canvas.camera.zoom, 1.0);
        assert_eq!(app.canvas.camera.offset, canvas::Camera::default().offset);
    }

    #[test]
    fn middle_button_drag_moves_the_viewport() {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        let size = vec2(1200.0, 820.0);
        let start = pos2(620.0, 310.0);
        frame(&ctx, &mut app, size, vec![Event::PointerMoved(start)]);
        frame(
            &ctx,
            &mut app,
            size,
            vec![Event::PointerButton {
                pos: start,
                button: PointerButton::Middle,
                pressed: true,
                modifiers: Modifiers::NONE,
            }],
        );
        let before = app.canvas.camera.offset;
        frame(
            &ctx,
            &mut app,
            size,
            vec![Event::PointerMoved(start + vec2(80.0, 45.0))],
        );
        assert!((app.canvas.camera.offset - before - vec2(80.0, 45.0)).length() < 0.001);
    }
}
