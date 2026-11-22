//! Derived geometry only: never cached signals, simulation state or document history.
use super::*;
mod connectivity;

#[derive(Default)]
pub(crate) struct CircuitCache {
    document: Option<Circuit>,
    ports: BTreeSet<Point>,
    pub markers: Vec<(Point, Option<Id>)>,
    selection: BTreeSet<Id>,
    pub selected_net: BTreeSet<Id>,
    visual_bounds: Vec<Option<Rect>>,
    scale: Option<(f32, f32)>,
    artwork_outline: bool,
    language: Option<crate::i18n::Language>,
    fonts: Option<egui::FontDefinitions>,
    text_options: Option<egui::epaint::text::TextOptions>,
    discovered_fonts: usize,
}

impl CircuitCache {
    pub fn synchronize(&mut self, circuit: &Circuit, selection: &BTreeSet<Id>) {
        let changed = self.document.as_ref() != Some(circuit);
        if changed {
            let ports = circuit
                .components
                .iter()
                .flat_map(Component::ports)
                .collect();
            if self.ports != ports
                || self
                    .document
                    .as_ref()
                    .is_none_or(|old| old.wires != circuit.wires)
            {
                let index = logisim_core::WireIndex::new(&circuit.wires);
                self.markers = circuit
                    .junctions()
                    .into_iter()
                    .chain(circuit.dangling_ends())
                    .map(|point| {
                        // Keep the previous first-in-document color at a junction.
                        let mut first = None;
                        index.visit(point, |i, _| {
                            first = Some(first.map_or(i, |j: usize| i.min(j)))
                        });
                        (point, first.map(|i| circuit.wires[i].id))
                    })
                    .collect();
                self.ports = ports;
            }
            // Comparing document values catches in-progress gestures and undo/redo as well
            // as project replacement, including reused IDs. No mutation hook can be missed.
            self.document = Some(circuit.clone());
            self.visual_bounds = vec![None; circuit.components.len()];
        }
        if changed || self.selection != *selection {
            self.selected_net = connectivity::selected_wires(circuit, selection);
            self.selection.clone_from(selection);
        }
    }

    pub fn prepare_view(&mut self, p: &Painter, canvas: &Canvas, artwork_outline: bool) {
        let scale = (canvas.camera.zoom, p.pixels_per_point());
        let fonts_changed = p.fonts(|f| {
            let changed = self.fonts.as_ref() != Some(f.definitions())
                || self.text_options.as_ref() != Some(f.options())
                || self.discovered_fonts != f.discovered_fonts().len();
            if changed {
                self.fonts = Some(f.definitions().clone());
                self.text_options = Some(*f.options());
                self.discovered_fonts = f.discovered_fonts().len();
            }
            changed
        });
        if self.scale != Some(scale)
            || self.language != Some(crate::i18n::current_language())
            || self.artwork_outline != artwork_outline
            || fonts_changed
        {
            self.visual_bounds.fill(None);
            self.scale = Some(scale);
            self.artwork_outline = artwork_outline;
            self.language = Some(crate::i18n::current_language());
        }
    }

    pub fn component(
        &mut self,
        index: usize,
        p: &Painter,
        canvas: &Canvas,
        c: &Component,
        simulation: &SimulationView,
        contrast: Option<theme::ArtworkContrast>,
    ) {
        let selected = self.selection.contains(&c.id);
        // Positive list: newly added kinds and all runtime-sized artwork must use the
        // normal renderer until their visual extent is known to be independent of state.
        let static_extent = matches!(
            c.kind,
            Kind::Tunnel
                | Kind::Text
                | Kind::Gate(_)
                | Kind::Parity(_)
                | Kind::ControlledBuffer
                | Kind::ControlledInverter
                | Kind::PullResistor
                | Kind::Transistor
                | Kind::TransmissionGate
                | Kind::Constant
                | Kind::Power
                | Kind::Ground
                | Kind::Splitter
                | Kind::BitExtender
                | Kind::Plexer(_)
                | Kind::Arithmetic(_)
                | Kind::PlaRom
                | Kind::Subcircuit(_)
        );
        if !static_extent || !p.is_visible() || p.opacity() < 1.0 {
            super::component(p, canvas, c, selected, false, Some(simulation), contrast);
            return;
        }
        if let Some(bounds) = self.visual_bounds[index] {
            let screen = Rect::from_min_max(
                canvas.screen_vec(bounds.min.to_vec2()),
                canvas.screen_vec(bounds.max.to_vec2()),
            );
            if !screen
                .expand(1.0_f32.max(p.pixels_per_point().recip()))
                .intersects(p.clip_rect())
            {
                return;
            }
            super::component(p, canvas, c, selected, false, Some(simulation), contrast);
            return;
        }
        let start = p
            .ctx()
            .graphics(|g| g.get(p.layer_id()).map_or(0, |l| l.next_idx().0));
        super::component(p, canvas, c, selected, false, Some(simulation), contrast);
        // Measure the actual unclipped shapes once, including rotated/multiline text,
        // artwork outside hit bounds and port dots. Keep no galleys/atlas UVs across frames.
        let painted = p.ctx().graphics(|g| {
            g.get(p.layer_id()).map_or(Rect::NOTHING, |l| {
                l.all_entries().skip(start).fold(Rect::NOTHING, |r, s| {
                    r.union(s.shape.visual_bounding_rect())
                })
            })
        });
        let body = super::bounds(c);
        let selection = Rect::from_min_max(
            canvas.screen_vec(body.min.to_vec2()),
            canvas.screen_vec(body.max.to_vec2()),
        )
        .expand(9.0);
        let all = painted.union(selection);
        if all.is_finite() {
            self.visual_bounds[index] = Some(Rect::from_min_max(
                canvas.world(all.min).to_pos2(),
                canvas.world(all.max).to_pos2(),
            ));
        }
    }
}

#[cfg(test)]
mod tests;
