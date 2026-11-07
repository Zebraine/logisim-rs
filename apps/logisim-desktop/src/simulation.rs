//! Desktop adapters: commands and presentation never own circuit semantics.
mod commands;
#[cfg(test)]
mod tests;
mod values;
mod view;

pub(crate) use commands::apply_project;
pub(crate) use view::present_project;
