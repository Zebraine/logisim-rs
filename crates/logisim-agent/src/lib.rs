//! Local agent protocol. A service owns its sessions for the lifetime of one transport.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

mod digest;
pub mod io;
pub mod mcp;
mod paths;
mod plugins;
mod project;
mod settings;
mod simulation;
use project::Session;
pub use project::patch::ProjectOp;
pub use simulation::SimulationMode;
use simulation::control::{Action, Input};

pub const PROTOCOL: u8 = 1;
#[derive(Debug, Serialize)]
pub struct Error {
    pub code: &'static str,
    pub message: String,
    pub details: Value,
}
impl Error {
    pub fn new(code: &'static str, message: impl ToString) -> Self {
        Self {
            code,
            message: message.to_string(),
            details: json!({}),
        }
    }
    pub fn invalid(message: impl ToString) -> Self {
        Self::new("invalid_arguments", message)
    }
}
pub type Result<T> = std::result::Result<T, Error>;

pub fn envelope(result: Result<Value>) -> Value {
    match result {
        Ok(data) => {
            let mut result = json!({"ok":true,"protocol":PROTOCOL});
            result["data"] = data;
            result
        }
        Err(error) => json!({"ok":false,"protocol":PROTOCOL,"error":error}),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub key: String,
    pub value: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch<T = Operation> {
    pub expected_sha256: Option<String>,
    pub ops: Vec<T>,
}
#[derive(Debug, Deserialize)]
#[serde(
    tag = "tool",
    content = "arguments",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Request {
    SettingsCatalog {},
    SettingsGet {
        config: PathBuf,
    },
    SettingsPatch {
        config: PathBuf,
        dry_run: bool,
        expected_sha256: Option<String>,
        ops: Vec<Operation>,
    },
    ProjectCatalog {},
    PluginCatalog {
        path: PathBuf,
    },
    ProjectOpen {
        path: PathBuf,
    },
    ProjectSnapshot {
        session: String,
    },
    ProjectPatch {
        session: String,
        dry_run: bool,
        expected_sha256: Option<String>,
        ops: Vec<ProjectOp>,
    },
    ProjectSave {
        session: String,
        expected_sha256: Option<String>,
        #[serde(default = "backup_enabled")]
        backup: bool,
    },
    ProjectConvert {
        session: String,
        output: PathBuf,
        expected_output_sha256: Option<String>,
        #[serde(default = "backup_enabled")]
        backup: bool,
    },
    ProjectClose {
        session: String,
    },
    SimulationOpen {
        session: String,
        #[serde(default)]
        mode: SimulationMode,
    },
    SimulationState {
        session: String,
    },
    SimulationPluginAction {
        session: String,
        id: Option<logisim_core::Id>,
        agent_id: Option<String>,
        action: String,
        #[serde(default)]
        argument: String,
    },
    SimulationSetInput {
        session: String,
        id: logisim_core::Id,
        #[serde(default, deserialize_with = "supplied")]
        value: Option<u32>,
        #[serde(default, deserialize_with = "supplied")]
        unknown: Option<bool>,
    },
    SimulationSettle {
        session: String,
    },
    SimulationStep {
        session: String,
    },
    SimulationTick {
        session: String,
    },
    SimulationReset {
        session: String,
    },
    SimulationSetEnabled {
        session: String,
        enabled: bool,
    },
}

// Missing optional arguments and explicit JSON null have different meanings in the protocol.
fn supplied<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> std::result::Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

// Serde structs also accept positional arrays. Protocol v1 uses named objects so
// a write always has an explicit mode and every edit identifies its key.
fn check_arguments(arguments: &Value) -> Result<()> {
    if !arguments.is_object() {
        return Err(Error::invalid(
            "Expected a JSON object with named arguments.",
        ));
    }
    if let Some(ops) = arguments.get("ops").and_then(Value::as_array)
        && let Some(index) = ops.iter().position(|op| !op.is_object())
    {
        return Err(project::patch::indexed(
            Error::invalid("Each operation must be a named JSON object."),
            index,
        ));
    }
    Ok(())
}

fn decode_error<T: serde::de::DeserializeOwned>(value: &Value, err: serde_json::Error) -> Error {
    if let Some(ops) = value["ops"].as_array() {
        for (i, op) in ops.iter().enumerate() {
            if let Err(e) = T::deserialize(op) {
                return project::patch::indexed(Error::invalid(e), i);
            }
        }
    }
    Error::invalid(err)
}
impl<T: serde::de::DeserializeOwned> Patch<T> {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let value = serde_json::from_slice(bytes).map_err(|e| Error::new("invalid_json", e))?;
        check_arguments(&value)?;
        // Decode the original object to retain serde's duplicate-field checks.
        serde_json::from_slice(bytes).map_err(|e| decode_error::<T>(&value, e))
    }
}
impl Request {
    fn session(&self) -> Option<&str> {
        match self {
            Self::ProjectSnapshot { session }
            | Self::ProjectPatch { session, .. }
            | Self::ProjectConvert { session, .. }
            | Self::ProjectSave { session, .. }
            | Self::ProjectClose { session }
            | Self::SimulationOpen { session, .. }
            | Self::SimulationState { session }
            | Self::SimulationPluginAction { session, .. }
            | Self::SimulationSetInput { session, .. }
            | Self::SimulationSettle { session }
            | Self::SimulationStep { session }
            | Self::SimulationTick { session }
            | Self::SimulationReset { session }
            | Self::SimulationSetEnabled { session, .. } => Some(session),
            Self::ProjectOpen { .. }
            | Self::SettingsCatalog {}
            | Self::SettingsGet { .. }
            | Self::SettingsPatch { .. }
            | Self::ProjectCatalog {}
            | Self::PluginCatalog { .. } => None,
        }
    }

    pub fn from_json(value: Value) -> Result<Self> {
        check_arguments(&value["arguments"])?;
        Self::deserialize(&value).map_err(|e| {
            if value["tool"] == "project_patch" {
                decode_error::<ProjectOp>(&value["arguments"], e)
            } else {
                Error::invalid(e)
            }
        })
    }
}

const MAX_SESSIONS: usize = 64;

#[derive(Default)]
pub struct Service {
    root: Option<paths::Root>,
    sessions: HashMap<String, Session>,
}
impl Service {
    pub fn with_root(directory: &Path) -> Result<Self> {
        Ok(Self {
            root: Some(paths::Root::new(directory)?),
            ..Self::default()
        })
    }

    /// Check a caller-supplied path, including CLI patch files read before dispatch.
    pub fn resolve_path(&self, path: &Path) -> Result<PathBuf> {
        match &self.root {
            Some(root) => root.resolve(path),
            None => Ok(path.to_owned()),
        }
    }

    fn open_session(&mut self, id: String, path: &Path) -> Result<Value> {
        let session = Session::open(&self.resolve_path(path)?)?;
        let data = session.summary(&id);
        self.sessions.insert(id, session);
        Ok(data)
    }

    fn control(&mut self, session: &str, action: Action) -> Result<Value> {
        let doc = self.sessions.get_mut(session).ok_or_else(unknown_session)?;
        let runtime = doc.simulation.as_mut().ok_or_else(simulation_not_open)?;
        runtime.control(&doc.project, session, action)
    }
    pub fn call(&mut self, request: Request) -> Result<Value> {
        match request {
            Request::PluginCatalog { path } => plugins::catalog(&self.resolve_path(&path)?),
            Request::ProjectCatalog {} => Ok(project::catalog::catalog()),
            Request::SettingsCatalog {} => Ok(settings::catalog()),
            Request::SettingsGet { config } => settings::get(&self.resolve_path(&config)?),
            Request::SettingsPatch {
                config,
                expected_sha256,
                ops,
                dry_run,
            } => {
                let config = self.resolve_path(&config)?;
                if !dry_run && let Some(root) = &self.root {
                    root.check_sidecars(&config, false)?;
                }
                settings::patch(&config, expected_sha256.as_deref(), ops, dry_run)
            }
            Request::ProjectOpen { path } => {
                let id = new_session_id(&self.sessions)?;
                self.open_session(id, &path)
            }
            Request::ProjectPatch {
                session,
                dry_run,
                expected_sha256,
                ops,
            } => self
                .sessions
                .get_mut(&session)
                .ok_or_else(unknown_session)?
                .patch(
                    &session,
                    expected_sha256.as_deref(),
                    dry_run,
                    &ops,
                    self.root.as_ref(),
                ),
            Request::ProjectConvert {
                session,
                output,
                expected_output_sha256,
                backup,
            } => {
                let format = logisim_file::ProjectFormat::from_extension(&output)
                    .ok_or_else(|| Error::invalid("Choose a .lrs or .circ destination."))?;
                let path = self.resolve_path(&output)?;
                let doc = self.sessions.get(&session).ok_or_else(unknown_session)?;
                if let Some(root) = &self.root {
                    root.resolve(&doc.path)?;
                    root.check_sidecars(&path, backup)?;
                }
                doc.convert(
                    &session,
                    &path,
                    format,
                    expected_output_sha256.as_deref(),
                    backup,
                )
            }
            Request::ProjectSave {
                session,
                expected_sha256,
                backup,
            } => {
                let doc = self
                    .sessions
                    .get_mut(&session)
                    .ok_or_else(unknown_session)?;
                if let Some(root) = &self.root {
                    root.resolve(&doc.path)?;
                    root.check_sidecars(&doc.path, backup)?;
                }
                doc.save(&session, expected_sha256.as_deref(), backup)
            }
            Request::ProjectClose { session } => {
                self.sessions.remove(&session).ok_or_else(unknown_session)?;
                Ok(json!({"session_id":session,"closed":true}))
            }
            Request::ProjectSnapshot { session } => self
                .sessions
                .get(&session)
                .ok_or_else(unknown_session)?
                .snapshot(&session),
            Request::SimulationOpen { session, mode } => {
                let doc = self
                    .sessions
                    .get_mut(&session)
                    .ok_or_else(unknown_session)?;
                let created = doc.simulation.is_none();
                let runtime = doc.simulation.get_or_insert_with(|| {
                    simulation::Runtime::open(&doc.project, &doc.path, mode)
                });
                let mut state = runtime.state(&doc.project, &session);
                state["opened"] = true.into();
                state["created"] = created.into();
                Ok(state)
            }
            Request::SimulationPluginAction {
                session,
                id,
                agent_id,
                action,
                argument,
            } => {
                let target = project::reference::Reference::from_ids(id, agent_id)
                    .map_err(Error::invalid)?;
                self.control(
                    &session,
                    Action::Plugin {
                        target,
                        action,
                        argument,
                    },
                )
            }
            Request::SimulationSetInput {
                session,
                id,
                value,
                unknown,
            } => {
                let input = match (value, unknown) {
                    (Some(n), None) => Input::Known(n),
                    (None, Some(true)) => Input::Unknown,
                    _ => {
                        return Err(Error::invalid(
                            "Specify exactly one of value or unknown:true.",
                        ));
                    }
                };
                self.control(&session, Action::SetInput(id, input))
            }
            Request::SimulationSettle { session } => self.control(&session, Action::Settle),
            Request::SimulationStep { session } => self.control(&session, Action::Step),
            Request::SimulationTick { session } => self.control(&session, Action::Tick),
            Request::SimulationReset { session } => self.control(&session, Action::Reset),
            Request::SimulationSetEnabled { session, enabled } => {
                self.control(&session, Action::SetEnabled(enabled))
            }
            Request::SimulationState { session } => {
                let doc = self.sessions.get(&session).ok_or_else(unknown_session)?;
                let runtime = doc.simulation.as_ref().ok_or_else(simulation_not_open)?;
                Ok(runtime.state(&doc.project, &session))
            }
        }
    }
}

fn new_session_id<T>(sessions: &HashMap<String, T>) -> Result<String> {
    if sessions.len() >= MAX_SESSIONS {
        return Err(Error::new(
            "session_limit",
            format!(
                "At most {MAX_SESSIONS} project sessions may be open. Close a session before opening another."
            ),
        ));
    }
    loop {
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes).map_err(|e| Error::new("unavailable", e))?;
        let id = format!("{:032x}", u128::from_be_bytes(bytes));
        if !sessions.contains_key(&id) {
            return Ok(id);
        }
    }
}

fn backup_enabled() -> bool {
    true
}

fn simulation_not_open() -> Error {
    Error::new(
        "simulation_not_open",
        "Open the simulation for this project session first.",
    )
}
fn unknown_session() -> Error {
    Error::new(
        "unknown_session",
        "Project session is not open in this process.",
    )
}
#[cfg(test)]
mod tests;
