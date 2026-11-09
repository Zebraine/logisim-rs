use crate::tests::support::widgets::*;
use crate::{editor::Tool, *};
use egui::Context;
use logisim_core::{Kind, Point};

#[test]
fn character_terminal_properties_translate_and_edit_without_moving_wires() {
    for language in Language::ALL {
        for kind in [Kind::Keyboard, Kind::Tty] {
            let ctx = Context::default();
            configure(&ctx);
            let mut app = Workspace::default();
            app.set_language(language);
            let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
            app.editor.set_tool(Tool::Place(kind));
            app.editor.place(Point::new(300, 300));
            let id = app.circuit().components[0].id;
            app.editor
                .session
                .circuit
                .add_wire(Point::new(200, 300), Point::new(300, 300));
            let before = app.circuit().clone();
            let labels = frame(&ctx, &mut app, vec![]);
            for s in [
                "Character encoding",
                "Trigger",
                if kind == Kind::Keyboard {
                    "Buffer length"
                } else {
                    "Display rows"
                },
            ] {
                assert!(has(&labels, &t(s)), "{s}");
            }
            for s in ["Facing", "Label", "Data bits"] {
                assert!(!has(&labels, &t(s)), "{s}");
            }
            click(&ctx, &mut app, &t("ASCII (7-bit)"));
            click(&ctx, &mut app, &t("UTF-16 (16-bit)"));
            assert_eq!(app.circuit().component(id).unwrap().bits, 16);
            assert_eq!(app.circuit().wires, before.wires);
            app.editor.undo();
            assert_eq!(app.circuit(), &before);
            app.editor.redo();
            click(&ctx, &mut app, &t("Rising edge"));
            click(&ctx, &mut app, &t("Falling edge"));
            assert_eq!(
                app.circuit().component(id).unwrap().terminal.trigger,
                logisim_core::Trigger::Falling
            );
            assert_eq!(app.circuit().wires, before.wires);
        }
    }
}
