use logisim_agent::mcp;
use logisim_agent::{Error, Patch, ProjectOp, Request, Result, Service, envelope, io};
use std::{ffi::OsString, path::Path, process::ExitCode};

fn simulation_request(command: &std::ffi::OsStr, options: &[OsString]) -> Result<Request> {
    let tool = match command.to_str() {
        Some("open") => "simulation_open",
        Some("state") => "simulation_state",
        Some("set-input") => "simulation_set_input",
        Some("plugin-action") => "simulation_plugin_action",
        Some("settle") => "simulation_settle",
        Some("step") => "simulation_step",
        Some("tick") => "simulation_tick",
        Some("reset") => "simulation_reset",
        Some("enable") => "simulation_set_enabled",
        _ => return Err(Error::invalid("Unknown simulation command.")),
    };
    let mut arguments = serde_json::Map::new();
    let mut words = options.iter();
    while let Some(option) = words.next() {
        let key = option
            .to_str()
            .and_then(|s| s.strip_prefix("--"))
            .ok_or_else(|| Error::invalid("Expected a named simulation option."))?;
        let field = if key == "agent-id" { "agent_id" } else { key };
        if arguments.contains_key(field) {
            return Err(Error::invalid(format!("Duplicate --{key}.")));
        }
        let value = if key == "unknown" {
            true.into()
        } else {
            let word = words
                .next()
                .and_then(|w| w.to_str())
                .filter(|s| key == "argument" || (!s.is_empty() && !s.starts_with("--")))
                .ok_or_else(|| Error::invalid(format!("Missing value for --{key}.")))?;
            match key {
                "id" | "value" => word.parse::<u64>().map_err(Error::invalid)?.into(),
                "enabled" => word.parse::<bool>().map_err(Error::invalid)?.into(),
                "session" | "mode" | "agent-id" | "action" | "argument" => word.into(),
                _ => return Err(Error::invalid(format!("Unknown --{key}."))),
            }
        };
        arguments.insert(field.into(), value);
    }
    Request::from_json(serde_json::json!({"tool":tool, "arguments":arguments}))
}

fn parse(args: &[OsString], service: &Service) -> Result<Request> {
    let [group, command, options @ ..] = args else {
        return Err(Error::invalid("Expected a group and command."));
    };
    if group == "simulation" {
        return simulation_request(command, options);
    }
    let mut flags = std::collections::BTreeMap::new();
    let mut mode = None;
    let mut no_backup = false;
    let mut iter = options.iter();
    while let Some(flag) = iter.next() {
        match flag.to_str() {
            Some(s @ ("--dry-run" | "--apply")) if mode.is_none() => mode = Some(s == "--dry-run"),
            Some("--no-backup") if !no_backup => no_backup = true,
            Some(
                s
                @ ("--config" | "--wasm" | "--path" | "--ops" | "--session" | "--expected-sha256"),
            ) if !flags.contains_key(s) => {
                let value = iter
                    .next()
                    .filter(|v| !v.is_empty())
                    .ok_or_else(|| Error::invalid(format!("Missing value for {s}.")))?;
                flags.insert(s, value);
            }
            _ => return Err(Error::invalid("Unknown or duplicate option.")),
        }
    }
    let mut take = |name| {
        flags
            .remove(name)
            .ok_or_else(|| Error::invalid(format!("Missing {name}.")))
    };
    let req = match (group.to_str(), command.to_str()) {
        (Some("settings"), Some("catalog")) => Request::SettingsCatalog {},
        (Some("settings"), Some("get")) => Request::SettingsGet {
            config: take("--config")?.into(),
        },
        (Some("settings"), Some("patch")) => {
            let config = service.resolve_path(Path::new(take("--config")?))?;
            let path = service.resolve_path(Path::new(take("--ops")?))?;
            let dry_run = mode
                .take()
                .ok_or_else(|| Error::invalid("Specify --dry-run or --apply."))?;
            let patch = Patch::from_bytes(&io::read(&path)?)?;
            Request::SettingsPatch {
                config,
                dry_run,
                ops: patch.ops,
                expected_sha256: patch.expected_sha256,
            }
        }
        (Some("plugin"), Some("catalog")) => Request::PluginCatalog {
            path: take("--wasm")?.into(),
        },
        (Some("project"), Some("catalog")) => Request::ProjectCatalog {},
        (Some("project"), Some("open")) => Request::ProjectOpen {
            path: take("--path")?.into(),
        },
        (Some(group), Some(action)) => {
            let session = take("--session")?
                .to_str()
                .ok_or_else(|| Error::invalid("Invalid session ID."))?
                .to_owned();
            match (group, action) {
                ("project", "patch") => {
                    let path = service.resolve_path(Path::new(take("--ops")?))?;
                    let dry_run = mode
                        .take()
                        .ok_or_else(|| Error::invalid("Specify --dry-run or --apply."))?;
                    let edits = Patch::<ProjectOp>::from_bytes(&io::read(&path)?)?;
                    Request::ProjectPatch {
                        session,
                        dry_run,
                        expected_sha256: edits.expected_sha256,
                        ops: edits.ops,
                    }
                }
                ("project", "save") => {
                    let expected_sha256 = flags
                        .remove("--expected-sha256")
                        .map(|v| {
                            v.to_str()
                                .map(str::to_owned)
                                .ok_or_else(|| Error::invalid("Invalid SHA-256."))
                        })
                        .transpose()?;
                    Request::ProjectSave {
                        session,
                        expected_sha256,
                        backup: !std::mem::take(&mut no_backup),
                    }
                }
                ("project", "snapshot") => Request::ProjectSnapshot { session },
                ("project", "close") => Request::ProjectClose { session },
                _ => return Err(Error::invalid("Unknown command.")),
            }
        }
        _ => return Err(Error::invalid("Invalid command.")),
    };
    if !flags.is_empty() || mode.is_some() || no_backup {
        return Err(Error::invalid("Unexpected option for this command."));
    }
    Ok(req)
}

fn startup(args: &[OsString]) -> Result<(Service, &[OsString])> {
    if args.first().is_some_and(|word| word == "--root") {
        let [_, directory, rest @ ..] = args else {
            return Err(Error::invalid("--root requires an existing directory."));
        };
        Ok((Service::with_root(Path::new(directory))?, rest))
    } else {
        Ok((Service::default(), args))
    }
}

fn run() -> std::io::Result<bool> {
    let words: Vec<_> = std::env::args_os().skip(1).collect();
    let mut out = std::io::stdout().lock();
    let (mut service, args) = match startup(&words) {
        Ok(startup) => startup,
        Err(e) => {
            io::write(&mut out, &envelope(Err(e)))?;
            return Ok(false);
        }
    };
    if args == [OsString::from("mcp")] {
        mcp::run(service, std::io::stdin().lock(), out)?;
        return Ok(true);
    }
    if args != [OsString::from("cli")] {
        let req = parse(args, &service);
        let stays_open = matches!(req, Ok(Request::ProjectOpen { .. }));
        let result = req.and_then(|r| service.call(r));
        let ok = result.is_ok();
        io::write(&mut out, &envelope(result))?;
        if !stays_open || !ok {
            return Ok(ok);
        }
    }
    io::lines(std::io::stdin().lock(), |line| {
        let result = line
            .and_then(|value| {
                if value.is_array() {
                    let words: Vec<String> =
                        serde_json::from_value(value).map_err(Error::invalid)?;
                    let args: Vec<_> = words.into_iter().map(OsString::from).collect();
                    parse(&args, &service)
                } else {
                    Request::from_json(value)
                }
            })
            .and_then(|r| service.call(r));
        io::write(&mut out, &envelope(result))
    })?;
    Ok(true)
}
fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
