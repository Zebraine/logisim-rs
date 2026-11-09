use crate::tests::support::widgets::*;
use crate::{editor::Tool, *};
use egui::{Context, Event, Key, Modifiers, PointerButton, Pos2};
use logisim_core::{Kind, Point};

#[test]
fn latch_library_modes_layout_controls_and_undo_work_in_both_languages() {
    use logisim_core::{ControlPosition, Latch, LatchEnable};
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        click(&ctx, &mut app, &t("Memory"));
        click(&ctx, &mut app, &t("D Latch"));
        assert_eq!(app.editor.tool, Tool::Place(Kind::Latch(Latch::D)));
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(!has(&labels, &t("Facing")));
        assert!(!has(&labels, &t("Data bits")));
        click(&ctx, &mut app, &t("High level"));
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(!has(&labels, &t("Without enable")));
        assert!(!has(&labels, &t("Rising edge")));
        click(&ctx, &mut app, &t("Low level"));
        assert_eq!(app.editor.template.latch_enable, LatchEnable::Low);
        click(&ctx, &mut app, &t("S-R Latch"));
        app.editor.place(Point::new(300, 200));
        app.editor.set_tool(Tool::Select);
        let id = app.circuit().components[0].id;
        app.editor.session.selection.insert(id);
        app.editor
            .session
            .circuit
            .add_wire(Point::new(100, 200), Point::new(240, 200));
        let before = app.circuit().clone();
        click(&ctx, &mut app, &t("High level"));
        click(&ctx, &mut app, &t("Without enable"));
        assert!(!app.circuit().component(id).unwrap().latch_has_enable());
        assert_eq!(app.circuit().wires, before.wires);
        app.editor.undo();
        assert_eq!(app.circuit(), &before);
        app.editor.redo();
        assert!(!app.circuit().component(id).unwrap().latch_has_enable());
        for (label, expected) in [
            ("New layout", false),
            ("New layout", true),
            ("Active-low preset / clear", true),
        ] {
            for _ in 0..3 {
                frame(&ctx, &mut app, vec![]);
            }
            let labels = frame(&ctx, &mut app, vec![]);
            let row = labels.iter().find(|(s, _)| s == &t(label)).unwrap().1;
            let checkbox = labels
                .iter()
                .find(|(s, r)| s.is_empty() && (r.center().y - row.center().y).abs() < 1.0)
                .unwrap()
                .1;
            let pos = Pos2::new(checkbox.left() - 10.0, row.center().y);
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
            let c = app.circuit().component(id).unwrap();
            if label == "New layout" {
                assert_eq!(c.sequential.new_layout, expected);
                assert_eq!(
                    c.sequential.controls,
                    if expected {
                        ControlPosition::BelowAbove
                    } else {
                        ControlPosition::Legacy
                    }
                );
                if !expected {
                    // The old layout still allows all three control positions.
                    click(&ctx, &mut app, &t("Legacy"));
                    click(&ctx, &mut app, &t("Below / above"));
                    assert_eq!(
                        app.circuit().component(id).unwrap().sequential.controls,
                        ControlPosition::BelowAbove
                    );
                }
            } else {
                assert_eq!(c.sequential.negate_controls, expected);
            }
            assert_eq!(app.circuit().wires, before.wires);
        }
    }
}

#[test]
fn sequential_library_properties_and_controls_work_in_both_languages() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |text| i18n::with_language(language, || i18n::tr(text).to_owned());
        click(&ctx, &mut app, &translated("Memory"));
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(Pos2::new(170.0, 360.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -150.0),
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        click(&ctx, &mut app, &translated("J-K Flip-Flop"));
        assert_eq!(
            app.editor.tool,
            Tool::Place(Kind::FlipFlop(logisim_core::FlipFlop::Jk))
        );
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, &translated("Trigger")));
        click(&ctx, &mut app, &translated("Rising edge"));
        click(&ctx, &mut app, &translated("Falling edge"));
        assert_eq!(
            app.editor.template.sequential.trigger,
            logisim_core::Trigger::Falling
        );
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(Pos2::new(170.0, 360.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -200.0),
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        click(&ctx, &mut app, &translated("Register"));
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, &translated("Chip select active on")));
        app.editor.simulation.view.has_clocks = true;
        click(&ctx, &mut app, &translated("Tick"));
        click(&ctx, &mut app, &translated("Auto ticks"));
        assert_eq!(
            app.take_simulation_actions(),
            vec![SimulationAction::Tick, SimulationAction::ToggleTicks]
        );
        assert!(!app.editor.can_undo());
    }
}

#[test]
fn counter_shift_widgets_edit_modes_width_and_undo_in_both_languages() {
    fn number(ctx: &Context, app: &mut Workspace, old: &str, new: &str) {
        click(ctx, app, old);
        frame(
            ctx,
            app,
            vec![
                Event::ModifiersChanged(Modifiers::COMMAND),
                Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                },
                Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                },
                Event::ModifiersChanged(Modifiers::NONE),
                Event::Text(new.into()),
                Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        frame(
            ctx,
            app,
            vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
    }
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        app.editor.set_tool(Tool::Place(Kind::Counter));
        app.editor.template.sequential.counter_max = 5;
        number(&ctx, &mut app, "8", "4");
        assert_eq!(app.editor.template.bits, 4);
        assert_eq!(app.editor.template.sequential.counter_max, 15);
        for action in logisim_core::CounterOverflow::ALL.into_iter().skip(1) {
            let previous = t(app.editor.template.sequential.counter_overflow.name());
            click(&ctx, &mut app, &previous);
            click(&ctx, &mut app, &t(action.name()));
            assert_eq!(app.editor.template.sequential.counter_overflow, action);
        }
        app.editor.place(Point::new(300, 200));
        app.editor.set_tool(Tool::Select);
        let id = app.circuit().components[0].id;
        app.editor.session.selection.insert(id);
        app.editor
            .session
            .circuit
            .add_wire(Point::new(100, 210), Point::new(270, 210));
        let before = app.circuit().clone();
        click(&ctx, &mut app, &t("New behavior"));
        click(&ctx, &mut app, &t("Old behavior"));
        assert!(app.circuit().component(id).unwrap().sequential.counter_old);
        assert_eq!(app.circuit().wires, before.wires);
        app.editor.undo();
        assert_eq!(app.circuit(), &before);
        app.editor.redo();
        assert!(app.circuit().component(id).unwrap().sequential.counter_old);
        click(&ctx, &mut app, &t("Rising edge"));
        click(&ctx, &mut app, &t("Falling edge"));
        assert_eq!(
            app.circuit().component(id).unwrap().sequential.trigger,
            logisim_core::Trigger::Falling
        );
        app.editor.set_tool(Tool::Place(Kind::ShiftRegister));
        number(&ctx, &mut app, "8", "32");
        assert_eq!(app.editor.template.bits, 32);
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, &t("Number of stages")));
        assert!(!has(&labels, &t("Data bits")));
        assert!(!has(&labels, &t("Facing")));
        app.editor.place(Point::new(500, 200));
        assert_eq!(app.circuit().components.last().unwrap().bits, 32);
    }
}

#[test]
fn random_library_seed_trigger_and_undo_are_available_in_both_languages() {
    fn number(ctx: &Context, app: &mut Workspace, old: &str, new: &str) {
        click(ctx, app, old);
        frame(
            ctx,
            app,
            vec![
                Event::ModifiersChanged(Modifiers::COMMAND),
                Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                },
                Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                },
                Event::ModifiersChanged(Modifiers::NONE),
                Event::Text(new.into()),
                Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        frame(
            ctx,
            app,
            vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
    }

    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        click(&ctx, &mut app, &t("Memory"));
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(Pos2::new(150.0, 350.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -300.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: Modifiers::NONE,
                    source: egui::MouseWheelSource::Unknown,
                },
            ],
        );
        click(&ctx, &mut app, &t("Random Generator"));
        assert_eq!(app.editor.tool, Tool::Place(Kind::Random));
        let labels = frame(&ctx, &mut app, vec![]);
        for key in ["Data bits", "Trigger", "Seed (0 = automatic)", "Label"] {
            assert!(has(&labels, &t(key)), "{key}");
        }
        assert!(!has(&labels, &t("Facing")));
        number(&ctx, &mut app, "0", "-2147483648");
        assert_eq!(app.editor.template.sequential.random_seed, i32::MIN);
        app.editor.place(Point::new(300, 200));
        app.editor.set_tool(Tool::Select);
        let c = app.circuit().components[0].clone();
        app.editor.session.selection.insert(c.id);
        app.editor
            .session
            .circuit
            .add_wire(c.ports()[0], c.ports()[0] + Point::new(100, 0));
        let before = app.circuit().clone();
        click(&ctx, &mut app, &t("Rising edge"));
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(!has(&labels, &t("High level")));
        click(&ctx, &mut app, &t("Falling edge"));
        assert_eq!(
            app.circuit().component(c.id).unwrap().sequential.trigger,
            logisim_core::Trigger::Falling
        );
        assert_eq!(app.circuit().wires, before.wires);
        app.editor.undo();
        assert_eq!(app.circuit(), &before);
        app.editor.redo();
        assert_eq!(
            app.circuit().component(c.id).unwrap().sequential.trigger,
            logisim_core::Trigger::Falling
        );
    }
}
