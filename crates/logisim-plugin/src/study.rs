//! Isolated benchmark controls. Each open still constructs fresh stores and validates metadata.
use super::*;
use std::cell::Cell;
#[cfg(feature = "compare-wasmtime")]
mod disk;

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

#[derive(PartialEq, Eq)]
pub enum Reuse {
    None,
    Engine,
    Module,
    #[cfg(feature = "compare-wasmtime")]
    Disk(std::path::PathBuf),
}

pub struct Loader {
    reuse: Reuse,
    engine: Option<Engine>,
    // One bounded entry. Exact source bytes avoid hash collisions. The engine, build and
    // target are fixed for the lifetime of the entry. No native bytes cross a trust boundary.
    entry: Option<(Vec<u8>, Module)>,
    #[cfg(feature = "compare-wasmtime")]
    disk: Option<backend::Cache>,
    #[cfg(feature = "compare-wasmtime")]
    disk_error: Option<String>,
    pub hits: usize,
    pub misses: usize,
}
impl Loader {
    pub fn new(reuse: Reuse) -> Self {
        Self {
            reuse,
            engine: None,
            entry: None,
            #[cfg(feature = "compare-wasmtime")]
            disk: None,
            #[cfg(feature = "compare-wasmtime")]
            disk_error: None,
            hits: 0,
            misses: 0,
        }
    }

    #[cfg(feature = "compare-wasmtime")]
    pub fn cache_error(&self) -> Option<&str> {
        self.disk_error.as_deref()
    }

    pub fn load(&mut self, bytes: &[u8]) -> Result<Plugin> {
        check_binary(bytes)?;
        if let Some((key, module)) = &self.entry
            && key == bytes
        {
            self.hits += 1;
            return Plugin::from_module(module.clone());
        }
        #[cfg(feature = "compare-wasmtime")]
        if let Reuse::Disk(path) = &self.reuse {
            if self.disk.is_none() && self.disk_error.is_none() {
                match disk::open(path) {
                    Ok(cache) => self.disk = Some(cache),
                    Err(e) => self.disk_error = Some(e),
                }
            }
            let mut settings = config();
            settings.cache(self.disk.clone());
            let engine = Engine::new(&settings).map_err(error)?;
            let hits = self.disk.as_ref().map_or(0, |c| c.cache_hits());
            let module = compile(&engine, bytes)?;
            if self.disk.as_ref().is_some_and(|c| c.cache_hits() > hits) {
                self.hits += 1;
            } else {
                self.misses += 1;
            }
            return Plugin::from_module(module);
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
