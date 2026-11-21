mod backup;
mod behavior;
mod image;
mod memory;
mod plugin;
mod recovery;
mod settings;

use super::*;
use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape};
use logisim_sim::ProjectSimulator as Simulator;

struct Harness {
    config_dir: crate::tests::support::Temp,
    preferences: crate::preferences::Preferences,
    ctx: egui::Context,
    ws: Workspace,
    files: Files,
    sim: Simulator,
}
impl Default for Harness {
    fn default() -> Self {
        let config = crate::tests::support::Temp::new();
        let mut ws = Workspace::default();
        let preferences =
            crate::preferences::Preferences::at(Some(config.path("preferences.json")), &mut ws);
        Self {
            config_dir: config,
            preferences,
            ws,
            ctx: Default::default(),
            files: Default::default(),
            sim: Default::default(),
        }
    }
}
impl Harness {
    fn frame(&mut self, events: Vec<Event>) -> Vec<(String, Pos2)> {
        self.frame_at(events, egui::vec2(1200., 820.))
    }
    fn frame_at(&mut self, events: Vec<Event>, size: egui::Vec2) -> Vec<(String, Pos2)> {
        self.sim
            .synchronize(self.ws.project(), self.ws.simulation_root());
        self.sim.advance(64);
        self.ws.set_simulation(crate::simulation::present_project(
            self.ws.circuit(),
            &self.sim,
            self.ws.simulation_path(),
        ));
        let mut out = self.ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                events,
                ..Default::default()
            },
            |ui| {
                ui.add_enabled_ui(!self.files.busy(), |ui| self.ws.show(ui));
                for action in self.ws.take_file_actions() {
                    self.files
                        .request(action, &self.ctx, &mut self.ws, &mut self.sim);
                }
                logisim_ui::i18n::with_language(self.ws.language(), || {
                    self.files.show(
                        &self.ctx,
                        &mut self.ws,
                        &mut self.sim,
                        &mut self.preferences,
                    )
                });
            },
        );
        fn collect(shape: Shape, text: &mut Vec<(String, Pos2)>) {
            match shape {
                Shape::Text(t) => {
                    text.push((t.galley.job.text.clone(), t.pos + t.galley.size() / 2.0))
                }
                Shape::Vec(v) => v.into_iter().for_each(|s| collect(s, text)),
                _ => (),
            }
        }
        out.textures_delta.clear();
        let mut text = vec![];
        for shape in out.shapes {
            collect(shape.shape, &mut text);
        }
        text
    }
    fn click(&mut self, label: &str) {
        for _ in 0..2 {
            self.frame(vec![]);
        }
        let labels = self.frame(vec![]);
        let pos = labels
            .iter()
            .find(|(s, _)| s == label)
            .unwrap_or_else(|| panic!("Missing {label}: {labels:?}"))
            .1;
        self.click_at(pos);
    }
    fn click_at(&mut self, pos: Pos2) {
        for pressed in [true, false] {
            self.frame(vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ]);
        }
    }
}

fn click_in(h: &mut Harness, size: egui::Vec2, label: &str) {
    let labels = h.frame_at(vec![], size);
    let pos = labels
        .iter()
        .rev()
        .find(|(s, _)| s == label)
        .unwrap_or_else(|| panic!("missing {label}"))
        .1;
    for pressed in [true, false] {
        h.frame_at(
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    pressed,
                    button: PointerButton::Primary,
                    modifiers: Modifiers::NONE,
                },
            ],
            size,
        );
    }
}

#[test]
fn file_picker_scrolls_independently_of_its_path_and_footer() {
    use logisim_ui::{Density, Language, Settings, i18n::with_language};
    let dir = crate::tests::support::Temp::new();
    for i in 0..40 {
        std::fs::write(dir.path(&format!("circuit-{i:02}.circ")), "").unwrap();
    }
    for language in Language::ALL {
        for density in [Density::Comfortable, Density::Compact] {
            for size in [egui::vec2(720., 480.), egui::vec2(1200., 820.)] {
                let mut h = Harness::default();
                h.ws.apply_settings(Settings {
                    language,
                    density,
                    ..Default::default()
                });
                h.files.directory.clone_from(&dir.0);
                h.files.picker(Mode::Open);
                for _ in 0..3 {
                    h.frame_at(vec![], size);
                }
                let labels = h.frame_at(vec![], size);
                let bounds = h
                    .ctx
                    .memory(|m| m.area_rect(egui::Id::unique("open-circuit")))
                    .unwrap();
                assert!(
                    bounds.width() <= size.x * 0.9 + 1. && bounds.height() <= size.y * 0.9 + 1.
                );
                let at = |labels: &[(String, Pos2)], key: &str| {
                    labels.iter().rfind(|(s, _)| s == key).unwrap().1
                };
                let first = at(&labels, "circuit-00.circ");
                let footer = with_language(language, || tr("Cancel"));
                let cancel = at(&labels, footer);
                assert!(bounds.contains(cancel) && cancel.y > first.y);
                for _ in 0..5 {
                    h.frame_at(
                        vec![
                            Event::PointerMoved(first),
                            Event::MouseWheel {
                                unit: egui::MouseWheelUnit::Point,
                                delta: egui::vec2(0., -550.),
                                modifiers: Modifiers::NONE,
                                phase: egui::TouchPhase::Move,
                                source: egui::MouseWheelSource::Unknown,
                            },
                        ],
                        size,
                    );
                }
                let labels = h.frame_at(vec![], size);
                assert!(at(&labels, "circuit-39.circ").y < cancel.y);
                assert_eq!(at(&labels, footer), cancel);
            }
        }
    }
}

#[test]
fn save_picker_filters_by_selected_type_and_rejects_a_conflicting_extension() {
    use logisim_ui::{Language, Settings, i18n::with_language};
    let dir = crate::tests::support::Temp::new();
    std::fs::write(dir.path("shown.LRS"), []).unwrap();
    std::fs::write(dir.path("other.circ"), b"original").unwrap();
    for language in Language::ALL {
        for size in [egui::vec2(720., 480.), egui::vec2(1200., 820.)] {
            let mut h = Harness::default();
            h.files.directory.clone_from(&dir.0);
            h.ws.apply_settings(Settings {
                language,
                ..Default::default()
            });
            h.files
                .request(FileAction::SaveAs, &h.ctx, &mut h.ws, &mut h.sim);
            let t = |key| with_language(language, || tr(key));
            for _ in 0..3 {
                h.frame_at(vec![], size);
            }
            let labels = h.frame_at(vec![], size);
            for key in [
                t("Logisim-rs Project (*.lrs)"),
                "Untitled.lrs",
                "shown.LRS",
                t("Save"),
                t("Cancel"),
            ] {
                assert!(
                    labels
                        .iter()
                        .any(|(s, pos)| s == key
                            && Rect::from_min_size(Pos2::ZERO, size).contains(*pos)),
                    "{key}: {labels:?}"
                );
            }
            assert!(!labels.iter().any(|(s, _)| s == "other.circ"));
            h.frame_at(
                vec![
                    Event::Key {
                        key: egui::Key::A,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::COMMAND,
                    },
                    Event::Text("other.circ".into()),
                ],
                size,
            );
            click_in(&mut h, size, t("Save"));
            assert!(
                h.frame_at(vec![], size)
                    .iter()
                    .any(|(s, _)| s == t("Choose a project file with the indicated extension."))
            );
            click_in(&mut h, size, t("Logisim-rs Project (*.lrs)"));
            click_in(&mut h, size, t("Legacy Logisim (*.circ)"));
            let labels = h.frame_at(vec![], size);
            assert!(labels.iter().any(|(s,_)| s == t("For older tools. Saving stops and lists any data the legacy format cannot preserve.")));
            assert!(!labels.iter().any(|(s, _)| s == "shown.LRS"));
            click_in(&mut h, size, t("Cancel"));
            assert!(h.files.storage.path.is_none() && h.ws.recent().is_empty());
            assert_eq!(std::fs::read(dir.path("other.circ")).unwrap(), b"original");
        }
    }
}
