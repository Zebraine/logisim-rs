use crate::tests::support::widgets::*;
use crate::*;
use egui::{Context, Event, Modifiers, PointerButton, Pos2};
use logisim_core::{Component, Kind, Point};

#[test]
fn display_properties_edit_real_widgets_with_localization_undo_and_stable_ids() {
    use logisim_core::MatrixInput;
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |s: &str| i18n::with_language(language, || i18n::tr(s).to_owned());
        let mut circuit = logisim_core::Circuit::default();
        let matrix = circuit.add_component(Component::new(Kind::DotMatrix, Point::new(300, 300)));
        let rgb = circuit.add_component(Component::new(Kind::RgbLed, Point::new(100, 200)));
        app.replace_circuit(circuit);
        app.editor.session.selection.insert(matrix);
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        let labels = frame(&ctx, &mut app, vec![]);
        for text in [
            "Matrix columns",
            "Matrix rows",
            "Light persistence (ticks)",
            "Dot shape",
        ] {
            assert!(has(&labels, &translated(text)), "{text}");
        }
        assert!(!has(&labels, &translated("Data bits")));
        click(&ctx, &mut app, &translated("Columns"));
        click(&ctx, &mut app, &translated("Select rows/columns"));
        assert_eq!(
            app.editor
                .session
                .circuit
                .component(matrix)
                .unwrap()
                .io
                .matrix_input,
            MatrixInput::Select
        );
        click(&ctx, &mut app, &translated("Square"));
        click(&ctx, &mut app, &translated("Circular"));
        assert!(
            app.editor
                .session
                .circuit
                .component(matrix)
                .unwrap()
                .io
                .matrix_circular
        );
        app.editor.undo();
        assert!(
            !app.editor
                .session
                .circuit
                .component(matrix)
                .unwrap()
                .io
                .matrix_circular
        );
        app.editor.redo();
        assert!(
            app.editor
                .session
                .circuit
                .component(matrix)
                .unwrap()
                .io
                .matrix_circular
        );
        app.editor.session.selection.clear();
        app.editor.session.selection.insert(rgb);
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, &translated("Label location")));
        assert!(!has(&labels, &translated("On color")));
        let rect = labels
            .iter()
            .find(|(s, _)| s == &translated("8-bit color channels"))
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
        assert!(
            app.editor
                .session
                .circuit
                .component(rgb)
                .unwrap()
                .io
                .rgb_multibit
        );
        assert_eq!(
            app.editor
                .session
                .circuit
                .component(rgb)
                .unwrap()
                .port_width(0),
            Some(8)
        );
        app.editor.undo();
        assert_eq!(
            app.editor
                .session
                .circuit
                .component(rgb)
                .unwrap()
                .port_width(0),
            Some(1)
        );
    }
}
