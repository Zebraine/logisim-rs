use crate::tests::support::widgets::*;
use crate::{editor::Tool, *};
use egui::{Context, Event, Key, Modifiers, PointerButton, Pos2};
use logisim_core::{Component, Kind, Point};

#[test]
fn memory_editor_real_input_keeps_rom_history_and_ram_instance_commands_separate() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |s: &str| i18n::with_language(language, || i18n::tr(s).to_owned());
        let mut circuit = logisim_core::Circuit::default();
        let rom = circuit.add_component(Component::new(Kind::Rom, Point::new(200, 100)));
        let ram = circuit.add_component(Component::new(Kind::Ram, Point::new(400, 100)));
        app.replace_circuit(circuit);
        app.editor.selection.insert(rom);
        click(&ctx, &mut app, &translated("Edit contents…"));
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, &translated("ROM contents")));
        let rect = labels
            .iter()
            .find(|(s, _)| s == &translated("Value (hex)"))
            .unwrap()
            .1;
        let pos = Pos2::new(rect.right() + 50.0, rect.center().y);
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
        frame(
            &ctx,
            &mut app,
            vec![
                Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                },
                Event::Text("A5".into()),
            ],
        );
        click(&ctx, &mut app, &translated("Write word"));
        assert_eq!(
            app.circuit().component(rom).unwrap().memory.contents.get(0),
            0xa5
        );
        assert!(app.take_simulation_actions().is_empty());
        click(&ctx, &mut app, &translated("Close"));
        app.editor.undo();
        assert_eq!(
            app.circuit().component(rom).unwrap().memory.contents.get(0),
            0
        );
        app.editor.redo();
        assert_eq!(
            app.circuit().component(rom).unwrap().memory.contents.get(0),
            0xa5
        );
        let before = app.project().clone();
        app.editor.selection.clear();
        app.editor.selection.insert(ram);
        let mut contents = logisim_core::MemoryContents::default();
        contents.set(0, 0x5a);
        app.editor.simulation.view.memories.insert(ram, contents);
        click(&ctx, &mut app, &translated("Edit contents…"));
        assert!(has(
            &frame(&ctx, &mut app, vec![]),
            &translated("RAM contents")
        ));
        click(&ctx, &mut app, &translated("Clear contents"));
        assert_eq!(
            app.take_simulation_actions(),
            vec![SimulationAction::FillMemory {
                id: ram,
                start: 0,
                end: 256,
                value: 0
            }]
        );
        assert_eq!(app.project(), &before);
        click(&ctx, &mut app, &translated("Close"));
        app.editor.undo();
        assert_eq!(
            app.circuit().component(rom).unwrap().memory.contents.get(0),
            0
        );
    }
}

#[test]
fn pla_table_apply_cancel_clear_history_and_document_replacement_in_both_languages() {
    use logisim_core::PlaInput;
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
                    delta: egui::vec2(0.0, -370.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: Modifiers::NONE,
                    source: egui::MouseWheelSource::Unknown,
                },
            ],
        );
        click(&ctx, &mut app, &t("PLA ROM"));
        assert_eq!(app.editor.tool, Tool::Place(Kind::PlaRom));
        app.editor.template.pla.resize(1, 1, 1);
        let labels = frame(&ctx, &mut app, vec![]);
        for label in [
            "Input bits",
            "AND terms",
            "Output bits",
            "Chip select active on",
        ] {
            assert!(has(&labels, &t(label)), "{label}");
        }
        assert!(!has(&labels, &t("Facing")));
        assert!(!has(&labels, &t("Data bits")));
        click(&ctx, &mut app, &t("Edit programming table…"));
        click(&ctx, &mut app, "—");
        click(&ctx, &mut app, "·");
        assert!(app.circuit().components.is_empty());
        assert_eq!(app.editor.template.pla.input(0, 0), PlaInput::Disconnected);
        click(&ctx, &mut app, &t("Apply"));
        assert_eq!(app.editor.template.pla.input(0, 0), PlaInput::Direct);
        assert!(app.editor.template.pla.output(0, 0));
        app.editor.place(Point::new(300, 200));
        app.editor.set_tool(Tool::Select);
        let id = app.circuit().components[0].id;
        app.editor.selection.insert(id);
        app.editor
            .circuit
            .add_wire(Point::new(100, 200), Point::new(300, 200));
        let original = app.circuit().clone();
        click(&ctx, &mut app, &t("Edit programming table…"));
        click(&ctx, &mut app, &t("Clear contents"));
        click(&ctx, &mut app, &t("Cancel"));
        assert_eq!(app.circuit(), &original);
        click(&ctx, &mut app, &t("Edit programming table…"));
        click(&ctx, &mut app, &t("Clear contents"));
        click(&ctx, &mut app, &t("Apply"));
        assert_eq!(
            app.circuit().component(id).unwrap().pla.input(0, 0),
            PlaInput::Disconnected
        );
        assert!(!app.circuit().component(id).unwrap().pla.output(0, 0));
        assert_eq!(app.circuit().wires, original.wires);
        app.editor.undo();
        assert_eq!(app.circuit(), &original);
        app.editor.redo();
        assert!(!app.circuit().component(id).unwrap().pla.output(0, 0));
        click(&ctx, &mut app, &t("Edit programming table…"));
        click(&ctx, &mut app, "—");
        app.replace_circuit(original.clone());
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(!has(&labels, &t("PLA programming table")));
        assert_eq!(app.circuit(), &original);
    }
}

#[test]
fn memory_import_button_history_template_and_stale_target_are_safe() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let t = |s: &str| i18n::with_language(language, || i18n::tr(s).to_owned());
        let mut circuit = logisim_core::Circuit::default();
        let mut component = Component::new(Kind::Rom, Point::new(200, 100));
        component.memory.contents.fill(0, 256, 0xaa);
        let rom = circuit.add_component(component);
        let ram = circuit.add_component(Component::new(Kind::Ram, Point::new(400, 100)));
        app.replace_circuit(circuit);
        app.editor.selection.insert(rom);
        click(&ctx, &mut app, &t("Edit contents…"));
        let before = app.project().clone();
        click(&ctx, &mut app, &t("Import .hex…"));
        assert_eq!(app.take_file_actions(), [crate::FileAction::ImportMemory]);
        let target = app.memory_import_target().unwrap();
        assert_eq!(
            app.project(),
            &before,
            "Opening the picker must not edit memory"
        );
        let mut contents = logisim_core::MemoryContents::default();
        contents.set(0, 0x1234);
        contents.set(1, 0xff);
        assert!(app.complete_memory_import(&target, contents.clone()));
        assert_eq!(
            app.circuit().component(rom).unwrap().memory.contents.get(0),
            0x34
        );
        assert_eq!(
            app.circuit().component(rom).unwrap().memory.contents.get(2),
            0
        );
        assert!(
            !app.complete_memory_import(&target, Default::default()),
            "Old requests cannot overwrite later edits"
        );
        click(&ctx, &mut app, &t("Close"));
        app.editor.undo();
        assert_eq!(app.project(), &before);
        app.editor.redo();
        assert_eq!(
            app.circuit().component(rom).unwrap().memory.contents.get(1),
            0xff
        );

        app.editor.selection.clear();
        app.editor.selection.insert(ram);
        click(&ctx, &mut app, &t("Edit contents…"));
        click(&ctx, &mut app, &t("Import .hex…"));
        assert_eq!(app.take_file_actions(), [crate::FileAction::ImportMemory]);
        let target = app.memory_import_target().unwrap();
        let before = app.project().clone();
        assert!(app.complete_memory_import(&target, contents.clone()));
        assert_eq!(app.project(), &before, "RAM image is runtime data");
        app.editor.path.push(999);
        assert!(!app.memory_import_is_current(&target));
        app.editor.path.clear();
        click(&ctx, &mut app, &t("Close"));
        assert!(!app.complete_memory_import(&target, contents.clone()));

        app.editor.set_tool(crate::editor::Tool::Place(Kind::Rom));
        app.editor.memory_target = Some((true, app.editor.template.id));
        let target = app.memory_import_target().unwrap();
        assert!(app.complete_memory_import(&target, contents));
        assert_eq!(app.project(), &before);
        assert_eq!(app.editor.template.memory.contents.get(0), 0x34);
        app.replace_project(before);
        assert!(!app.memory_import_is_current(&target));
    }
}
