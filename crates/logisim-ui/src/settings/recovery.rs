//! File status supplied by the desktop host. Rendering never accesses the filesystem.
use crate::{i18n::tr, tf};
use egui::Ui;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SnapshotState {
    #[default]
    Empty,
    Available {
        source: Option<String>,
        saved_at: u64,
    },
    Damaged,
    Unavailable {
        snapshot_present: bool,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecoveryView {
    pub owned: bool,
    pub directory: Option<String>,
    pub state: SnapshotState,
    pub error: Option<String>,
}
impl RecoveryView {
    pub(crate) fn can_clear(&self) -> bool {
        match self.state {
            SnapshotState::Available { .. } => self.owned,
            SnapshotState::Damaged => true,
            SnapshotState::Unavailable { snapshot_present } => snapshot_present,
            SnapshotState::Empty => false,
        }
    }
    pub fn details(&self, ui: &mut Ui) {
        match &self.state {
            SnapshotState::Empty => {
                ui.label(tr("No recovery snapshot"));
            }
            SnapshotState::Available { source, saved_at } => {
                ui.label(source.as_deref().unwrap_or_else(|| tr("Untitled")));
                ui.weak(tf!("Saved at {}", utc_time(*saved_at)));
            }
            SnapshotState::Damaged => {
                ui.label(tr("Damaged recovery snapshot"));
            }
            SnapshotState::Unavailable { .. } => {
                ui.label(tr("Recovery storage unavailable"));
            }
        }
        if let Some(e) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, tr(e));
            if let Some(path) = &self.directory {
                ui.weak(path);
            }
        }
    }
}
// Split full Gregorian 400-year cycles before formatting a Unix timestamp in UTC.
fn utc_time(seconds: u64) -> String {
    let days = seconds / 86400;
    let mut year = 1970 + 400 * (days / 146097);
    let mut day = days % 146097;
    let leap = |y: u64| y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
    loop {
        let length = 365 + u64::from(leap(year));
        if day < length {
            break;
        }
        day -= length;
        year += 1;
    }
    let mut month = 1;
    for length in [
        31,
        28 + u64::from(leap(year)),
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ] {
        if day < length {
            break;
        }
        month += 1;
        day -= length;
    }
    format!(
        "{year:04}-{month:02}-{:02} {:02}:{:02} UTC",
        day + 1,
        seconds / 3600 % 24,
        seconds / 60 % 60
    )
}
#[cfg(test)]
mod tests;
