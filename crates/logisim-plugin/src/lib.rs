//! Sandboxed module loading. No filesystem access or implicit plugin discovery.
#[cfg(feature = "plugin-metrics")]
pub mod metrics;
// With metrics disabled the expansion contains only the original expression.
macro_rules! timed {
    ($name:literal, $value:expr) => {{
        #[cfg(feature = "plugin-metrics")]
        let _span = crate::metrics::timer($name);
        $value
    }};
}
mod frame;
mod interaction;
mod metadata;
mod presentation;
mod raster;
mod reconfiguration;
mod stable;
#[cfg(feature = "engine-study")]
pub mod study;
#[cfg(test)]
mod tests;
use backend::{
    AsContextMut, Config, Engine, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder,
    TypedFunc,
};
use logisim_plugin_api::{
    Component, Evaluation, MAX_MESSAGE, Manifest, Request, Response, Update, supports_abi,
};
pub use stable::StableInstance;
use std::{collections::BTreeMap, sync::Arc};
#[cfg(not(feature = "compare-wasmtime"))]
use wasmi as backend;
#[cfg(feature = "compare-wasmtime")]
use wasmtime as backend;
pub const MAX_MODULE: usize = 4 * 1024 * 1024;
const FUEL: u64 = 10_000_000;
type Result<T> = std::result::Result<T, String>;
fn error(e: impl std::fmt::Display) -> String {
    #[cfg(feature = "compare-wasmtime")]
    {
        format!("{e:#}")
    } // Wasmtime attaches the trap cause below a Wasm backtrace.
    #[cfg(not(feature = "compare-wasmtime"))]
    {
        e.to_string()
    }
}

fn check_binary(bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_MODULE || !bytes.starts_with(b"\0asm") {
        return Err("Choose a WebAssembly binary within the 4 MiB limit.".into());
    }
    Ok(())
}

fn config() -> Config {
    let mut config = Config::default();
    config.consume_fuel(true);
    config
}

fn new_engine() -> Result<Engine> {
    #[cfg(not(feature = "compare-wasmtime"))]
    {
        Ok(Engine::new(&config()))
    }
    #[cfg(feature = "compare-wasmtime")]
    {
        Engine::new(&config()).map_err(error)
    }
}

fn compile(engine: &Engine, bytes: &[u8]) -> Result<Module> {
    #[cfg(feature = "compare-wasmtime")]
    timed!(
        "admission_validation",
        wasmi::Module::validate(&wasmi::Engine::default(), bytes)
    )
    .map_err(error)?;
    let module = timed!("module_compile", Module::new(engine, bytes)).map_err(error)?;
    if module.imports().next().is_some() {
        return Err("Plugin imports are not supported (including WASI).".into());
    }
    Ok(module)
}

#[inline]
fn fuel_budget() -> u64 {
    #[cfg(feature = "engine-study")]
    {
        study::budget()
    }
    #[cfg(not(feature = "engine-study"))]
    {
        FUEL
    }
}

#[derive(Clone)]
pub struct Plugin {
    module: Module,
    manifest: Manifest,
    presentation: logisim_plugin_api::presentation::Presentation,
    reconfiguration: logisim_plugin_api::reconfiguration::Policy,
    interaction: logisim_plugin_api::interaction::Interaction,
}
impl Plugin {
    pub fn load(bytes: &[u8]) -> Result<Self> {
        check_binary(bytes)?;
        Self::from_module(compile(&new_engine()?, bytes)?)
    }

    fn from_module(module: Module) -> Result<Self> {
        let mut instance = Instance::new(&module)?;
        let Response::Manifest(manifest) = instance.call(&Request::Describe)? else {
            return Err("Plugin did not return a manifest.".into());
        };
        manifest.validate()?;
        if manifest.abi != instance.abi {
            return Err("Plugin manifest and exported ABI differ.".into());
        }
        let presentation = instance.presentation()?;
        presentation.validate(&manifest)?;
        let reconfiguration: logisim_plugin_api::reconfiguration::Policy = metadata::read(
            &mut instance.store,
            &instance.memory,
            instance.reconfiguration.as_ref(),
        )?;
        reconfiguration.validate(&manifest)?;
        let interaction: logisim_plugin_api::interaction::Interaction = metadata::read(
            &mut instance.store,
            &instance.memory,
            instance.interaction.as_ref(),
        )?;
        interaction.validate(&manifest)?;
        Ok(Self {
            module,
            manifest,
            presentation,
            reconfiguration,
            interaction,
        })
    }
    /// Load only the module with the identity and tool definitions saved in this project.
    pub fn load_dependency<'a>(
        bytes: &[u8],
        id: &str,
        version: &str,
        components: impl IntoIterator<Item = &'a Component>,
    ) -> Result<Self> {
        #[cfg(feature = "plugin-metrics")]
        let _span = metrics::timer("load_dependency");
        let plugin = Self::load(bytes)?;
        plugin.check_dependency(id, version, components)?;
        Ok(plugin)
    }

    /// Validate the saved identity and each saved tool, including on a module-cache hit.
    pub fn check_dependency<'a>(
        &self,
        id: &str,
        version: &str,
        components: impl IntoIterator<Item = &'a Component>,
    ) -> Result<()> {
        if self.manifest.id != id || self.manifest.version != version {
            return Err("Plugin identity or version does not match the circuit.".into());
        }
        for component in components {
            self.check_component(component)?;
        }
        Ok(())
    }
    pub fn library(&self) -> logisim_plugin_api::presentation::Library {
        logisim_plugin_api::presentation::Library {
            manifest: self.manifest.clone(),
            presentation: self.presentation.clone(),
            interaction: self.interaction.clone(),
        }
    }
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn instantiate(&self, component: &Component) -> Result<Instance> {
        #[cfg(feature = "plugin-metrics")]
        let _span = metrics::timer("instantiate");
        self.check_component(component)?;
        let mut instance = Instance::new(&self.module)?;
        instance.bind_raster(component)?;
        Ok(instance)
    }
    pub fn check_component(&self, component: &Component) -> Result<()> {
        component.validate()?;
        if component.plugin != self.manifest.id
            || component.version != self.manifest.version
            || !self.manifest.tools.contains(&component.tool)
        {
            return Err("Plugin version or definition differs from the saved component.".into());
        }
        Ok(())
    }
}
pub struct Instance {
    ui: Option<TypedFunc<(), u64>>,
    interaction: Option<TypedFunc<(), u64>>,
    reconfiguration: Option<TypedFunc<(), u64>>,
    frame: Option<frame::FrameApi>,
    raster: Option<raster::RasterApi>,
    abi: u32,
    store: Store<StoreLimits>,
    memory: Memory,
    buffer: TypedFunc<u32, u32>,
    call: TypedFunc<u32, u64>,
}
impl Instance {
    fn new(module: &Module) -> Result<Self> {
        #[cfg(feature = "plugin-metrics")]
        let _span = metrics::timer("vm_new");
        let limits = StoreLimitsBuilder::new()
            .memory_size(16 * 1024 * 1024)
            .memories(1)
            .tables(1)
            .table_elements(10_000)
            .instances(1)
            .trap_on_grow_failure(true)
            .build();
        let mut store = Store::new(module.engine(), limits);
        store.limiter(|s| s);
        store.set_fuel(fuel_budget()).map_err(error)?;
        #[cfg(feature = "compare-wasmtime")]
        let instance = Linker::<StoreLimits>::new(module.engine())
            .instantiate(&mut store, module)
            .map_err(error)?;
        #[cfg(not(feature = "compare-wasmtime"))]
        let instance = Linker::<StoreLimits>::new(module.engine())
            .instantiate_and_start(&mut store, module)
            .map_err(error)?;
        let abi = instance
            .get_typed_func::<(), u32>(store.as_context_mut(), "logisim_abi")
            .map_err(error)?;
        let abi = abi.call(&mut store, ()).map_err(error)?;
        if !supports_abi(abi) {
            return Err("Unsupported plugin ABI version.".into());
        }
        let frame = frame::FrameApi::load(&instance, &mut store, abi)?;
        Ok(Self {
            ui: presentation::load(&instance, &mut store)?,
            interaction: metadata::load(
                &instance,
                &mut store,
                "logisim_interaction_version",
                "logisim_interaction",
                logisim_plugin_api::interaction::VERSION,
            )?,
            reconfiguration: metadata::load(
                &instance,
                &mut store,
                "logisim_reconfiguration_version",
                "logisim_reconfiguration",
                logisim_plugin_api::reconfiguration::VERSION,
            )?,
            raster: raster::RasterApi::load(&instance, &mut store)?,
            frame,
            abi,
            memory: instance
                .get_memory(store.as_context_mut(), "memory")
                .ok_or("Plugin has no exported memory.")?,
            buffer: instance
                .get_typed_func(store.as_context_mut(), "logisim_buffer")
                .map_err(error)?,
            call: instance
                .get_typed_func(store.as_context_mut(), "logisim_call")
                .map_err(error)?,
            store,
        })
    }
    fn call(&mut self, request: &Request) -> Result<Response> {
        let bytes = timed!("encode", serde_json::to_vec(request)).map_err(error)?;
        if bytes.len() > MAX_MESSAGE {
            return Err("Plugin request exceeds message limit.".into());
        }
        self.store.set_fuel(fuel_budget()).map_err(error)?;
        let offset = timed!(
            "guest_buffer",
            self.buffer.call(&mut self.store, bytes.len() as u32)
        )
        .map_err(error)?;
        timed!(
            "encode",
            self.memory.write(&mut self.store, offset as usize, &bytes)
        )
        .map_err(error)?;
        let packed = timed!(
            "guest_dispatch",
            self.call.call(&mut self.store, bytes.len() as u32)
        );
        #[cfg(feature = "engine-study")]
        study::record_remaining(&self.store)?;
        let packed = packed.map_err(error)?;
        let length = (packed >> 32) as usize;
        if length > MAX_MESSAGE {
            return Err("Plugin response exceeds message limit.".into());
        }
        #[cfg(feature = "plugin-metrics")]
        metrics::on_call(|c| {
            c.json_calls += 1;
            c.request_bytes += bytes.len();
            c.response_bytes += length;
        });
        #[cfg(feature = "plugin-metrics")]
        let _decode = metrics::timer("decode");
        let mut output = vec![0; length];
        self.memory
            .read(&self.store, packed as u32 as usize, &mut output)
            .map_err(error)?;
        match serde_json::from_slice(&output).map_err(error)? {
            Response::Error(e) => Err(e.chars().take(2048).collect()),
            other => Ok(other),
        }
    }
    pub fn evaluate(&mut self, component: &Component, request: Evaluation) -> Result<Update> {
        component.tool.validate_evaluation(&request)?;
        let Response::Update(update) = self.call(&Request::Evaluate(request))? else {
            return Err("Plugin did not return an evaluation result.".into());
        };
        component.tool.validate_update(&update)?;
        Ok(update)
    }
}
#[derive(Clone, Default)]
pub struct Registry {
    entries: Arc<BTreeMap<(String, String), Arc<Plugin>>>,
    failures: Arc<BTreeMap<(String, String), String>>,
}
impl Registry {
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.entries, &other.entries) && Arc::ptr_eq(&self.failures, &other.failures)
    }
    /// Preserve a running VM only when its exact loaded module is still registered.
    pub fn same_plugin(&self, other: &Self, component: &Component) -> bool {
        let key = (component.plugin.clone(), component.version.clone());
        match (self.entries.get(&key), other.entries.get(&key)) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
    pub fn load(&mut self, bytes: &[u8]) -> Result<()> {
        let plugin = Plugin::load(bytes)?;
        self.insert(plugin)
    }
    pub fn insert(&mut self, plugin: Plugin) -> Result<()> {
        let key = (plugin.manifest.id.clone(), plugin.manifest.version.clone());
        if self.entries.contains_key(&key) {
            return Err("This plugin version is already loaded.".into());
        }
        Arc::make_mut(&mut self.failures).remove(&key);
        Arc::make_mut(&mut self.entries).insert(key, Arc::new(plugin));
        Ok(())
    }
    pub fn unavailable(&mut self, plugin: String, version: String, message: String) {
        let key = (plugin, version);
        Arc::make_mut(&mut self.entries).remove(&key);
        Arc::make_mut(&mut self.failures).insert(key, message);
    }
    pub fn libraries(
        &self,
    ) -> impl Iterator<Item = logisim_plugin_api::presentation::Library> + '_ {
        self.entries.values().map(|p| p.library())
    }
    pub fn manifests(&self) -> impl Iterator<Item = &Manifest> {
        self.entries.values().map(|p| p.manifest())
    }
    pub fn instantiate(&self, component: &Component) -> Result<Instance> {
        self.plugin(component)?.instantiate(component)
    }
    pub fn instantiate_stable(&self, component: &Component) -> Result<Option<StableInstance>> {
        self.plugin(component)?.instantiate_stable(component)
    }
    fn plugin(&self, component: &Component) -> Result<&Plugin> {
        self.entries
            .get(&(component.plugin.clone(), component.version.clone()))
            .ok_or_else(|| {
                if let Some(message) = self
                    .failures
                    .get(&(component.plugin.clone(), component.version.clone()))
                {
                    return message.clone();
                }
                format!(
                    "Plugin not loaded: {} {}",
                    component.plugin, component.version
                )
            })
            .map(Arc::as_ref)
    }
}
