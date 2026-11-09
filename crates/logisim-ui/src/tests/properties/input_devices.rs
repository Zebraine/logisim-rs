use crate::tests::support::widgets::*;
use crate::{editor::Tool, *};
use egui::{Context, Event, Modifiers, PointerButton, Pos2};
use logisim_core::{Component, Kind, Point};

#[test]
fn joystick_library_properties_and_rotation_undo_in_both_languages() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        click(&ctx, &mut app, &t("Input/Output"));
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(Pos2::new(150.0, 350.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -200.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: Modifiers::NONE,
                    source: egui::MouseWheelSource::Unknown,
                },
            ],
        );
        click(&ctx, &mut app, &t("Joystick"));
        assert_eq!(app.editor.tool, Tool::Place(Kind::Joystick));
        let labels = frame(&ctx, &mut app, vec![]);
        for s in ["Data bits", "Facing", "Stick color", "Label color"] {
            assert!(has(&labels, &t(s)), "{s}");
        }
        for s in ["Label", "Label location", "Font size", "Off color"] {
            assert!(!has(&labels, &t(s)), "{s}");
        }
        app.editor.place(Point::new(300, 200));
        app.editor.set_tool(Tool::Select);
        let id = app.circuit().components[0].id;
        app.editor.selection.insert(id);
        app.editor
            .circuit
            .add_wire(Point::new(300, 210), Point::new(400, 210));
        let before = app.circuit().clone();
        click(&ctx, &mut app, &t("East"));
        click(&ctx, &mut app, &t("North"));
        assert_eq!(
            app.circuit().component(id).unwrap().facing,
            logisim_core::Facing::North
        );
        assert_eq!(app.circuit().wires, before.wires);
        app.editor.undo();
        assert_eq!(app.circuit(), &before);
        app.editor.redo();
        assert_eq!(
            app.circuit().component(id).unwrap().facing,
            logisim_core::Facing::North
        );
    }
}

#[test]
fn io_library_properties_and_momentary_inspector_work_in_both_languages() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |s: &str| i18n::with_language(language, || i18n::tr(s).to_owned());
        click(&ctx, &mut app, &translated("Input/Output"));
        for kind in [
            Kind::Button,
            Kind::Switch,
            Kind::DipSwitch,
            Kind::Slider,
            Kind::Led,
            Kind::RgbLed,
            Kind::SevenSegment,
            Kind::HexDisplay,
            Kind::DotMatrix,
        ] {
            if matches!(kind, Kind::DipSwitch | Kind::RgbLed | Kind::DotMatrix) {
                frame(
                    &ctx,
                    &mut app,
                    vec![
                        Event::PointerMoved(Pos2::new(130.0, 350.0)),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0.0, -170.0),
                            modifiers: Modifiers::NONE,
                            phase: egui::TouchPhase::Move,
                            source: egui::MouseWheelSource::Unknown,
                        },
                    ],
                );
                for _ in 0..20 {
                    frame(&ctx, &mut app, vec![]);
                }
            }
            click(&ctx, &mut app, &translated(kind.name()));
            assert_eq!(app.editor.tool, Tool::Place(kind));
            let labels = frame(&ctx, &mut app, vec![]);
            assert!(has(
                &labels,
                &translated(if kind == Kind::Button {
                    "Button color"
                } else if matches!(kind, Kind::Switch | Kind::DipSwitch | Kind::Slider) {
                    "Body color"
                } else if kind == Kind::RgbLed {
                    "8-bit color channels"
                } else {
                    "On color"
                })
            ));
            assert_eq!(
                has(&labels, &translated("Data bits")),
                matches!(kind, Kind::Switch | Kind::Slider)
            );
            if kind == Kind::DipSwitch {
                assert!(has(&labels, &translated("Number of switches")));
            }
            if kind == Kind::Slider {
                assert!(has(&labels, &translated("Value (hex)")));
                click(&ctx, &mut app, &translated("Left to right"));
                click(&ctx, &mut app, &translated("Right to left"));
                assert!(app.editor.template.io.slider_reverse);
                click(&ctx, &mut app, &translated("Binary"));
                click(&ctx, &mut app, &translated("Hexadecimal"));
                assert_eq!(app.editor.template.radix, logisim_core::Radix::Hex);
            }
            if matches!(kind, Kind::Led | Kind::SevenSegment) {
                let rect = labels
                    .iter()
                    .find(|(s, _)| s == &translated("Active on high"))
                    .unwrap()
                    .1;
                let checkbox = labels
                    .iter()
                    .find(|(s, r)| s.is_empty() && (r.center().y - rect.center().y).abs() < 1.0)
                    .unwrap()
                    .1;
                let pos = Pos2::new(checkbox.left() - 10.0, rect.center().y);
                for pressed in [true, false] {
                    frame(
                        &ctx,
                        &mut app,
                        vec![
                            Event::PointerMoved(pos),
                            Event::PointerButton {
                                pos,
                                button: PointerButton::Primary,
                                pressed,
                                modifiers: Modifiers::NONE,
                            },
                        ],
                    );
                }
                assert!(!app.editor.template.io.active_high);
            }
        }
        let mut circuit = logisim_core::Circuit::default();
        let id = circuit.add_component(Component::new(Kind::Button, Point::new(200, 200)));
        app.replace_circuit(circuit);
        app.editor.selection.insert(id);
        let before = app.project().clone();
        let mut labels = frame(&ctx, &mut app, vec![]);
        for _ in 0..2 {
            labels = frame(&ctx, &mut app, vec![]);
        }
        let pos = labels
            .iter()
            .find(|(s, _)| s == &translated("Hold to press"))
            .unwrap()
            .1
            .center();
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        assert_eq!(
            app.take_simulation_actions(),
            vec![SimulationAction::PressButton { id }]
        );
        let pos = egui::pos2(1100.0, 750.0);
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        assert_eq!(
            app.take_simulation_actions(),
            vec![SimulationAction::ReleaseButtons]
        );
        assert_eq!(app.project(), &before);
        assert!(!app.editor.can_undo());
    }
}

#[test]
fn centered_button_label_is_editable_and_drawn_inside_the_button_in_both_languages() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        app.editor.set_tool(Tool::Place(Kind::Button));
        app.editor.place(Point::new(300, 300));
        app.editor.circuit.components[0].label = "C".into();
        let id = app.editor.circuit.components[0].id;
        click(&ctx, &mut app, &t("North"));
        click(&ctx, &mut app, &t("Center"));
        assert!(app.circuit().component(id).unwrap().label_center);
        let labels = frame(&ctx, &mut app, vec![]);
        let label = labels
            .iter()
            .find(|(s, r)| s == "C" && r.center().x > 300.0)
            .unwrap()
            .1;
        let center = app.canvas.screen_vec(
            crate::drawing::bounds(app.circuit().component(id).unwrap())
                .center()
                .to_vec2(),
        );
        assert!(label.center().distance(center) < 1.0);
        app.editor.undo();
        assert!(!app.circuit().component(id).unwrap().label_center);
        app.editor.redo();
        assert!(app.circuit().component(id).unwrap().label_center);
    }
}
