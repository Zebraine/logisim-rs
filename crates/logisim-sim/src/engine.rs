//! Single-circuit engine responsibilities. Instance ownership stays at the crate boundary.
mod control;
mod schedule;
mod state;
#[cfg(test)]
mod tests;
mod view;
