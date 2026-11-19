//! Value inspection at the pointer. Prefer the nearest terminal over a wire below it.
use super::Editor;
use crate::{drawing, i18n::tr, plugins, tf};
use egui::{Response, Vec2};
use logisim_core::{Component, Kind, plugin::api::presentation::Library};

impl Editor {
    pub(super) fn signal_hover(
        &self,
        response: &Response,
        world: Vec2,
        tolerance: f32,
        libraries: &[Library],
    ) {
        let nearest = self
            .session
            .circuit
            .components
            .iter()
            .flat_map(|c| {
                c.ports().into_iter().enumerate().map(move |(index, p)| {
                    let distance = (drawing::pos(p).to_vec2() - world).length_sq();
                    (distance, c, index)
                })
            })
            .filter(|(d, _, _)| *d <= tolerance * tolerance)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, c, index)) = nearest {
            response.clone().on_hover_ui_at_pointer(|ui| {
                let name = port_name(c, index, libraries);
                if let Some(signal) = self.simulation.view.ports.get(&(c.id, index)) {
                    ui.label(name);
                    signal.pill(ui);
                } else if let Some(bits) = c.port_width(index) {
                    ui.label(tf!("{} · {} bit(s)", name, bits));
                } else {
                    ui.label(name);
                }
            });
        } else if let Some(id) = self.hit(world, tolerance)
            && let Some(signal) = self
                .simulation
                .view
                .wires
                .get(&id)
                .or_else(|| self.simulation.view.ports.get(&(id, 0)))
        {
            response
                .clone()
                .on_hover_ui_at_pointer(|ui| signal.pill(ui));
        }
    }
}

fn port_name(c: &Component, index: usize, libraries: &[Library]) -> String {
    match c.kind {
        Kind::Plugin => {
            let p = c.plugin.as_ref().expect("component with plugin ports");
            plugins::text(plugins::metadata(libraries, c), &p.tool.ports[index].label).to_owned()
        }
        Kind::Subcircuit(_) => {
            let p = &c.interface.ports[index];
            let name = if p.label.is_empty() {
                tf!("Pin #{}", p.pin)
            } else {
                p.label.clone()
            };
            format!(
                "{} · {}",
                name,
                tr(if p.output { "Output" } else { "Input" })
            )
        }
        Kind::Plexer(_) => crate::plexers::port_label(c, c.plexer_terminals()[index].role),
        Kind::Arithmetic(_) => crate::arithmetic::port_label(c, index),
        Kind::Parity(_) => crate::parity::port_label(c, index),
        Kind::Transistor | Kind::TransmissionGate => {
            tr(crate::transistors::port_label(c, index)).to_owned()
        }
        Kind::PullResistor | Kind::ControlledBuffer | Kind::ControlledInverter => {
            tr(crate::bus::port_label(c, index)).to_owned()
        }
        Kind::Latch(_) => tr(crate::latches::port_label(c, index)).to_owned(),
        Kind::PlaRom => tr(crate::pla::port_label(index)).to_owned(),
        Kind::Rom | Kind::Ram => tr(crate::memory::port_label(c, index)).to_owned(),
        Kind::Clock
        | Kind::Register
        | Kind::Counter
        | Kind::ShiftRegister
        | Kind::FlipFlop(_)
        | Kind::Random => tr(crate::sequential::port_label(c, index)).to_owned(),
        Kind::Button
        | Kind::Switch
        | Kind::DipSwitch
        | Kind::Slider
        | Kind::Joystick
        | Kind::Keyboard
        | Kind::Tty
        | Kind::Led
        | Kind::SevenSegment
        | Kind::HexDisplay
        | Kind::RgbLed
        | Kind::DotMatrix => tr(crate::io::port_label(c, index)).to_owned(),
        Kind::Gate(_)
        | Kind::Pin
        | Kind::Tunnel
        | Kind::Text
        | Kind::Constant
        | Kind::Power
        | Kind::Ground
        | Kind::Probe
        | Kind::Splitter
        | Kind::BitExtender => tr(c.kind.name()).to_owned(),
    }
}
