//! The export directory is static. Inspect it before compiling, accessing a native
//! cache or executing a start function. The engine still validates the entire module.
use super::*;
use logisim_plugin_api::AllowedEngines;
use wasmparser::{ExternalKind, Parser, Payload};

#[derive(Clone, Copy)]
pub(super) struct Declaration(pub Option<AllowedEngines>);

impl Declaration {
    pub fn read(bytes: &[u8]) -> Result<Self> {
        check_binary(bytes)?;
        let mut entry = None;
        for payload in Parser::new(0).parse_all(bytes) {
            if let Payload::ExportSection(exports) = payload.map_err(error)? {
                for export in exports {
                    let export = export.map_err(error)?;
                    let engines = match export.name {
                        "logisim_abi" => None,
                        name if name.starts_with("logisim_abi4_") => Some(
                            [
                                AllowedEngines::Wasmi,
                                AllowedEngines::Wasmtime,
                                AllowedEngines::Both,
                            ]
                            .into_iter()
                            .find(|e| e.entry() == name)
                            .ok_or("Unknown ABI 4 engine declaration.")?,
                        ),
                        _ => continue,
                    };
                    if export.kind != ExternalKind::Func || entry.replace(Self(engines)).is_some() {
                        return Err("Expected one plugin ABI function export.".into());
                    }
                }
                break;
            }
        }
        entry.ok_or_else(|| "Missing plugin ABI entry point.".into())
    }

    pub fn select(self, requested: WasmEngine) -> Result<WasmEngine> {
        match (self.0, requested) {
            (None, WasmEngine::Wasmtime) => {
                eprintln!("Plugin ABI 1–3 is wasmi only. Using wasmi for this module.");
                Ok(WasmEngine::Wasmi)
            }
            (Some(AllowedEngines::Wasmi), WasmEngine::Wasmtime)
            | (Some(AllowedEngines::Wasmtime), WasmEngine::Wasmi) => Err(format!(
                "Plugin ABI 4 does not permit the selected {} engine.",
                requested.code()
            )),
            _ => Ok(requested),
        }
    }

    pub fn entry(self) -> &'static str {
        self.0.map_or("logisim_abi", AllowedEngines::entry)
    }
}
