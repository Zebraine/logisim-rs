#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum PinPull {
    #[default]
    None,
    Up,
    Down,
}
impl PinPull {
    pub const ALL: [Self; 3] = [Self::None, Self::Up, Self::Down];
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "Unchanged",
            Self::Up => "Pull up",
            Self::Down => "Pull down",
        }
    }
}

/// Editable input-pin behavior. Interactive runtime values are not document state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct PinOptions {
    pub three_state: bool,
    pub pull: PinPull,
}
