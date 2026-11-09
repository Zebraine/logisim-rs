//! Input-level regressions: drive the same widgets/gestures as the desktop application.
use crate::editor::Tool;
use crate::*;
use egui::{Event, Key, Modifiers, Pos2};
use logisim_core::{Component, Kind, Point};

use super::support::*;

#[test]
fn joystick_drag_captures_both_world_axes_without_document_edits() {
    for facing in logisim_core::Facing::ALL {
        for zoom in [0.75, 2.0] {
            let (ctx, mut app) = setup();
            let mut c = Component::new(Kind::Joystick, Point::new(200, 200));
            c.facing = facing;
            let center = c.joystick_center();
            let mut circuit = logisim_core::Circuit::default();
            let id = circuit.add_component(c);
            app.replace_circuit(circuit);
            app.editor.set_tool(Tool::Poke);
            frame(&ctx, &mut app, vec![], Modifiers::NONE);
            app.canvas.camera.zoom = zoom;
            let before = app.project().clone();
            let start = app.canvas.screen(center + Point::new(10, -5));
            button(&ctx, &mut app, start, true);
            assert_eq!(
                app.take_simulation_actions(),
                vec![SimulationAction::SetJoystick {
                    id,
                    offset: Point::new(10, -5)
                }]
            );
            let outside = app.canvas.screen(center + Point::new(100, 100));
            frame(
                &ctx,
                &mut app,
                vec![Event::PointerMoved(outside)],
                Modifiers::NONE,
            );
            assert_eq!(
                app.take_simulation_actions(),
                vec![SimulationAction::SetJoystick {
                    id,
                    offset: Point::new(14, 14)
                }]
            );
            frame(&ctx, &mut app, vec![], Modifiers::NONE);
            assert!(app.take_simulation_actions().is_empty());
            button(&ctx, &mut app, Pos2::new(100.0, 750.0), false);
            assert_eq!(
                app.take_simulation_actions(),
                vec![SimulationAction::ReleaseJoysticks]
            );
            assert_eq!(app.project(), &before);
            assert!(!app.editor.can_undo());
        }
    }
}

#[test]
fn joystick_releases_on_focus_modal_escape_tool_and_instance_changes() {
    for cause in 0..5 {
        let (ctx, mut app) = setup();
        let c = Component::new(Kind::Joystick, Point::new(200, 200));
        let center = c.joystick_center();
        let mut circuit = logisim_core::Circuit::default();
        circuit.add_component(c);
        app.replace_circuit(circuit);
        app.editor.set_tool(Tool::Poke);
        frame(&ctx, &mut app, vec![], Modifiers::NONE);
        let pos = app.canvas.screen(center + Point::new(10, 0));
        button(&ctx, &mut app, pos, true);
        assert_eq!(app.take_simulation_actions().len(), 1);
        match cause {
            0 => frame(
                &ctx,
                &mut app,
                vec![Event::WindowFocused(false)],
                Modifiers::NONE,
            ),
            1 => {
                app.settings_open = true;
                frame(&ctx, &mut app, vec![], Modifiers::NONE);
            }
            2 => key(&ctx, &mut app, Key::Escape, Modifiers::NONE),
            3 => app.editor.set_tool(Tool::Select),
            _ => {
                app.editor.appearance_mode = true;
                frame(&ctx, &mut app, vec![], Modifiers::NONE);
            }
        }
        assert_eq!(
            app.take_simulation_actions(),
            vec![SimulationAction::ReleaseJoysticks],
            "cause {cause}"
        );
        assert!(!app.editor.can_undo());
    }
}

#[test]
fn momentary_button_press_drag_release_focus_and_modal_are_runtime_only() {
    for facing in logisim_core::Facing::ALL {
        let (ctx, mut app) = setup();
        let mut c = Component::new(Kind::Button, Point::new(200, 200));
        c.facing = facing;
        let mut circuit = logisim_core::Circuit::default();
        let id = circuit.add_component(c);
        app.replace_circuit(circuit);
        app.editor.set_tool(Tool::Poke);
        frame(&ctx, &mut app, vec![], Modifiers::NONE);
        let before = app.project().clone();
        let at = drawing::bounds(app.circuit().component(id).unwrap()).center();
        let pos = app.canvas.screen_vec(at.to_vec2());
        button(&ctx, &mut app, pos, true);
        assert_eq!(
            app.take_simulation_actions(),
            vec![SimulationAction::PressButton { id }]
        );
        let outside = Pos2::new(100.0, 750.0);
        frame(
            &ctx,
            &mut app,
            vec![Event::PointerMoved(outside)],
            Modifiers::NONE,
        );
        assert!(app.take_simulation_actions().is_empty());
        key(&ctx, &mut app, Key::R, Modifiers::COMMAND);
        assert_eq!(app.take_simulation_actions(), vec![SimulationAction::Reset]);
        button(&ctx, &mut app, outside, false);
        assert_eq!(
            app.take_simulation_actions(),
            vec![SimulationAction::ReleaseButtons]
        );
        assert_eq!(app.project(), &before);
        assert!(!app.editor.can_undo());
        button(&ctx, &mut app, pos, true);
        assert_eq!(
            app.take_simulation_actions(),
            vec![SimulationAction::PressButton { id }]
        );
        frame(
            &ctx,
            &mut app,
            vec![Event::WindowFocused(false)],
            Modifiers::NONE,
        );
        assert_eq!(
            app.take_simulation_actions(),
            vec![SimulationAction::ReleaseButtons]
        );
        button(&ctx, &mut app, outside, false);
        frame(
            &ctx,
            &mut app,
            vec![Event::WindowFocused(true)],
            Modifiers::NONE,
        );
        button(&ctx, &mut app, pos, true);
        assert_eq!(
            app.take_simulation_actions(),
            vec![SimulationAction::PressButton { id }]
        );
        app.settings_open = true;
        frame(&ctx, &mut app, vec![], Modifiers::NONE);
        assert_eq!(
            app.take_simulation_actions(),
            vec![SimulationAction::ReleaseButtons]
        );
        assert_eq!(app.project(), &before);
        assert!(!app.editor.can_undo());
    }
}

#[test]
fn switch_and_dip_clicks_are_runtime_only_and_follow_oriented_cells() {
    for facing in logisim_core::Facing::ALL {
        for kind in [Kind::Switch, Kind::DipSwitch] {
            let (ctx, mut app) = setup();
            let mut c = Component::new(kind, Point::new(200, 200));
            c.facing = facing;
            let mut circuit = logisim_core::Circuit::default();
            let id = circuit.add_component(c);
            app.replace_circuit(circuit);
            app.editor.set_tool(Tool::Poke);
            app.editor.simulation.view.inputs.insert(
                id,
                InputValue {
                    bits: if kind == Kind::Switch { 1 } else { 4 },
                    value: 0,
                    ..Default::default()
                },
            );
            frame(&ctx, &mut app, vec![], Modifiers::NONE);
            let before = app.project().clone();
            let body = drawing::bounds(app.circuit().component(id).unwrap());
            for bit in if kind == Kind::Switch {
                vec![0]
            } else {
                vec![0, 3]
            } {
                let at = if kind == Kind::Switch {
                    body.center().to_vec2()
                } else {
                    body.min.to_vec2()
                        + if matches!(
                            facing,
                            logisim_core::Facing::East | logisim_core::Facing::West
                        ) {
                            vec2(15.0, 10.0 + 20.0 * f32::from(bit))
                        } else {
                            vec2(10.0 + 20.0 * f32::from(bit), 15.0)
                        }
                };
                let pos = app.canvas.screen_vec(at);
                button(&ctx, &mut app, pos, true);
                assert!(app.take_simulation_actions().is_empty());
                button(&ctx, &mut app, pos, false);
                assert_eq!(
                    app.take_simulation_actions(),
                    vec![SimulationAction::ToggleInput { id, bit }]
                );
            }
            assert_eq!(app.project(), &before);
            assert!(!app.editor.can_undo());
        }
    }
}

#[test]
fn slider_drag_is_persistent_one_undo_clamped_reversible_and_cancelable() {
    for facing in logisim_core::Facing::ALL {
        for reverse in [false, true] {
            let (ctx, mut app) = setup();
            let mut c = Component::new(Kind::Slider, Point::new(300, 200));
            c.facing = facing;
            c.bits = 32;
            c.io.slider_reverse = reverse;
            let mut circuit = logisim_core::Circuit::default();
            let id = circuit.add_component(c);
            app.replace_circuit(circuit);
            app.editor.set_tool(Tool::Poke);
            frame(&ctx, &mut app, vec![], Modifiers::NONE);
            let before = app.project().clone();
            let c = app.circuit().component(id).unwrap();
            let min = drawing::bounds(c).min.to_vec2();
            let mid = app.canvas.screen_vec(min + vec2(60.0, 20.0));
            click(&ctx, &mut app, mid);
            assert_eq!(app.circuit().component(id).unwrap().io.slider_value, 0);
            for _ in 0..2 {
                frame(&ctx, &mut app, vec![], Modifiers::NONE);
            }
            let start = app
                .canvas
                .screen_vec(crate::io::slider_knob(app.circuit().component(id).unwrap()));
            let mid = app.canvas.screen_vec(min + vec2(60.0, 20.0));
            drag(&ctx, &mut app, start, mid);
            assert_eq!(
                app.circuit().component(id).unwrap().io.slider_value,
                0x80000000
            );
            assert_ne!(app.project(), &before);
            assert!(app.take_simulation_actions().is_empty());
            key(&ctx, &mut app, Key::Z, Modifiers::COMMAND);
            assert_eq!(app.project(), &before);
            assert!(!app.editor.can_undo());
            key(
                &ctx,
                &mut app,
                Key::Z,
                Modifiers::COMMAND.plus(Modifiers::SHIFT),
            );
            assert_eq!(
                app.circuit().component(id).unwrap().io.slider_value,
                0x80000000
            );
            let end = app
                .canvas
                .screen_vec(min + vec2(if reverse { -50.0 } else { 170.0 }, 20.0));
            drag(&ctx, &mut app, mid, end);
            assert_eq!(
                app.circuit().component(id).unwrap().io.slider_value,
                u32::MAX
            );
            let start = app
                .canvas
                .screen_vec(crate::io::slider_knob(app.circuit().component(id).unwrap()));
            button(&ctx, &mut app, start, true);
            frame(
                &ctx,
                &mut app,
                vec![Event::PointerMoved(mid)],
                Modifiers::NONE,
            );
            assert_eq!(
                app.circuit().component(id).unwrap().io.slider_value,
                0x80000000
            );
            key(&ctx, &mut app, Key::Escape, Modifiers::NONE);
            button(&ctx, &mut app, mid, false);
            assert_eq!(
                app.circuit().component(id).unwrap().io.slider_value,
                u32::MAX
            );
            button(&ctx, &mut app, start, true);
            frame(
                &ctx,
                &mut app,
                vec![Event::PointerMoved(mid)],
                Modifiers::NONE,
            );
            frame(
                &ctx,
                &mut app,
                vec![Event::WindowFocused(false)],
                Modifiers::NONE,
            );
            assert_eq!(
                app.circuit().component(id).unwrap().io.slider_value,
                u32::MAX
            );
        }
    }
}
