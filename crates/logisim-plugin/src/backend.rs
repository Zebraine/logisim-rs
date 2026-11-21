//! Runtime-selected Wasm handles. All handles of a VM originate from its module.
use super::{Result, error};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WasmEngine {
    #[default]
    Wasmi,
    Wasmtime,
}

impl WasmEngine {
    pub const fn available(self) -> bool {
        matches!(self, Self::Wasmi) || cfg!(feature = "wasmtime-abi4")
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Wasmi => "wasmi",
            Self::Wasmtime => "wasmtime",
        }
    }
}

pub(super) enum Engine {
    Wasmi(wasmi::Engine),
    #[cfg(feature = "wasmtime-abi4")]
    Wasmtime(wasmtime::Engine),
}

#[derive(Clone)]
pub(super) enum Module {
    Wasmi(wasmi::Module),
    #[cfg(feature = "wasmtime-abi4")]
    Wasmtime(wasmtime::Module),
}

impl Engine {
    pub(super) fn new(choice: WasmEngine) -> Result<Self> {
        match choice {
            WasmEngine::Wasmi => {
                let mut config = wasmi::Config::default();
                config.consume_fuel(true);
                Ok(Self::Wasmi(wasmi::Engine::new(&config)))
            }
            #[cfg(feature = "wasmtime-abi4")]
            WasmEngine::Wasmtime => wasmtime::Engine::new(&wasmtime_config())
                .map(Self::Wasmtime)
                .map_err(error),
            #[cfg(not(feature = "wasmtime-abi4"))]
            WasmEngine::Wasmtime => {
                Err("This build does not include the experimental engine.".into())
            }
        }
    }

    pub(super) fn compile(&self, bytes: &[u8]) -> Result<Module> {
        macro_rules! compile {
            ($runtime:ident, $engine:expr, $variant:ident) => {{
                let module = timed!("module_compile", $runtime::Module::new($engine, bytes))
                    .map_err(error)?;
                if module.imports().next().is_some() {
                    return Err("Plugin imports are not supported (including WASI).".into());
                }
                Ok(Module::$variant(module))
            }};
        }
        match self {
            Self::Wasmi(engine) => compile!(wasmi, engine, Wasmi),
            #[cfg(feature = "wasmtime-abi4")]
            Self::Wasmtime(engine) => {
                timed!(
                    "admission_validation",
                    wasmi::Module::validate(&wasmi::Engine::default(), bytes)
                )
                .map_err(error)?;
                compile!(wasmtime, engine, Wasmtime)
            }
        }
    }
}

#[cfg(feature = "wasmtime-abi4")]
pub(super) fn wasmtime_config() -> wasmtime::Config {
    let mut config = wasmtime::Config::new();
    config.consume_fuel(true);
    config
}

pub(super) enum Store {
    Wasmi(Box<wasmi::Store<wasmi::StoreLimits>>),
    #[cfg(feature = "wasmtime-abi4")]
    Wasmtime(wasmtime::Store<wasmtime::StoreLimits>),
}

impl From<wasmi::Store<wasmi::StoreLimits>> for Store {
    fn from(store: wasmi::Store<wasmi::StoreLimits>) -> Self {
        Self::Wasmi(Box::new(store))
    }
}
#[cfg(feature = "wasmtime-abi4")]
impl From<wasmtime::Store<wasmtime::StoreLimits>> for Store {
    fn from(store: wasmtime::Store<wasmtime::StoreLimits>) -> Self {
        Self::Wasmtime(store)
    }
}

impl Store {
    pub(super) fn set_fuel(&mut self, fuel: u64) -> Result<()> {
        match self {
            Self::Wasmi(store) => store.set_fuel(fuel).map_err(error),
            #[cfg(feature = "wasmtime-abi4")]
            Self::Wasmtime(store) => store.set_fuel(fuel).map_err(error),
        }
    }

    #[cfg(feature = "engine-study")]
    pub(super) fn get_fuel(&self) -> Result<u64> {
        match self {
            Self::Wasmi(store) => store.get_fuel().map_err(error),
            #[cfg(feature = "wasmtime-abi4")]
            Self::Wasmtime(store) => store.get_fuel().map_err(error),
        }
    }
}

pub(super) enum Instance {
    Wasmi(wasmi::Instance),
    #[cfg(feature = "wasmtime-abi4")]
    Wasmtime(wasmtime::Instance),
}

impl Module {
    pub(super) fn engine_choice(&self) -> WasmEngine {
        match self {
            Self::Wasmi(_) => WasmEngine::Wasmi,
            #[cfg(feature = "wasmtime-abi4")]
            Self::Wasmtime(_) => WasmEngine::Wasmtime,
        }
    }

    pub(super) fn instantiate(&self, budget: u64) -> Result<(Instance, Store)> {
        macro_rules! instantiate {
            ($runtime:ident, $module:expr, $variant:ident, $start:ident) => {{
                let module = $module;
                let limits = $runtime::StoreLimitsBuilder::new()
                    .memory_size(16 * 1024 * 1024)
                    .memories(1)
                    .tables(1)
                    .table_elements(10_000)
                    .instances(1)
                    .trap_on_grow_failure(true)
                    .build();
                let mut store = $runtime::Store::new(module.engine(), limits);
                store.limiter(|s| s);
                store.set_fuel(budget).map_err(error)?;
                let instance = $runtime::Linker::new(module.engine())
                    .$start(&mut store, module)
                    .map_err(error)?;
                Ok((Instance::$variant(instance), Store::from(store)))
            }};
        }
        match self {
            Self::Wasmi(module) => instantiate!(wasmi, module, Wasmi, instantiate_and_start),
            #[cfg(feature = "wasmtime-abi4")]
            Self::Wasmtime(module) => instantiate!(wasmtime, module, Wasmtime, instantiate),
        }
    }
}

// Handles stay private to the VM that created them. The only callers get the store from that VM.
macro_rules! dispatch {
    ($handle:expr, $store:expr, $borrow:ident, |$h:ident, $s:ident| $body:expr) => {
        match ($handle, $store) {
            (Self::Wasmi($h), Store::Wasmi(boxed)) => {
                let $s = boxed.$borrow();
                $body
            }
            #[cfg(feature = "wasmtime-abi4")]
            (Self::Wasmtime($h), Store::Wasmtime($s)) => $body,
            #[cfg(feature = "wasmtime-abi4")]
            _ => unreachable!("mixed internal VM handles"),
        }
    };
}

impl Instance {
    pub(super) fn has_export(&self, store: &mut Store, name: &str) -> bool {
        dispatch!(self, store, as_mut, |i, s| i.get_export(s, name).is_some())
    }

    pub(super) fn memory(&self, store: &mut Store, name: &str) -> Option<Memory> {
        match (self, store) {
            (Self::Wasmi(i), Store::Wasmi(s)) => i.get_memory(s.as_mut(), name).map(Memory::Wasmi),
            #[cfg(feature = "wasmtime-abi4")]
            (Self::Wasmtime(i), Store::Wasmtime(s)) => i.get_memory(s, name).map(Memory::Wasmtime),
            #[cfg(feature = "wasmtime-abi4")]
            _ => unreachable!("mixed internal VM handles"),
        }
    }

    pub(super) fn get_typed_func<P, R>(
        &self,
        store: &mut Store,
        name: &str,
    ) -> Result<TypedFunc<P, R>>
    where
        TypedFunc<P, R>: Export,
    {
        TypedFunc::load(self, store, name)
    }
}

pub(super) enum Memory {
    Wasmi(wasmi::Memory),
    #[cfg(feature = "wasmtime-abi4")]
    Wasmtime(wasmtime::Memory),
}
impl Memory {
    pub(super) fn data<'a>(&self, store: &'a Store) -> &'a [u8] {
        dispatch!(self, store, as_ref, |m, s| m.data(s))
    }
    pub(super) fn data_mut<'a>(&self, store: &'a mut Store) -> &'a mut [u8] {
        dispatch!(self, store, as_mut, |m, s| m.data_mut(s))
    }
    pub(super) fn read(&self, store: &Store, offset: usize, bytes: &mut [u8]) -> Result<()> {
        dispatch!(self, store, as_ref, |m, s| m
            .read(s, offset, bytes)
            .map_err(error))
    }
    pub(super) fn write(&self, store: &mut Store, offset: usize, bytes: &[u8]) -> Result<()> {
        dispatch!(self, store, as_mut, |m, s| m
            .write(s, offset, bytes)
            .map_err(error))
    }
}

pub(super) enum TypedFunc<P, R> {
    Wasmi(wasmi::TypedFunc<P, R>),
    #[cfg(feature = "wasmtime-abi4")]
    Wasmtime(wasmtime::TypedFunc<P, R>),
}

pub(super) trait Export: Sized {
    fn load(instance: &Instance, store: &mut Store, name: &str) -> Result<Self>;
}

macro_rules! signature {
    ($params:ty, $result:ty) => {
        impl Export for TypedFunc<$params, $result> {
            fn load(instance: &Instance, store: &mut Store, name: &str) -> Result<Self> {
                match (instance, store) {
                    (Instance::Wasmi(i), Store::Wasmi(s)) => i
                        .get_typed_func(s.as_mut(), name)
                        .map(Self::Wasmi)
                        .map_err(error),
                    #[cfg(feature = "wasmtime-abi4")]
                    (Instance::Wasmtime(i), Store::Wasmtime(s)) => {
                        i.get_typed_func(s, name).map(Self::Wasmtime).map_err(error)
                    }
                    #[cfg(feature = "wasmtime-abi4")]
                    _ => unreachable!("mixed internal VM handles"),
                }
            }
        }
        impl TypedFunc<$params, $result> {
            pub(super) fn call(&self, store: &mut Store, params: $params) -> Result<$result> {
                dispatch!(self, store, as_mut, |f, s| f.call(s, params).map_err(error))
            }
        }
    };
}
signature!((), u32);
signature!((), u64);
signature!(u32, u32);
signature!(u32, u64);
