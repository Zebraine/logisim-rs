//! MCP stdio transport. Only requests with a JSON-RPC ID can perform an operation.
use logisim_agent::{Error, Request, Service, envelope, io};
use serde_json::{Value, json};
use std::io::{BufRead, Write};

const REVISION: &str = "2025-11-25";
const TOOLS: [(&str, &str); 17] = [
    (
        "settings_catalog",
        "List portable v16 preference keys, defaults and allowed values.",
    ),
    (
        "settings_get",
        "Read preferences and the SHA-256 of an explicit configuration file.",
    ),
    (
        "settings_patch",
        "Validate a batch of portable preference edits. Set dry_run to false to commit.",
    ),
    (
        "project_catalog",
        "List component kinds, accepted aliases and writable property schemas. No session is required.",
    ),
    (
        "project_open",
        "Read a circuit file into a session owned by this connection.",
    ),
    (
        "project_snapshot",
        "Read the circuit definitions, components, wires and interfaces in a session.",
    ),
    (
        "project_patch",
        "Preview or apply an ordered batch to the in-memory project. Use project_save to write it.",
    ),
    (
        "project_save",
        "Atomically save the session to its source file. Checks the source SHA-256 and makes a one-generation backup by default.",
    ),
    (
        "project_close",
        "Release the project and its simulation snapshot.",
    ),
    (
        "simulation_open",
        "Open the root simulation and return current state. Reopening retains the first engine selection and current runtime.",
    ),
    (
        "simulation_state",
        "Read current four-state signals, counters, plugin status and diagnostics.",
    ),
    (
        "simulation_set_input",
        "Set a root Pin, Switch or DipSwitch input. Enabled propagation settles immediately.",
    ),
    (
        "simulation_settle",
        "Finish pending propagation without advancing the clock.",
    ),
    (
        "simulation_step",
        "Execute one event propagation batch, including when paused.",
    ),
    (
        "simulation_tick",
        "Advance one tick in a settled hierarchy containing a clock, then settle.",
    ),
    (
        "simulation_reset",
        "Reset the session simulation and settle its initial signals.",
    ),
    (
        "simulation_set_enabled",
        "Pause automatic propagation or resume and drain pending work.",
    ),
];

fn project_operations() -> Value {
    let id = json!({"type":"integer","minimum":1});
    let point = json!({"type":"array","minItems":2,"maxItems":2,"items":{"type":"integer","minimum":-1000000,"maximum":1000000}});
    let properties = json!({"type":"object","description":"Use project_catalog or the component snapshot's property_schema for allowed fields and values."});
    let mut variants = Vec::new();
    for (op, required, optional) in [
        (
            "add_component",
            &["circuit", "kind", "position"][..],
            &["properties", "circuit_id"][..],
        ),
        ("move_component", &["id", "position"], &["circuit"]),
        ("update_component", &["id", "properties"], &["circuit"]),
        ("remove_component", &["id"], &["circuit"]),
        ("add_wire", &["circuit", "from", "to"], &[]),
        ("remove_wire", &["id"], &["circuit"]),
        ("add_circuit", &["name"], &[]),
        ("rename_circuit", &["id", "name"], &[]),
        ("remove_circuit", &["id"], &[]),
        ("set_main_circuit", &["id"], &[]),
    ] {
        let mut fields = json!({"op":{"const":op}});
        for &key in required.iter().chain(optional) {
            fields[key] = match key {
                "id" | "circuit" | "circuit_id" => id.clone(),
                "position" | "from" | "to" => point.clone(),
                "properties" => properties.clone(),
                "kind" => {
                    json!({"type":"string","minLength":1,"description":"Editable component kind, such as Bit Adder, Register, or subcircuit. A subcircuit requires a target circuit_id."})
                }
                "name" => json!({"type":"string","minLength":1}),
                _ => unreachable!(),
            };
        }
        let required: Vec<_> = std::iter::once("op")
            .chain(required.iter().copied())
            .collect();
        let mut variant = json!({"type":"object","required":required,"additionalProperties":false,"properties":fields});
        if op == "add_component" {
            variant["if"] = json!({"properties":{"kind":{"pattern":"^[Ss][Uu][Bb][Cc][Ii][Rr][Cc][Uu][Ii][Tt]$"}}});
            variant["then"] = json!({"required":["circuit_id"]});
            variant["else"] = json!({"not":{"required":["circuit_id"]}});
        }
        variants.push(variant);
    }
    json!({"type":"array","items":{"oneOf":variants}})
}

fn tool_list() -> Value {
    let string = json!({"type":"string","minLength":1});
    let mut tools = Vec::with_capacity(TOOLS.len());
    for (name, description) in TOOLS {
        let (properties, required) = match name {
            "settings_catalog" | "project_catalog" => (json!({}), json!([])),
            "settings_get" => (json!({"config": string}), json!(["config"])),
            "settings_patch" => (
                json!({
                    "config": string, "dry_run": {"type": "boolean"},
                    "expected_sha256": {"type": "string", "pattern": "^[0-9a-fA-F]{64}$"},
                    "ops": {"type": "array", "items": {
                        "type": "object", "required": ["key", "value"], "additionalProperties": false,
                        "properties": {"key": string, "value": {}}
                    }}
                }),
                json!(["config", "dry_run", "ops"]),
            ),
            "project_patch" => (
                json!({"session":string,"dry_run":{"type":"boolean"},"expected_sha256":{"type":"string","pattern":"^[0-9a-fA-F]{64}$"},"ops":project_operations()}),
                json!(["session", "dry_run", "ops"]),
            ),
            "project_save" => (
                json!({"session":string,"expected_sha256":{"type":"string","pattern":"^[0-9a-fA-F]{64}$"},"backup":{"type":"boolean","default":true}}),
                json!(["session"]),
            ),
            "simulation_open" => (
                json!({"session":string, "mode":{"type":"string", "enum":["auto","stable","event"], "default":"auto"}}),
                json!(["session"]),
            ),
            "simulation_set_input" => (
                json!({"session":string, "id":{"type":"integer","minimum":1},
                       "value":{"type":"integer","minimum":0,"maximum":u32::MAX}, "unknown":{"const":true}}),
                json!(["session", "id"]),
            ),
            "simulation_set_enabled" => (
                json!({"session":string, "enabled":{"type":"boolean"}}),
                json!(["session", "enabled"]),
            ),
            "project_open" => (json!({"path": string}), json!(["path"])),
            _ => (json!({"session": string}), json!(["session"])),
        };
        let mut schema = json!({"type":"object", "properties":properties, "required":required, "additionalProperties":false});
        if name == "simulation_set_input" {
            schema["oneOf"] = json!([{"required":["value"]},{"required":["unknown"]}]);
        }
        tools.push(json!({"name":name, "description":description, "inputSchema":schema}));
    }
    json!({"tools":tools})
}

#[derive(Default, PartialEq, Eq)]
enum Phase {
    #[default]
    New,
    Negotiated,
    Ready,
}

struct Failure(i32, Error);
impl Failure {
    fn invalid_params(message: impl ToString) -> Self {
        Self(-32602, Error::invalid(message))
    }
    fn response(self, id: Value) -> Value {
        json!({"jsonrpc":"2.0","id":id,"error":{"code":self.0,"message":self.1.message,"data":self.1}})
    }
}
#[derive(Default)]
struct Connection {
    phase: Phase,
    service: Service,
}
impl Connection {
    fn dispatch(&mut self, method: &str, params: &Value) -> Result<Value, Failure> {
        if method == "ping" {
            return Ok(json!({}));
        }
        if method == "initialize" {
            if self.phase != Phase::New {
                return Err(Failure(
                    -32600,
                    Error::new("already_initialized", "Connection already initialized."),
                ));
            }
            let requested = params["protocolVersion"]
                .as_str()
                .ok_or_else(|| Failure::invalid_params("Missing protocolVersion."))?;
            if !params["capabilities"].is_object()
                || !params["clientInfo"]["name"].is_string()
                || !params["clientInfo"]["version"].is_string()
            {
                return Err(Failure::invalid_params(
                    "Expected clientInfo and capabilities.",
                ));
            }
            self.phase = Phase::Negotiated;
            let version = match requested {
                "2025-06-18" => requested,
                _ => REVISION,
            };
            return Ok(
                json!({"protocolVersion":version,"capabilities":{"tools":{}},"serverInfo":{"name":"logisim-agent","version":env!("CARGO_PKG_VERSION")}}),
            );
        }
        if self.phase != Phase::Ready {
            return Err(Failure(
                -32002,
                Error::new(
                    "not_initialized",
                    "Complete the MCP initialization handshake first.",
                ),
            ));
        }
        match method {
            "tools/list" => Ok(tool_list()),
            "tools/call" => {
                let name = params["name"]
                    .as_str()
                    .ok_or_else(|| Failure::invalid_params("Expected a tool name."))?;
                if !TOOLS.iter().any(|(n, _)| *n == name) {
                    return Err(Failure(
                        -32602,
                        Error::new("unknown_tool", format!("Unknown tool: {name}")),
                    ));
                }
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let request = Request::from_json(json!({"tool":name,"arguments":arguments}))
                    .map_err(|e| Failure(-32602, e))?;
                let data = self.service.call(request).map_err(|e| {
                    Failure(
                        if e.code == "invalid_arguments" {
                            -32602
                        } else {
                            -32000
                        },
                        e,
                    )
                })?;
                let result = envelope(Ok(data));
                Ok(
                    json!({"content":[{"type":"text","text":result.to_string()}],"structuredContent":result}),
                )
            }
            _ => Err(Failure(
                -32601,
                Error::new("method_not_found", "Unknown JSON-RPC method."),
            )),
        }
    }
    fn receive(&mut self, message: Value) -> Option<Value> {
        let id = message.get("id");
        let method = message["method"].as_str();
        if message["jsonrpc"] != "2.0"
            || method.is_none()
            || id.is_some_and(|id| !(id.is_string() || id.is_i64() || id.is_u64()))
        {
            return Some(
                Failure(
                    -32600,
                    Error::new("invalid_request", "Invalid JSON-RPC 2.0 request."),
                )
                .response(Value::Null),
            );
        }
        let method = method.unwrap();
        let Some(id) = id else {
            if method == "notifications/initialized" && self.phase == Phase::Negotiated {
                self.phase = Phase::Ready;
            }
            return None;
        };
        let result = if message.get("params").is_some_and(|p| !p.is_object()) {
            Err(Failure::invalid_params(
                "Expected parameters as a JSON object.",
            ))
        } else {
            self.dispatch(method, &message["params"])
        };
        Some(match result {
            Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
            Err(error) => error.response(id.clone()),
        })
    }
}
pub fn run(input: impl BufRead, mut out: impl Write) -> std::io::Result<()> {
    let mut connection = Connection::default();
    io::lines(input, |line| {
        let reply = match line {
            Ok(message) => connection.receive(message),
            Err(e) => Some(
                Failure(
                    if e.code == "invalid_json" {
                        -32700
                    } else {
                        -32600
                    },
                    e,
                )
                .response(Value::Null),
            ),
        };
        if let Some(reply) = reply {
            io::write(&mut out, &reply)?;
        }
        Ok(())
    })
}
