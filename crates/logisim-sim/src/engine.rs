//! Single-circuit engine responsibilities. Instance ownership stays at the crate boundary.
mod control;
#[cfg(test)]
mod incremental_tests;
mod schedule;
mod state;
mod view;
