use crate::tests::support::widgets::*;
use crate::*;
use egui::{Context, Event, Key, Modifiers};
use logisim_core::{Component, Kind, Point};

#[test]
fn translated_properties_and_appearance_preserve_user_names_and_shortcuts() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        let mut circuit = logisim_core::Circuit::default();
        let mut text = Component::new(Kind::Text, Point::new(100, 100));
        text.label = "Save · 中文标签 {}".into();
        let id = circuit.add_component(text);
        app.replace_project(logisim_core::Project::single("File", circuit));
        app.set_language(language);
        app.editor.session.selection.insert(id);
        let project = app.project().clone();
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, "Save · 中文标签 {}"));
        let expected = i18n::with_language(language, || i18n::tr("Font size").to_owned());
        assert!(has(&labels, &expected));
        frame(
            &ctx,
            &mut app,
            vec![Event::Key {
                key: Key::E,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND.plus(Modifiers::SHIFT),
            }],
        );
        assert!(app.editor.session.context.appearance_mode);
        let labels = frame(&ctx, &mut app, vec![]);
        let expected = i18n::with_language(language, || tf!("Appearance: {}", "File"));
        assert!(has(&labels, &expected));
        assert_eq!(app.project(), &project);
        let document = DocumentView {
            title: "Save".into(),
            untitled: false,
            ..Default::default()
        };
        assert_eq!(
            i18n::with_language(language, || document.display_title().to_owned()),
            "Save"
        );
    }
}
