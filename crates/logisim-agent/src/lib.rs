//! Local agent protocol. A service owns its sessions for the lifetime of one transport.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::HashMap, path::PathBuf};

mod digest;
pub mod io;
mod settings;

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

struct Session {
    project: logisim_core::Project,
    path: PathBuf,
    sha256: String,
    warnings: Vec<String>,
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
                let bytes = io::read(&path)?;
                let xml =
                    std::str::from_utf8(&bytes).map_err(|e| Error::new("invalid_project", e))?;
                let imported =
                    logisim_file::parse(xml).map_err(|e| Error::new("invalid_project", e))?;
                let mut random = [0u8; 16];
                getrandom::fill(&mut random).map_err(|e| Error::new("unavailable", e))?;
                let id = format!("{:032x}", u128::from_be_bytes(random));
                let session = Session {
                    project: imported.document,
                    path: path.canonicalize().map_err(|e| Error::new("io", e))?,
                    sha256: digest::sha256(&bytes),
                    warnings: imported.warnings,
                };
                let data = json!({"session_id":id,"path":session.path,"source_sha256":session.sha256,"schema":1,"main":session.project.main,"partial":!session.warnings.is_empty(),"warnings":session.warnings});
                self.sessions.insert(id, session);
                Ok(data)
            }
            Request::ProjectClose { session } => {
                self.sessions.remove(&session).ok_or_else(|| {
                    Error::new(
                        "unknown_session",
                        "Project session is not open in this process.",
                    )
                })?;
                Ok(json!({"session_id":session,"closed":true}))
            }
            _ => Err(Error::new("unknown_tool", "Tool is not available.")),
        }
    }
}

#[cfg(test)]
mod tests;
