//! Sandboxed module loading. No filesystem access or implicit plugin discovery.
#[cfg(test)]
mod tests;
use logisim_plugin_api::{
    ABI_VERSION, Component, Evaluation, MAX_MESSAGE, Manifest, Request, Response, Update,
};
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

pub struct Plugin {
    module: Module,
    manifest: Manifest,
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
        Ok(Self { module, manifest })
    }
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn instantiate(&self, component: &Component) -> Result<Instance> {
        component.validate()?;
        if component.plugin != self.manifest.id
            || component.version != self.manifest.version
            || !self.manifest.tools.contains(&component.tool)
        {
            return Err("Plugin version or definition differs from the saved component.".into());
        }
        Instance::new(&self.module)
    }
}
pub struct Instance {
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
        if abi.call(&mut store, ()).map_err(error)? != ABI_VERSION {
            return Err("Unsupported plugin ABI version.".into());
        }
        Ok(Self {
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
        component.tool.validate_properties(&request.properties)?;
        if request.tool != component.tool.id
            || request.inputs.len() != component.tool.ports.len()
            || request.state.len() > logisim_plugin_api::MAX_STATE
        {
            return Err("Invalid plugin evaluation request.".into());
        }
        for (v, p) in request.inputs.iter().zip(&component.tool.ports) {
            v.validate()?;
            if v.width != p.width {
                return Err("Invalid plugin input width.".into());
            }
        }
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
}
impl Registry {
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.entries, &other.entries)
    }
    pub fn load(&mut self, bytes: &[u8]) -> Result<()> {
        let plugin = Plugin::load(bytes)?;
        let key = (plugin.manifest.id.clone(), plugin.manifest.version.clone());
        if self.entries.contains_key(&key) {
            return Err("This plugin version is already loaded.".into());
        }
        Arc::make_mut(&mut self.entries).insert(key, Arc::new(plugin));
        Ok(())
    }
    pub fn manifests(&self) -> impl Iterator<Item = &Manifest> {
        self.entries.values().map(|p| p.manifest())
    }
    pub fn instantiate(&self, component: &Component) -> Result<Instance> {
        self.entries
            .get(&(component.plugin.clone(), component.version.clone()))
            .ok_or_else(|| {
                format!(
                    "Plugin not loaded: {} {}",
                    component.plugin, component.version
                )
            })?
            .instantiate(component)
    }
}
