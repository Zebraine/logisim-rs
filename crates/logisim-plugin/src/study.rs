//! Isolated benchmark controls. Each open still constructs fresh stores and validates metadata.
use super::*;
use std::cell::Cell;

thread_local! { static BUDGET: Cell<u64> = const { Cell::new(FUEL) }; }

pub(super) fn budget() -> u64 {
    BUDGET.get()
}

pub fn with_multiplier<T>(multiple: u64, run: impl FnOnce() -> T) -> Result<T> {
    if !matches!(multiple, 1 | 2 | 4 | 8) {
        return Err("Expected fuel multiplier 1, 2, 4 or 8".into());
    }
    struct Restore(u64);
    impl Drop for Restore {
        fn drop(&mut self) {
            BUDGET.set(self.0);
        }
    }
    let _restore = Restore(BUDGET.replace(FUEL * multiple));
    Ok(run())
}

// Buffer allocation and dispatch share one refill. Sample before propagating a dispatch trap.
pub(super) fn record_remaining(store: &Store<StoreLimits>) -> Result<()> {
    let left = store.get_fuel().map_err(error)?;
    metrics::on_call(|calls| calls.remaining_fuel.push(left));
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Reuse {
    None,
    Engine,
    Module,
}

pub struct Loader {
    reuse: Reuse,
    engine: Option<Engine>,
    // One bounded entry. Exact source bytes avoid hash collisions. The engine, build and
    // target are fixed for the lifetime of the entry. No native bytes cross a trust boundary.
    entry: Option<(Vec<u8>, Module)>,
    pub hits: usize,
    pub misses: usize,
}
impl Loader {
    pub fn new(reuse: Reuse) -> Self {
        Self {
            reuse,
            engine: None,
            entry: None,
            hits: 0,
            misses: 0,
        }
    }

    pub fn load(&mut self, bytes: &[u8]) -> Result<Plugin> {
        check_binary(bytes)?;
        if let Some((key, module)) = &self.entry
            && key == bytes
        {
            self.hits += 1;
            return Plugin::from_module(module.clone());
        }
        self.misses += 1;
        if self.reuse == Reuse::None {
            return Plugin::load(bytes);
        }
        if self.engine.is_none() {
            self.engine = Some(new_engine()?);
        }
        let module = compile(self.engine.as_ref().expect("initialized engine"), bytes)?;
        let plugin = Plugin::from_module(module.clone())?;
        if self.reuse == Reuse::Module {
            self.entry = Some((bytes.to_vec(), module));
        }
        Ok(plugin)
    }
}
