//! Select once per electrical configuration; never fall back after executing a batch.
use logisim_sim::ProjectSimulator;

#[derive(Default, Debug, PartialEq, Eq)]
pub(super) enum Propagation {
    #[default]
    Unselected,
    Event,
    Compiled,
}
impl Propagation {
    pub(super) fn step(&mut self, sim: &mut ProjectSimulator) -> bool {
        if *self == Self::Unselected {
            *self = if sim.prepare_compiled().is_ok() {
                Self::Compiled
            } else {
                Self::Event
            };
        }
        match self {
            Self::Event => sim.step(),
            Self::Compiled => match sim.step_compiled() {
                Ok(_) => true,
                Err(_) => {
                    // Runtime plugin failures already expose their component error
                    // and E outputs. Keep the selected engine across pause/resume;
                    // reset, electrical edits or module replacement allow retry.
                    sim.set_enabled(false);
                    false
                }
            },
            Self::Unselected => unreachable!(),
        }
    }
}
