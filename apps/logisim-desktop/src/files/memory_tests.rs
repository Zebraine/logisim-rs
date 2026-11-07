use super::*;
use crate::file_tests::Temp;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape};
use logisim_core::{Circuit, Component, Kind, Point};

#[derive(Default)]
struct Harness {
    ctx: egui::Context,
    ws: Workspace,
    files: Files,
    sim: Simulator,
}
impl Harness {
    fn frame(&mut self, events: Vec<Event>) -> Vec<(String, Pos2)> {
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
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1200.0, 820.0))),
                events,
                ..Default::default()
            },
            |ui| {
                ui.add_enabled_ui(!self.files.busy(), |ui| self.ws.show(ui));
                for action in self.ws.take_file_actions() {
                    self.files
                        .request(action, &self.ctx, &mut self.ws, &mut self.sim);
                }
                self.files.show(&self.ctx, &mut self.ws, &mut self.sim);
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
    fn open_image(&mut self, path: &Path) {
        self.click("Import .hex…");
        assert!(matches!(
            self.files.dialog,
            Some(Dialog::MemoryPicker { .. })
        ));
        self.frame(vec![Event::Text(path.to_string_lossy().into_owned())]);
        self.frame(vec![Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
        self.frame(vec![]);
    }
}

#[test]
fn real_memory_picker_cancels_retries_and_preserves_document_file_and_ram() {
    let dir = Temp::new();
    let source = dir.path("source.circ");
    let image = dir.path("contents.HEX");
    let invalid = dir.path("bad.hex");
    std::fs::write(&image, "v2.0 raw\n12 AB 3*cd").unwrap();
    std::fs::write(&invalid, "v2.0 raw\n12 zz").unwrap();
    let mut circuit = Circuit::default();
    let mut c = Component::new(Kind::Rom, Point::new(200, 100));
    c.memory.address_bits = 4;
    c.memory.contents.fill(0, 16, 0xaa);
    let rom = circuit.add_component(c);
    let mut c = Component::new(Kind::Ram, Point::new(500, 100));
    c.memory.address_bits = 4;
    let ram = circuit.add_component(c);
    let mut h = Harness::default();
    logisim_ui::configure(&h.ctx);
    h.ws.replace_circuit(circuit);
    h.files
        .storage
        .save(h.ws.project(), &source, SaveMode::Current)
        .unwrap();
    h.files.directory = dir.0.clone();
    let original_disk = std::fs::read(&source).unwrap();
    let original = h.ws.project().clone();
    h.click("ROM 16 × 8");
    h.click("Edit contents…");
    h.click("Import .hex…");
    let labels = h.frame(vec![]);
    assert!(labels.iter().any(|(s, _)| s == "· contents.HEX"));
    assert!(!labels.iter().any(|(s, _)| s == "· source.circ"));
    h.click("Cancel");
    assert_eq!(h.ws.project(), &original);
    h.open_image(&invalid);
    assert!(matches!(h.files.dialog, Some(Dialog::Message { .. })));
    assert_eq!(h.ws.project(), &original);
    h.click("OK");
    h.open_image(&image);
    assert!(!h.files.busy());
    assert_eq!(
        h.ws.circuit()
            .component(rom)
            .unwrap()
            .memory
            .contents
            .get(1),
        0xab
    );
    assert_eq!(
        h.ws.circuit()
            .component(rom)
            .unwrap()
            .memory
            .contents
            .get(15),
        0
    );
    assert_eq!(std::fs::read(&source).unwrap(), original_disk);
    assert!(h.files.storage.dirty(h.ws.project()));
    h.click("Close");
    h.files
        .storage
        .save(h.ws.project(), &source, SaveMode::Current)
        .unwrap();
    let saved = h.ws.project().clone();
    h.click("RAM 16 × 8");
    h.click("Edit contents…");
    h.open_image(&image);
    assert_eq!(
        h.sim
            .view(&[])
            .unwrap()
            .memory_contents(ram)
            .unwrap()
            .get(1),
        0xab
    );
    assert_eq!(h.ws.project(), &saved);
    assert!(!h.files.storage.dirty(h.ws.project()));
    h.open_image(&invalid);
    assert!(matches!(h.files.dialog, Some(Dialog::Message { .. })));
    assert_eq!(
        h.sim
            .view(&[])
            .unwrap()
            .memory_contents(ram)
            .unwrap()
            .get(1),
        0xab
    );
    h.click("OK");
    let target = h.ws.memory_import_target().unwrap();
    h.click("Close");
    assert!(crate::memory_import::load(&image, &target, &mut h.ws, &mut h.sim).is_err());
    let (loaded, warnings) = Storage::new().load(&source).unwrap();
    assert!(warnings.is_empty());
    assert_eq!(
        loaded
            .main_circuit()
            .circuit
            .component(rom)
            .unwrap()
            .memory
            .contents
            .get(1),
        0xab
    );
    assert_eq!(
        loaded
            .main_circuit()
            .circuit
            .component(ram)
            .unwrap()
            .memory
            .contents,
        Default::default()
    );
    h.sim.reset();
    h.sim.advance(64);
    assert_eq!(
        h.sim
            .view(&[])
            .unwrap()
            .memory_contents(ram)
            .unwrap()
            .get(1),
        0
    );
}
