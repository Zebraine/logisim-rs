use crate::tests::support::widgets::*;
use crate::{editor::Tool, *};
use egui::{Context, Event, Modifiers, PointerButton, Pos2};
use logisim_core::{Component, Kind, Point};

#[test]
fn bus_control_properties_use_translated_widgets_and_keep_wires_with_undo() {
    use logisim_core::{Circuit, GateSize, Pull};
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |s: &str| i18n::with_language(language, || i18n::tr(s).to_owned());
        let mut circuit = Circuit::default();
        let gate = circuit.add_component(Component::new(
            Kind::ControlledInverter,
            Point::new(300, 300),
        ));
        let resistor =
            circuit.add_component(Component::new(Kind::PullResistor, Point::new(500, 300)));
        let p = circuit.component(gate).unwrap().ports()[2];
        circuit.add_wire(p, p + Point::new(0, 80));
        app.replace_circuit(circuit);
        app.editor.session.selection.insert(gate);
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        let wires = app.circuit().wires.clone();
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, &translated("Control location")));
        assert!(has(&labels, &translated("Negate control input")));
        click(&ctx, &mut app, &translated("Right-handed"));
        click(&ctx, &mut app, &translated("Left-handed"));
        assert!(app.circuit().component(gate).unwrap().bus.control_left);
        assert_eq!(app.circuit().wires, wires);
        click(&ctx, &mut app, &translated("Narrow"));
        click(&ctx, &mut app, &translated("Wide"));
        assert_eq!(
            app.circuit().component(gate).unwrap().bus.size,
            GateSize::Wide
        );
        assert_eq!(app.circuit().wires, wires);
        app.editor.undo();
        assert_eq!(
            app.circuit().component(gate).unwrap().bus.size,
            GateSize::Narrow
        );
        app.editor.redo();
        let labels = frame(&ctx, &mut app, vec![]);
        let row = labels
            .iter()
            .find(|(s, _)| s == &translated("Negate control input"))
            .unwrap()
            .1;
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
        assert!(app.circuit().component(gate).unwrap().bus.negate_control);
        app.editor.undo();
        assert!(!app.circuit().component(gate).unwrap().bus.negate_control);
        app.editor.session.selection.clear();
        app.editor.session.selection.insert(resistor);
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, &translated("Pull direction")));
        assert!(!has(&labels, &translated("Data bits")));
        assert!(!has(&labels, &translated("Label")));
        click(&ctx, &mut app, &translated("Zero"));
        click(&ctx, &mut app, &translated("One"));
        assert_eq!(
            app.circuit().component(resistor).unwrap().bus.pull,
            Pull::One
        );
        click(&ctx, &mut app, &translated("One"));
        click(&ctx, &mut app, &translated("Error"));
        assert_eq!(
            app.circuit().component(resistor).unwrap().bus.pull,
            Pull::Error
        );
        app.editor.undo();
        assert_eq!(
            app.circuit().component(resistor).unwrap().bus.pull,
            Pull::One
        );
        // Library entries are real placement tools in either language.
        click(&ctx, &mut app, &translated("Gates"));
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
        click(&ctx, &mut app, &translated("Controlled Buffer"));
        assert_eq!(app.editor.tool, Tool::Place(Kind::ControlledBuffer));
    }
}

#[test]
fn transistor_properties_and_library_tools_work_in_both_languages() {
    use logisim_core::{Circuit, TransistorType};
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |s: &str| i18n::with_language(language, || i18n::tr(s).to_owned());
        let mut circuit = Circuit::default();
        let transistor =
            circuit.add_component(Component::new(Kind::Transistor, Point::new(300, 300)));
        let transmission =
            circuit.add_component(Component::new(Kind::TransmissionGate, Point::new(500, 300)));
        let port = circuit.component(transistor).unwrap().ports()[2];
        circuit.add_wire(port, port + Point::new(0, -80));
        circuit.normalize_wires();
        app.replace_circuit(circuit);
        app.editor.session.selection.insert(transistor);
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        let wires = app.circuit().wires.clone();
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(has(&labels, &translated("Transistor type")));
        assert!(has(&labels, &translated("Gate location")));
        assert!(has(&labels, &translated("Data bits")));
        assert!(!has(&labels, &translated("Label")));
        click(&ctx, &mut app, &translated("P-Type"));
        click(&ctx, &mut app, &translated("N-Type"));
        assert_eq!(
            app.circuit()
                .component(transistor)
                .unwrap()
                .transistor
                .transistor_type,
            TransistorType::N
        );
        click(&ctx, &mut app, &translated("Top/Left"));
        click(&ctx, &mut app, &translated("Bottom/Right"));
        assert!(
            app.circuit()
                .component(transistor)
                .unwrap()
                .transistor
                .gate_bottom_right
        );
        assert_eq!(app.circuit().wires, wires);
        app.editor.undo();
        assert!(
            !app.circuit()
                .component(transistor)
                .unwrap()
                .transistor
                .gate_bottom_right
        );
        app.editor.redo();
        app.editor.session.selection.clear();
        app.editor.session.selection.insert(transmission);
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        let labels = frame(&ctx, &mut app, vec![]);
        assert!(!has(&labels, &translated("Transistor type")));
        assert!(has(&labels, &translated("Gate location")));
        click(&ctx, &mut app, &translated("Top/Left"));
        click(&ctx, &mut app, &translated("Bottom/Right"));
        assert!(
            app.circuit()
                .component(transmission)
                .unwrap()
                .transistor
                .gate_bottom_right
        );
        assert_eq!(app.circuit().wires, wires);
        click(&ctx, &mut app, &translated("Wiring"));
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(Pos2::new(170.0, 360.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -180.0),
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        click(&ctx, &mut app, &translated("Transistor"));
        assert_eq!(app.editor.tool, Tool::Place(Kind::Transistor));
        click(&ctx, &mut app, &translated("Transmission Gate"));
        assert_eq!(app.editor.tool, Tool::Place(Kind::TransmissionGate));
    }
}

#[test]
fn pin_pull_label_and_tristate_properties_edit_in_both_languages() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        app.editor.set_tool(Tool::Place(Kind::Pin));
        app.editor.place(Point::new(300, 300));
        let id = app.circuit().components[0].id;
        app.editor
            .session
            .circuit
            .add_wire(Point::new(300, 300), Point::new(400, 300));
        let wires = app.circuit().wires.clone();
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        let labels = frame(&ctx, &mut app, vec![]);
        for name in ["Label location", "Three-state input", "Pull behavior"] {
            assert!(has(&labels, &t(name)));
        }
        let row = labels
            .iter()
            .find(|(s, _)| *s == t("Three-state input"))
            .unwrap()
            .1;
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
        assert!(app.circuit().component(id).unwrap().pin.three_state);
        click(&ctx, &mut app, &t("Unchanged"));
        click(&ctx, &mut app, &t("Pull down"));
        assert_eq!(
            app.circuit().component(id).unwrap().pin.pull,
            logisim_core::PinPull::Down
        );
        assert_eq!(app.circuit().wires, wires);
        app.editor.undo();
        assert_eq!(
            app.circuit().component(id).unwrap().pin.pull,
            logisim_core::PinPull::None
        );
        app.editor.redo();
        assert_eq!(
            app.circuit().component(id).unwrap().pin.pull,
            logisim_core::PinPull::Down
        );
    }
}
