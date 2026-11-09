use crate::tests::support::widgets::*;
use crate::{editor::Tool, *};
use egui::{Context, Event, Key, Modifiers, Pos2};
use logisim_core::{Kind, Point};

#[test]
fn arithmetic_library_and_modes_edit_in_both_languages_with_undo() {
    use logisim_core::{Arithmetic, Shift};
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |text| i18n::with_language(language, || i18n::tr(text).to_owned());
        click(&ctx, &mut app, &translated("Arithmetic"));
        for kind in Arithmetic::ALL {
            if kind == Arithmetic::Negator {
                // The expanded library exceeds the project pane: use its real scroll path.
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
            assert_eq!(app.editor.tool, Tool::Place(Kind::Arithmetic(kind)));
            let labels = frame(&ctx, &mut app, vec![]);
            assert!(has(&labels, &translated("Data bits")));
            assert!(!has(&labels, &translated("Facing")));
            assert!(!has(&labels, &translated("Label")));
            assert_eq!(
                has(&labels, &translated("Numeric type")),
                kind == Arithmetic::Comparator
            );
            assert_eq!(
                has(&labels, &translated("Shift type")),
                kind == Arithmetic::Shifter
            );
            assert_eq!(
                has(&labels, &translated("Search type")),
                kind == Arithmetic::BitFinder
            );
            assert_eq!(
                has(&labels, &translated("Inputs")),
                kind == Arithmetic::BitAdder
            );
        }
        click(&ctx, &mut app, &translated("Shifter"));
        for shift in Shift::ALL.into_iter().skip(1) {
            let previous = app.editor.template.arithmetic.shift;
            click(&ctx, &mut app, &translated(previous.name()));
            click(&ctx, &mut app, &translated(shift.name()));
            assert_eq!(app.editor.template.arithmetic.shift, shift);
        }
        click(&ctx, &mut app, &translated("Comparator"));
        click(&ctx, &mut app, &translated("Two's complement"));
        click(&ctx, &mut app, &translated("Unsigned"));
        assert!(!app.editor.template.arithmetic.signed);
        app.editor.place(Point::new(300, 300));
        app.editor.set_tool(Tool::Select);
        let id = app.circuit().components[0].id;
        app.editor.session.selection.insert(id);
        click(&ctx, &mut app, &translated("Unsigned"));
        click(&ctx, &mut app, &translated("Two's complement"));
        assert!(app.circuit().component(id).unwrap().arithmetic.signed);
        app.editor.undo();
        assert!(!app.circuit().component(id).unwrap().arithmetic.signed);
        app.editor.redo();
        assert!(app.circuit().component(id).unwrap().arithmetic.signed);
    }
}

#[test]
fn bit_search_and_input_count_edits_use_translated_widgets_and_undo() {
    use logisim_core::{Arithmetic, BitSearch};
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
        let translated = |text| i18n::with_language(language, || i18n::tr(text).to_owned());
        app.editor
            .set_tool(Tool::Place(Kind::Arithmetic(Arithmetic::BitFinder)));
        for mode in BitSearch::ALL.into_iter().skip(1) {
            let old = app.editor.template.arithmetic.search;
            click(&ctx, &mut app, &translated(old.name()));
            click(&ctx, &mut app, &translated(mode.name()));
            assert_eq!(app.editor.template.arithmetic.search, mode);
        }
        app.editor.place(Point::new(300, 200));
        app.editor.set_tool(Tool::Select);
        let id = app.circuit().components[0].id;
        app.editor.session.selection.insert(id);
        click(&ctx, &mut app, &translated(BitSearch::HighestZero.name()));
        click(&ctx, &mut app, &translated(BitSearch::LowestOne.name()));
        assert_eq!(
            app.circuit().component(id).unwrap().arithmetic.search,
            BitSearch::LowestOne
        );
        app.editor.undo();
        assert_eq!(
            app.circuit().component(id).unwrap().arithmetic.search,
            BitSearch::HighestZero
        );
        app.editor.redo();
        assert_eq!(
            app.circuit().component(id).unwrap().arithmetic.search,
            BitSearch::LowestOne
        );

        app.editor
            .set_tool(Tool::Place(Kind::Arithmetic(Arithmetic::BitAdder)));
        number(&ctx, &mut app, "1", "4");
        assert_eq!(app.editor.template.arithmetic.bit_inputs, 4);
        assert_eq!(app.editor.template.arithmetic_output_bits(), 6);
        app.editor.place(Point::new(300, 400));
        app.editor.set_tool(Tool::Select);
        let c = app.circuit().components.last().unwrap().clone();
        app.editor.session.selection.insert(c.id);
        for port in [c.ports()[1], c.ports()[4]] {
            app.editor
                .session
                .circuit
                .add_wire(port - Point::new(80, 0), port);
        }
        let before = app.circuit().clone();
        number(&ctx, &mut app, "4", "2");
        assert_eq!(
            app.circuit().component(c.id).unwrap().arithmetic.bit_inputs,
            2
        );
        assert_eq!(app.circuit().wires, before.wires);
        app.editor.undo();
        assert_eq!(app.circuit(), &before);
        app.editor.redo();
        assert_eq!(
            app.circuit().component(c.id).unwrap().arithmetic.bit_inputs,
            2
        );
    }
}
