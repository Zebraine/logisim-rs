//! MCP stdio transport. Only requests with a JSON-RPC ID can perform an operation.
use logisim_agent::{Error, Request, Service, envelope, io};
use serde_json::{Value, json};
use std::io::{BufRead, Write};

const REVISION: &str = "2025-11-25";
const SUPPORTED: [&str; 4] = ["2024-11-05", "2025-03-26", "2025-06-18", REVISION];

#[derive(Clone, Copy)]
enum Effect {
    ReadOnly,
    Additive { idempotent: bool },
    Modifies { idempotent: bool },
}

impl Effect {
    fn annotations(self) -> Value {
        let idempotent = match self {
            Self::ReadOnly => true,
            Self::Additive { idempotent } | Self::Modifies { idempotent } => idempotent,
        };
        json!({"readOnlyHint":matches!(self,Self::ReadOnly),
               "destructiveHint":matches!(self,Self::Modifies {..}),
               "idempotentHint":idempotent,"openWorldHint":false})
    }
}

const TOOLS: [(&str, &str, Effect); 19] = [
    (
        "plugin_catalog",
        "Inspect a Wasm manifest, tool ports, actions and document property schemas.",
        Effect::ReadOnly,
    ),
    (
        "settings_catalog",
        "List portable v17 preference keys, defaults and allowed values.",
        Effect::ReadOnly,
    ),
    (
        "settings_get",
        "Read preferences and the SHA-256 of an explicit configuration file.",
        Effect::ReadOnly,
    ),
    (
        "settings_patch",
        "Validate a batch of portable preference edits. Set dry_run to false to commit.",
        Effect::Modifies { idempotent: true },
    ),
    (
        "project_catalog",
        "List component kinds, accepted aliases and writable property schemas. No session is required.",
        Effect::ReadOnly,
    ),
    (
        "project_open",
        "Read a circuit file into a session owned by this connection.",
        Effect::Additive { idempotent: false },
    ),
    (
        "project_snapshot",
        "Read the circuit definitions, components, wires and interfaces in a session.",
        Effect::ReadOnly,
    ),
    (
        "project_patch",
        "Preview or apply an ordered batch to the in-memory project. Use project_save to write it.",
        Effect::Modifies { idempotent: false },
    ),
    (
        "project_save",
        "Atomically save the session to its source file. Checks the source SHA-256 and makes a one-generation backup by default.",
        Effect::Modifies { idempotent: false },
    ),
    (
        "project_close",
        "Release the project and its simulation snapshot.",
        Effect::Modifies { idempotent: true },
    ),
    (
        "simulation_open",
        "Open the root simulation and return current state. Reopening retains the first engine selection and current runtime.",
        Effect::Additive { idempotent: true },
    ),
    (
        "simulation_state",
        "Read current four-state signals, counters, plugin status and diagnostics.",
        Effect::ReadOnly,
    ),
    (
        "simulation_set_input",
        "Set a root Pin, Switch or DipSwitch input. Enabled propagation settles immediately.",
        Effect::Modifies { idempotent: true },
    ),
    (
        "simulation_plugin_action",
        "Invoke a declared runtime action on a root plugin instance. Supply its numeric id or persistent agent_id. Returns changed and the full simulation state.",
        Effect::Modifies { idempotent: false },
    ),
    (
        "simulation_settle",
        "Finish pending propagation without advancing the clock.",
        Effect::Modifies { idempotent: true },
    ),
    (
        "simulation_step",
        "Execute one event propagation batch, including when paused.",
        Effect::Modifies { idempotent: false },
    ),
    (
        "simulation_tick",
        "Advance one tick in a settled hierarchy containing a clock, then settle.",
        Effect::Modifies { idempotent: false },
    ),
    (
        "simulation_reset",
        "Reset the session simulation and settle its initial signals.",
        Effect::Modifies { idempotent: false },
    ),
    (
        "simulation_set_enabled",
        "Pause automatic propagation or resume and drain pending work.",
        Effect::Modifies { idempotent: false },
    ),
];

fn persistent_id(prefix: char) -> Value {
    json!({"type":"string","pattern":format!("^{prefix}-[0-9a-f]{{32}}$")})
}

fn alternatives(numeric: &str, persistent: &str) -> Value {
    json!([{"required":[numeric]},{"required":[persistent]}])
}

fn project_operations() -> Value {
    let id = json!({"type":"integer","minimum":1});
    let point = json!({"type":"array","minItems":2,"maxItems":2,"items":{"type":"integer","minimum":-1000000,"maximum":1000000}});
    let properties = json!({"type":"object","description":"Use project_catalog or the component snapshot's property_schema for allowed fields and values."});
    let mut variants = Vec::new();
    for (op, required, optional, reference) in [
        (
            "add_component",
            &["circuit", "kind", "position"][..],
            &["properties", "circuit_id", "circuit_agent_id"][..],
            None,
        ),
        ("move_component", &["position"], &["circuit"], Some('c')),
        ("update_component", &["properties"], &["circuit"], Some('c')),
        ("remove_component", &[], &["circuit"], Some('c')),
        ("add_wire", &["circuit", "from", "to"], &[], None),
        ("remove_wire", &[], &["circuit"], Some('w')),
        ("add_circuit", &["name"], &[], None),
        ("rename_circuit", &["name"], &["circuit"], Some('k')),
        ("remove_circuit", &[], &["circuit"], Some('k')),
        ("set_main_circuit", &[], &["circuit"], Some('k')),
    ] {
        let mut fields = json!({"op":{"const":op}});
        for &key in required.iter().chain(optional) {
            fields[key] = match key {
                "circuit" | "circuit_id" => id.clone(),
                "circuit_agent_id" => persistent_id('k'),
                "position" | "from" | "to" => point.clone(),
                "properties" => properties.clone(),
                "kind" => {
                    json!({"type":"string","minLength":1,"description":"Use an addable name or alias from project_catalog. Subcircuit instances require one target circuit reference."})
                }
                "name" => json!({"type":"string","minLength":1}),
                _ => unreachable!(),
            };
        }
        let required: Vec<_> = std::iter::once("op")
            .chain(required.iter().copied())
            .collect();
        let mut variant = json!({"type":"object","required":required,"additionalProperties":false,"properties":fields});
        if let Some(prefix) = reference {
            variant["properties"]["id"] = id.clone();
            variant["properties"]["agent_id"] = persistent_id(prefix);
            variant["oneOf"] = alternatives("id", "agent_id");
        }
        if op == "add_component" {
            variant["if"] = json!({"properties":{"kind":{"pattern":"^[Ss][Uu][Bb][Cc][Ii][Rr][Cc][Uu][Ii][Tt]$"}}});
            let targets = alternatives("circuit_id", "circuit_agent_id");
            variant["then"] = json!({"oneOf":targets});
            variant["else"] = json!({"not":{"anyOf":targets}});
        }
        variants.push(variant);
    }
    let identity = json!({"type":"string","pattern":"^[A-Za-z0-9._-]{1,128}$"});
    let path = json!({"type":"string","maxLength":4096,"x-maxUtf8Bytes":4096});
    let source = json!({"type":"object","additionalProperties":false,"required":["plugin","version","path"],
        "properties":{"plugin":identity,"version":identity,"path":path,"fallback":path},
        "anyOf":[{"properties":{"path":{"minLength":1}}},{"required":["fallback"],"properties":{"fallback":{"minLength":1}}}]});
    let mut plugin_fields = logisim_agent::ProjectOp::plugin_document_schema();
    plugin_fields["properties"] = json!({"type":"object","additionalProperties":{"type":"string","maxLength":1024},
        "description":"String values keyed by property ID. Use plugin_catalog or the instance property_schema."});
    plugin_fields["circuit"] = id.clone();
    let mut add = plugin_fields.clone();
    add["op"] = json!({"const":"add_plugin"});
    add["position"] = point;
    add["tool"] = identity.clone();
    add["plugin"] = identity.clone();
    add["version"] = identity;
    add["source"] = source;
    variants.push(json!({"type":"object","additionalProperties":false,"properties":add,
        "required":["op","circuit","position","tool"],"anyOf":[{"required":["source"]},{"required":["plugin","version"]}]}));
    plugin_fields["op"] = json!({"const":"update_plugin"});
    plugin_fields["id"] = id;
    plugin_fields["agent_id"] = persistent_id('c');
    variants.push(json!({"type":"object","additionalProperties":false,"properties":plugin_fields,"required":["op"],"oneOf":alternatives("id","agent_id")}));
    json!({"type":"array","items":{"oneOf":variants}})
}

fn tool_list() -> Value {
    let string = json!({"type":"string","minLength":1});
    let mut tools = Vec::with_capacity(TOOLS.len());
    for (name, description, effect) in TOOLS {
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
            "simulation_plugin_action" => (
                json!({"session":string,
                    "id":{"type":["integer","null"],"minimum":1},
                    "agent_id":{"type":["string","null"],"pattern":"^c-[0-9a-f]{32}$"},
                    "action":{"type":"string","minLength":1},
                    "argument":{"type":"string","maxLength":1024,"x-maxUtf8Bytes":1024,"default":""}}),
                json!(["session", "action"]),
            ),
            "simulation_set_enabled" => (
                json!({"session":string, "enabled":{"type":"boolean"}}),
                json!(["session", "enabled"]),
            ),
            "plugin_catalog" | "project_open" => (json!({"path": string}), json!(["path"])),
            _ => (json!({"session": string}), json!(["session"])),
        };
        let mut schema = json!({"type":"object", "properties":properties, "required":required, "additionalProperties":false});
        if name == "simulation_set_input" {
            schema["oneOf"] = json!([{"required":["value"]},{"required":["unknown"]}]);
        }
        if name == "simulation_plugin_action" {
            schema["oneOf"] = json!([
                {"required":["id"],"properties":{"id":{"type":"integer"}}},
                {"required":["agent_id"],"properties":{"agent_id":{"type":"string"}}}
            ]);
        }
        tools.push(json!({"name":name, "description":description, "inputSchema":schema, "annotations":effect.annotations()}));
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
            let version = if SUPPORTED.contains(&requested) {
                requested
            } else {
                REVISION
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
                if !TOOLS.iter().any(|(n, ..)| *n == name) {
                    return Err(Failure(
                        -32602,
                        Error::new("unknown_tool", format!("Unknown tool: {name}")),
                    ));
                }
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                if !arguments.is_object() {
                    return Err(Failure::invalid_params("Expected a tool arguments object."));
                }
                let outcome = Request::from_json(json!({"tool":name,"arguments":arguments}))
                    .and_then(|request| self.service.call(request));
                let is_error = outcome.is_err();
                let result = envelope(outcome);
                Ok(
                    json!({"content":[{"type":"text","text":result.to_string()}],
                    "structuredContent":result,"isError":is_error}),
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
pub fn run(service: Service, input: impl BufRead, mut out: impl Write) -> std::io::Result<()> {
    let mut connection = Connection {
        phase: Phase::New,
        service,
    };
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
