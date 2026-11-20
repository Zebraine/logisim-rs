mod mcp;
use logisim_agent::{Error, Patch, Request, Result, Service, envelope, io};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::ExitCode,
};

fn parse(args: &[OsString]) -> Result<Request> {
    let [group, command, options @ ..] = args else {
        return Err(Error::invalid("Expected a group and command."));
    };
    let mut flags = std::collections::BTreeMap::new();
    let mut mode = None;
    let mut iter = options.iter();
    while let Some(flag) = iter.next() {
        match flag.to_str() {
            Some(s @ ("--dry-run" | "--apply")) if mode.is_none() => mode = Some(s == "--dry-run"),
            Some(s @ ("--config" | "--path" | "--ops" | "--session")) if !flags.contains_key(s) => {
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
            let config = PathBuf::from(take("--config")?);
            let path = Path::new(take("--ops")?);
            let dry_run = mode
                .take()
                .ok_or_else(|| Error::invalid("Specify --dry-run or --apply."))?;
            let patch = Patch::from_bytes(&io::read(path)?)?;
            Request::SettingsPatch {
                config,
                dry_run,
                ops: patch.ops,
                expected_sha256: patch.expected_sha256,
            }
        }
        (Some("project"), Some("open")) => Request::ProjectOpen {
            path: take("--path")?.into(),
        },
        (Some(group), Some(action)) => {
            let session = take("--session")?
                .to_str()
                .ok_or_else(|| Error::invalid("Invalid session ID."))?
                .to_owned();
            match (group, action) {
                ("project", "snapshot") => Request::ProjectSnapshot { session },
                ("project", "close") => Request::ProjectClose { session },
                ("simulation", "open") => Request::SimulationOpen { session },
                ("simulation", "state") => Request::SimulationState { session },
                _ => return Err(Error::invalid("Unknown command.")),
            }
        }
        _ => return Err(Error::invalid("Invalid command.")),
    };
    if !flags.is_empty() || mode.is_some() {
        return Err(Error::invalid("Unexpected option for this command."));
    }
    Ok(req)
}

fn run() -> std::io::Result<bool> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args == [OsString::from("mcp")] {
        mcp::run(std::io::stdin().lock(), std::io::stdout().lock())?;
        return Ok(true);
    }
    let mut service = Service::default();
    let mut out = std::io::stdout().lock();
    if args != [OsString::from("cli")] {
        let req = parse(&args);
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
            .and_then(Request::from_json)
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
