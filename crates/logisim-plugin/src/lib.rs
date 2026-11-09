//! Sandboxed module loading. No filesystem access or implicit plugin discovery.
mod frame;
mod metadata;
mod presentation;
mod reconfiguration;
mod stable;
#[cfg(test)]
mod tests;
use logisim_plugin_api::{
    Component, Evaluation, MAX_MESSAGE, Manifest, Request, Response, Update, supports_abi,
};
pub use stable::StableInstance;
use std::{collections::BTreeMap, sync::Arc};
use wasmi::{
    Config, Engine, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder, TypedFunc,
};
pub const MAX_MODULE: usize = 4 * 1024 * 1024;
const FUEL: u64 = 10_000_000;
type Result<T> = std::result::Result<T, String>;
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Clone)]
pub struct Plugin {
    module: Module,
    manifest: Manifest,
    presentation: logisim_plugin_api::presentation::Presentation,
    reconfiguration: logisim_plugin_api::reconfiguration::Policy,
}
impl Plugin {
    pub fn load(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_MODULE || !bytes.starts_with(b"\0asm") {
            return Err("Choose a WebAssembly binary within the 4 MiB limit.".into());
        }
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, bytes).map_err(error)?;
        if module.imports().next().is_some() {
            return Err("Plugin imports are not supported (including WASI).".into());
        }
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
        let reconfiguration: logisim_plugin_api::reconfiguration::Policy =
            instance.metadata(instance.reconfiguration)?;
        reconfiguration.validate(&manifest)?;
        Ok(Self {
            module,
            manifest,
            presentation,
            reconfiguration,
        })
    }
    pub fn library(&self) -> logisim_plugin_api::presentation::Library {
        logisim_plugin_api::presentation::Library {
            manifest: self.manifest.clone(),
            presentation: self.presentation.clone(),
        }
    }
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn instantiate(&self, component: &Component) -> Result<Instance> {
        self.check_component(component)?;
        Instance::new(&self.module)
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
    reconfiguration: Option<TypedFunc<(), u64>>,
    frame: Option<frame::FrameApi>,
    abi: u32,
    store: Store<StoreLimits>,
    memory: Memory,
    buffer: TypedFunc<u32, u32>,
    call: TypedFunc<u32, u64>,
}
impl Instance {
    fn new(module: &Module) -> Result<Self> {
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
        store.set_fuel(FUEL).map_err(error)?;
        let instance = Linker::<StoreLimits>::new(module.engine())
            .instantiate_and_start(&mut store, module)
            .map_err(error)?;
        let abi = instance
            .get_typed_func::<(), u32>(&store, "logisim_abi")
            .map_err(error)?;
        let abi = abi.call(&mut store, ()).map_err(error)?;
        if !supports_abi(abi) {
            return Err("Unsupported plugin ABI version.".into());
        }
        let frame = frame::FrameApi::load(&instance, &mut store, abi)?;
        Ok(Self {
            ui: presentation::load(&instance, &mut store)?,
            reconfiguration: metadata::load(
                &instance,
                &mut store,
                "logisim_reconfiguration_version",
                "logisim_reconfiguration",
                logisim_plugin_api::reconfiguration::VERSION,
            )?,
            frame,
            abi,
            memory: instance
                .get_memory(&store, "memory")
                .ok_or("Plugin has no exported memory.")?,
            buffer: instance
                .get_typed_func(&store, "logisim_buffer")
                .map_err(error)?,
            call: instance
                .get_typed_func(&store, "logisim_call")
                .map_err(error)?,
            store,
        })
    }
    fn call(&mut self, request: &Request) -> Result<Response> {
        let bytes = serde_json::to_vec(request).map_err(error)?;
        if bytes.len() > MAX_MESSAGE {
            return Err("Plugin request exceeds message limit.".into());
        }
        self.store.set_fuel(FUEL).map_err(error)?;
        let offset = self
            .buffer
            .call(&mut self.store, bytes.len() as u32)
            .map_err(error)?;
        self.memory
            .write(&mut self.store, offset as usize, &bytes)
            .map_err(error)?;
        let packed = self
            .call
            .call(&mut self.store, bytes.len() as u32)
            .map_err(error)?;
        let length = (packed >> 32) as usize;
        if length > MAX_MESSAGE {
            return Err("Plugin response exceeds message limit.".into());
        }
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
