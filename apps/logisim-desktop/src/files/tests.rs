mod backup;
mod behavior;
mod memory;
mod plugin;
mod recovery;

use super::*;
use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape};
use logisim_sim::ProjectSimulator as Simulator;

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
