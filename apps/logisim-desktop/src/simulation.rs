//! Desktop adapters: commands and presentation never own circuit semantics.
mod commands;
mod host;
#[cfg(test)]
mod tests;
mod values;
mod view;
pub(crate) mod worker;

pub(crate) use commands::apply_project;
pub(crate) use host::Host;
pub(crate) use view::present_project;
