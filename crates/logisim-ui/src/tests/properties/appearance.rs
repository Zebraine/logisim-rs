use crate::tests::support::widgets::*;
use crate::*;
use egui::{Context, Event, Key, Modifiers};
use logisim_core::{Component, Kind, Point};

#[test]
fn selection_inspector_and_both_appearance_menus_use_the_active_bindings() {
    use crate::shortcuts::{Command, Group};
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        let mut circuit = logisim_core::Circuit::default();
        for x in [100, 200] {
            circuit.add_component(Component::new(Kind::Pin, Point::new(x, 100)));
        }
        app.replace_project(logisim_core::Project::single("main", circuit));
        app.set_language(language);
        let mut settings = app.preferences();
        for (command, chord) in [
            (Command::ToolRotate, "f10"),
            (Command::AppearanceUndo, "f6"),
            (Command::AppearanceCopy, "f5"),
            (Command::AppearanceRaise, "f7"),
        ] {
            settings
                .shortcuts
                .bind(command, chord.parse().unwrap())
                .unwrap();
        }
        app.apply_settings(settings);
        app.editor.select_all();
        let label = |s| language.translate(s);
        let caption = |command: Command| {
            format!(
                "{} · {}",
                label(command.label()),
                settings.shortcuts.get(command).label(ctx.os().is_mac())
            )
        };
        assert!(has(
            &frame(&ctx, &mut app, vec![]),
            &caption(Command::ToolRotate)
        ));
        app.editor.session.context.appearance_mode = true;
        frame(&ctx, &mut app, vec![]);
        app.appearance
            .command(&mut app.editor, crate::appearance::Command::All);
        assert!(has(
            &frame(&ctx, &mut app, vec![]),
            &caption(Command::AppearanceRaise)
        ));

        let expected: Vec<_> = Command::ALL
            .into_iter()
            .filter(|command| command.group() == Group::Appearance)
            .map(caption)
            .chain(["Delete", "Add Vertex", "Remove Vertex"].map(|s| label(s).to_owned()))
            .collect();
        click(&ctx, &mut app, label("Edit"));
        let menu = frame(&ctx, &mut app, vec![]);
        for text in &expected {
            assert!(
                menu.iter().any(|(s, rect)| s == text && rect.top() < 600.),
                "{text}"
            );
        }
        click(&ctx, &mut app, label("Edit"));
        let pos = app.canvas.rect.center();
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut app,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Secondary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        let menu = frame(&ctx, &mut app, vec![]);
        for text in &expected {
            assert!(
                menu.iter()
                    .any(|(s, rect)| s == text && rect.left() >= app.canvas.rect.left()),
                "{text}"
            );
        }
    }
}

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
