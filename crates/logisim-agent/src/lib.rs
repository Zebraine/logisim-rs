//! Local agent protocol. A service owns its sessions for the lifetime of one transport.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::HashMap, path::PathBuf};

mod digest;
pub mod io;
mod project;
mod settings;
mod simulation;
use project::Session;

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
        Ok(data) => json!({"ok":true,"protocol":PROTOCOL,"data":data}),
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
pub struct Patch {
    pub expected_sha256: Option<String>,
    pub ops: Vec<Operation>,
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
    ProjectOpen {
        path: PathBuf,
    },
    ProjectSnapshot {
        session: String,
    },
    ProjectClose {
        session: String,
    },
    SimulationOpen {
        session: String,
    },
    SimulationState {
        session: String,
    },
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
        && ops.iter().any(|op| !op.is_object())
    {
        return Err(Error::invalid(
            "Each operation must be an object with key and value.",
        ));
    }
    Ok(())
}

impl Patch {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let value = serde_json::from_slice(bytes).map_err(|e| Error::new("invalid_json", e))?;
        check_arguments(&value)?;
        // Decode the original object to retain serde's duplicate-field checks.
        serde_json::from_slice(bytes).map_err(Error::invalid)
    }
}
impl Request {
    pub fn from_json(value: Value) -> Result<Self> {
        check_arguments(&value["arguments"])?;
        serde_json::from_value(value).map_err(Error::invalid)
    }
}

#[derive(Default)]
pub struct Service {
    sessions: HashMap<String, Session>,
}
impl Service {
    pub fn call(&mut self, request: Request) -> Result<Value> {
        match request {
            Request::SettingsCatalog {} => Ok(settings::catalog()),
            Request::SettingsGet { config } => settings::get(&config),
            Request::SettingsPatch {
                config,
                expected_sha256,
                ops,
                dry_run,
            } => settings::patch(&config, expected_sha256.as_deref(), ops, dry_run),
            Request::ProjectOpen { path } => {
                let session = Session::open(&path)?;
                let mut random = [0; 16];
                getrandom::fill(&mut random).map_err(|e| Error::new("unavailable", e))?;
                let id = format!("{:032x}", u128::from_be_bytes(random));
                let data = session.summary(&id);
                self.sessions.insert(id, session);
                Ok(data)
            }
            Request::ProjectClose { session } => {
                self.sessions.remove(&session).ok_or_else(unknown_session)?;
                Ok(json!({"session_id":session,"closed":true}))
            }
            Request::ProjectSnapshot { session } => Ok(self
                .sessions
                .get(&session)
                .ok_or_else(unknown_session)?
                .snapshot(&session)),
            Request::SimulationOpen { session } => {
                let doc = self
                    .sessions
                    .get_mut(&session)
                    .ok_or_else(unknown_session)?;
                let state = doc
                    .simulation
                    .get_or_insert_with(|| simulation::observe(&doc.project, &doc.path, &session));
                Ok(
                    json!({"session_id":session,"main":doc.project.main,"opened":true,"engine":state["engine"],"error":state["error"],"plugins":state["plugins"]}),
                )
            }
            Request::SimulationState { session } => self
                .sessions
                .get(&session)
                .ok_or_else(unknown_session)?
                .simulation
                .clone()
                .ok_or_else(|| {
                    Error::new(
                        "simulation_not_open",
                        "Open the simulation for this project session first.",
                    )
                }),
        }
    }
}

fn unknown_session() -> Error {
    Error::new(
        "unknown_session",
        "Project session is not open in this process.",
    )
}
#[cfg(test)]
mod tests;
