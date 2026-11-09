use crate::tests::support::widgets::*;
use crate::{editor::Tool, *};
use egui::{Context, Event, Modifiers, PointerButton, Pos2};
use logisim_core::{Component, Kind, Point};

#[test]
fn parity_properties_and_library_tools_work_in_both_languages() {
    use logisim_core::{Circuit, GateSize, Parity, ParityOutput};
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let translated = |s: &str| i18n::with_language(language, || i18n::tr(s).to_owned());
        let mut circuit = Circuit::default();
        let gate = circuit.add_component(Component::new(
            Kind::Parity(Parity::Odd),
            Point::new(300, 300),
        ));
        let port = circuit.component(gate).unwrap().ports()[1];
        circuit.add_wire(port, port + Point::new(-100, 0));
        circuit.normalize_wires();
        app.replace_circuit(circuit);
        app.editor.session.selection.insert(gate);
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        let wires = app.circuit().wires.clone();
        let labels = frame(&ctx, &mut app, vec![]);
        for name in [
            "Inputs",
            "Gate size",
            "Output value",
            "Invert input 1",
            "Invert input 2",
        ] {
            let expected = if let Some(n) = name.strip_prefix("Invert input ") {
                i18n::with_language(language, || tf!("Invert input {}", n))
            } else {
                translated(name)
            };
            assert!(has(&labels, &expected), "{language:?} {name}");
        }
        click(&ctx, &mut app, "0/1");
        click(&ctx, &mut app, &translated("0/floating"));
        assert_eq!(
            app.circuit().component(gate).unwrap().parity.output,
            ParityOutput::ZeroFloating
        );
        click(&ctx, &mut app, &translated("Narrow"));
        click(&ctx, &mut app, &translated("Wide"));
        assert_eq!(
            app.circuit().component(gate).unwrap().gate_size,
            GateSize::Wide
        );
        assert_eq!(app.circuit().wires, wires);
        let labels = frame(&ctx, &mut app, vec![]);
        let row = labels
            .iter()
            .find(|(s, _)| s == &i18n::with_language(language, || tf!("Invert input {}", 1)))
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
        assert!(app.circuit().component(gate).unwrap().parity_negated(0));
        assert_eq!(app.circuit().wires, wires);
        app.editor.undo();
        assert!(!app.circuit().component(gate).unwrap().parity_negated(0));
        app.editor.redo();
        assert!(app.circuit().component(gate).unwrap().parity_negated(0));
        click(&ctx, &mut app, &translated("Gates"));
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(Pos2::new(170.0, 360.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -160.0),
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        for parity in Parity::ALL {
            click(&ctx, &mut app, &translated(parity.name()));
            assert_eq!(app.editor.tool, Tool::Place(Kind::Parity(parity)));
        }
    }
}

#[test]
fn ttl_library_properties_power_internal_view_and_undo_in_both_languages() {
    fn toggle(ctx: &Context, app: &mut Workspace, text: &str) {
        frame(ctx, app, vec![]);
        let labels = frame(ctx, app, vec![]);
        let row = labels.iter().find(|(s, _)| s == text).unwrap().1;
        let checkbox = labels
            .iter()
            .find(|(s, r)| s.is_empty() && (r.center().y - row.center().y).abs() < 1.0)
            .unwrap()
            .1;
        let pos = Pos2::new(checkbox.left() - 10.0, row.center().y);
        for pressed in [true, false] {
            frame(
                ctx,
                app,
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
    }
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        click(&ctx, &mut app, "TTL");
        for chip in logisim_core::Ttl::ALL {
            if chip == logisim_core::Ttl::Ic747266 {
                frame(
                    &ctx,
                    &mut app,
                    vec![
                        Event::PointerMoved(Pos2::new(150.0, 350.0)),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0.0, -120.0),
                            phase: egui::TouchPhase::Move,
                            modifiers: Modifiers::NONE,
                            source: egui::MouseWheelSource::Unknown,
                        },
                    ],
                );
            }
            click(&ctx, &mut app, chip.name());
            assert_eq!(app.editor.tool, Tool::Place(Kind::Ttl(chip)));
            let labels = frame(&ctx, &mut app, vec![]);
            for key in [
                "Facing",
                "Include Vcc/Gnd pins",
                "Show internal structure",
                chip.description(),
            ] {
                assert!(has(&labels, &t(key)), "{key}");
            }
            for key in ["Data bits", "Inputs", "Gate size"] {
                assert!(!has(&labels, &t(key)));
            }
        }
        toggle(&ctx, &mut app, &t("Include Vcc/Gnd pins"));
        toggle(&ctx, &mut app, &t("Show internal structure"));
        assert!(app.editor.template.ttl.power_pins && app.editor.template.ttl.internal);
        app.editor.place(Point::new(300, 200));
        app.editor.set_tool(Tool::Select);
        let c = app.circuit().components[0].clone();
        app.editor.session.selection.insert(c.id);
        let p = c.ports()[0];
        app.editor
            .session
            .circuit
            .add_wire(p, p + Point::new(0, 100));
        let before = app.circuit().clone();
        toggle(&ctx, &mut app, &t("Include Vcc/Gnd pins"));
        assert_eq!(app.circuit().component(c.id).unwrap().ports().len(), 12);
        assert_eq!(app.circuit().wires, before.wires);
        app.editor.undo();
        assert_eq!(app.circuit(), &before);
        app.editor.redo();
        click(&ctx, &mut app, &t("East"));
        click(&ctx, &mut app, &t("North"));
        assert_eq!(
            app.circuit().component(c.id).unwrap().facing,
            logisim_core::Facing::North
        );
        assert_eq!(app.circuit().wires, before.wires);
        let edited = app.circuit().clone();
        app.set_language(if language == Language::English {
            Language::SimplifiedChinese
        } else {
            Language::English
        });
        frame(&ctx, &mut app, vec![]);
        assert_eq!(app.circuit(), &edited);
        let pin = i18n::with_language(language, || {
            crate::ttl::port_label(&c, c.ttl_index(3).unwrap())
        });
        assert!(pin.contains('3'));
    }
}

#[test]
fn ordinary_gate_xor_mode_and_input_inversion_edit_with_undo_in_both_languages() {
    for language in Language::ALL {
        let ctx = Context::default();
        configure(&ctx);
        let mut app = Workspace::default();
        app.set_language(language);
        let t = |s| i18n::with_language(language, || i18n::tr(s).to_owned());
        app.editor
            .set_tool(Tool::Place(Kind::Gate(logisim_core::Gate::Xor)));
        app.editor.place(Point::new(300, 300));
        let c = &app.circuit().components[0];
        let id = c.id;
        let port = c.ports()[1];
        app.editor
            .session
            .circuit
            .add_wire(port, port + Point::new(-40, 0));
        let geometry = |c: &logisim_core::Circuit| {
            c.wires
                .iter()
                .flat_map(|w| (w.a.x.min(w.b.x)..=w.a.x.max(w.b.x)).map(move |x| (x, w.a.y)))
                .collect::<std::collections::BTreeSet<_>>()
        };
        let wires = geometry(app.circuit());
        click(&ctx, &mut app, &t("Exactly one"));
        click(&ctx, &mut app, &t("Odd parity"));
        assert!(app.circuit().component(id).unwrap().gate_options.xor_odd);
        let labels = frame(&ctx, &mut app, vec![]);
        let row_label = i18n::with_language(language, || tf!("Invert input {}", 1));
        let row = labels.iter().find(|(s, _)| *s == row_label).unwrap().1;
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
        assert!(app.circuit().component(id).unwrap().gate_negated(0));
        assert_eq!(geometry(app.circuit()), wires);
        app.editor.undo();
        assert!(!app.circuit().component(id).unwrap().gate_negated(0));
        app.editor.redo();
        assert!(app.circuit().component(id).unwrap().gate_negated(0));
    }
}
