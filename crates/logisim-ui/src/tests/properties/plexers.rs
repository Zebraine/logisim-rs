use crate::tests::support::widgets::*;
use crate::{editor::Tool, *};
use egui::{Context, Event, Key, Modifiers, PointerButton, Pos2};
use logisim_core::{Kind, Point};

#[test]
fn plexers_library_and_output_properties_work_in_both_languages() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |text| i18n::with_language(language, || i18n::tr(text).to_owned());
        click(&ctx, &mut app, &translated("Plexers"));
        for kind in logisim_core::Plexer::ALL {
            if kind == logisim_core::Plexer::DisplayDecoder {
                frame(
                    &ctx,
                    &mut app,
                    vec![
                        Event::PointerMoved(Pos2::new(130.0, 350.0)),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0.0, -100.0),
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
            assert_eq!(app.editor.tool, Tool::Place(Kind::Plexer(kind)));
            let labels = frame(&ctx, &mut app, vec![]);
            assert_eq!(
                has(&labels, &translated("Select bits")),
                kind != logisim_core::Plexer::DisplayDecoder
            );
            assert_eq!(
                has(&labels, &translated("Bus input")),
                kind == logisim_core::Plexer::DisplayDecoder
            );
            assert_eq!(has(&labels, &translated("Data bits")), kind.has_data_bits());
            assert_eq!(
                has(&labels, &translated("No input state")),
                kind == logisim_core::Plexer::PriorityEncoder
            );
        }
        click(&ctx, &mut app, &translated("Multiplexer"));
        click(&ctx, &mut app, &translated("Floating"));
        click(&ctx, &mut app, &translated("Zero output"));
        assert!(app.editor.template.plexing.disabled_zero);
        click(&ctx, &mut app, &translated("Bottom / left"));
        click(&ctx, &mut app, &translated("Top / right"));
        assert!(app.editor.template.plexing.top_right);
        app.editor.place(Point::new(300, 300));
        app.editor.set_tool(Tool::Select);
        let id = app.circuit().components[0].id;
        app.editor.selection.insert(id);
        click(&ctx, &mut app, &translated("Zero output"));
        click(&ctx, &mut app, &translated("Floating"));
        assert!(!app.circuit().component(id).unwrap().plexing.disabled_zero);
        app.editor.undo();
        assert!(app.circuit().component(id).unwrap().plexing.disabled_zero);
        app.editor.redo();
        assert!(!app.circuit().component(id).unwrap().plexing.disabled_zero);
    }
}

#[test]
fn extra_plexer_property_widgets_edit_groups_and_formats_with_undo_in_both_languages() {
    use logisim_core::Plexer;
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        app.editor
            .set_tool(Tool::Place(Kind::Plexer(Plexer::BitSelector)));
        click(&ctx, &mut app, "1");
        frame(
            &ctx,
            &mut app,
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
                Event::Text("3".into()),
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
            &ctx,
            &mut app,
            vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert_eq!(app.editor.template.plexing.group_bits, 3);
        assert_eq!(app.editor.template.bit_selector_select_bits(), 2);
        click(&ctx, &mut app, &translated("Bottom / left"));
        click(&ctx, &mut app, &translated("Top / right"));
        app.editor.place(Point::new(300, 200));
        assert!(app.circuit().components[0].plexing.top_right);

        app.editor
            .set_tool(Tool::Place(Kind::Plexer(Plexer::DisplayDecoder)));
        app.editor.place(Point::new(300, 400));
        app.editor.set_tool(Tool::Select);
        let c = app.circuit().components.last().unwrap().clone();
        app.editor.selection.insert(c.id);
        app.editor
            .circuit
            .add_wire(Point::new(160, 400), Point::new(260, 400));
        let before = app.circuit().clone();
        let mut labels = vec![];
        for _ in 0..3 {
            labels = frame(&ctx, &mut app, vec![]);
        }
        assert!(!has(&labels, &translated("Data bits")));
        let x = labels
            .iter()
            .find(|(t, _)| *t == translated("East"))
            .unwrap()
            .1
            .min
            .x
            + 2.0;
        let y = labels
            .iter()
            .find(|(t, _)| *t == translated("Bus input"))
            .unwrap()
            .1
            .center()
            .y;
        for pressed in [true, false] {
            let pos = Pos2::new(x, y);
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
        assert!(!app.circuit().component(c.id).unwrap().plexing.multibit);
        assert_eq!(app.circuit().wires, before.wires);
        app.editor.undo();
        assert_eq!(app.circuit(), &before);
        app.editor.redo();
        assert!(!app.circuit().component(c.id).unwrap().plexing.multibit);
    }
}
