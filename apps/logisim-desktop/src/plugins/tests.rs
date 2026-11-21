use super::*;
use crate::{
    storage::{SaveMode, Storage},
    tests::support::Temp,
};
use std::fs;

use crate::tests::support::plugins::fixture;

fn output(resolution: &Resolution, p: &Project) -> u32 {
    let mut sim = logisim_sim::Simulator::default();
    sim.set_plugins(resolution.registry());
    sim.synchronize(&p.main_circuit().circuit);
    sim.advance(32);
    sim.port_value(p.main_circuit().circuit.components[0].id, 0)
        .unwrap()
        .number()
        .unwrap()
}
#[test]
fn relative_path_wins_after_bundle_moves_and_absolute_fallback_works() {
    let temp = Temp::new();
    let (mut p, one) = fixture("test.paths", 1);
    let (_, zero) = fixture("test.paths", 0);
    fs::create_dir(temp.path("moved")).unwrap();
    fs::write(temp.path("moved/local.wasm"), one).unwrap();
    fs::write(temp.path("absolute.wasm"), zero).unwrap();
    p.plugin_sources[0].path = "local.wasm".into();
    p.plugin_sources[0].fallback = temp.path("absolute.wasm").to_string_lossy().into_owned();
    let resolved = Resolution::new(WasmEngine::Wasmi, &p, temp.path("moved"), None, vec![]);
    assert!(!resolved.missing());
    assert_eq!(output(&resolved, &p), 1);
    let mut normalized = p.clone();
    absolutize(&mut normalized, &temp.path("moved"));
    assert_eq!(
        output(
            &Resolution::new(
                WasmEngine::Wasmi,
                &normalized,
                temp.path("moved"),
                None,
                vec![]
            ),
            &p
        ),
        1
    );
    fs::remove_file(temp.path("moved/local.wasm")).unwrap();
    let resolved = Resolution::new(WasmEngine::Wasmi, &p, temp.path("moved"), None, vec![]);
    assert!(!resolved.missing());
    assert_eq!(output(&resolved, &p), 0);
}
#[test]
fn explicit_invalid_location_does_not_reuse_old_module_or_fallback() {
    let temp = Temp::new();
    let (mut p, good) = fixture("test.paths", 1);
    let (_, wrong) = fixture("test.wrong", 1);
    fs::write(temp.path("valid.wasm"), good).unwrap();
    fs::write(temp.path("wrong.wasm"), wrong).unwrap();
    fs::write(temp.path("broken.wasm"), b"not wasm").unwrap();
    p.plugin_sources[0].fallback = temp.path("valid.wasm").to_string_lossy().into_owned();
    let mut resolution = Resolution::new(WasmEngine::Wasmi, &p, temp.0.clone(), None, vec![]);
    assert!(!resolution.missing());
    for path in ["wrong.wasm", "broken.wasm", "absent.wasm"] {
        resolution.rows[0].location = path.into();
        resolution.check(0);
        assert!(resolution.missing());
        assert!(
            resolution
                .registry()
                .instantiate(
                    p.main_circuit().circuit.components[0]
                        .plugin
                        .as_ref()
                        .unwrap()
                )
                .is_err()
        );
        assert_eq!(resolution.sources(&p.plugin_sources), p.plugin_sources);
    }
    resolution.rows[0].location = "valid.wasm".into();
    resolution.check(0);
    assert!(!resolution.missing());
    assert_eq!(output(&resolution, &p), 1);
    assert_eq!(
        resolution.sources(&p.plugin_sources)[0].path,
        temp.path("valid.wasm").to_string_lossy()
    );
    for mismatch in ["version", "definition"] {
        let mut changed = p.clone();
        if mismatch == "version" {
            changed.plugin_sources[0].version = "2.0".into();
            changed.main_circuit_mut().circuit.components[0]
                .plugin
                .as_mut()
                .unwrap()
                .version = "2.0".into();
        } else {
            changed.main_circuit_mut().circuit.components[0]
                .plugin
                .as_mut()
                .unwrap()
                .tool
                .bounds[1][0] = 30;
        }
        assert!(
            Resolution::new(WasmEngine::Wasmi, &changed, temp.0.clone(), None, vec![]).missing()
        );
    }
}
#[test]
fn save_as_and_library_import_preserve_dependency_bases_without_dirtying_open() {
    let temp = Temp::new();
    let (mut p, bytes) = fixture("test.paths", 1);
    fs::create_dir(temp.path("library")).unwrap();
    fs::create_dir(temp.path("saved")).unwrap();
    fs::write(temp.path("library/lib.wasm"), bytes).unwrap();
    p.plugin_sources[0].path = "lib.wasm".into();
    let original = temp.path("library/source.circ");
    fs::write(&original, logisim_file::serialize(&p).unwrap()).unwrap();
    let mut storage = Storage::new();
    let (loaded, warnings) = storage.load(&original).unwrap();
    assert!(warnings.is_empty());
    assert!(!storage.dirty(&loaded));
    assert!(Path::new(&loaded.plugin_sources[0].path).is_absolute());
    let destination = temp.path("saved/new.circ");
    storage
        .save(&loaded, &destination, SaveMode::Selected(None), false)
        .unwrap();
    let disk = logisim_file::parse(&fs::read_to_string(&destination).unwrap())
        .unwrap()
        .document;
    assert_eq!(disk.plugin_sources[0].path, "../library/lib.wasm");
    assert!(!storage.dirty(&loaded));
    assert_eq!(
        output(
            &Resolution::new(WasmEngine::Wasmi, &disk, temp.path("saved"), None, vec![]),
            &disk
        ),
        1
    );
    let import = crate::library_import::load(&original, &Project::default()).unwrap();
    assert!(Path::new(&import.project.plugin_sources[0].path).is_absolute());
    assert!(
        !Resolution::new(
            WasmEngine::Wasmi,
            &import.project,
            temp.path("saved"),
            None,
            vec![]
        )
        .missing()
    );
}

#[test]
fn host_directory_is_last_single_basename_and_checks_the_module() {
    let temp = Temp::new();
    let (mut project, good) = fixture("test.paths", 1);
    let (_, wrong) = fixture("test.wrong", 1);
    let (_, zero) = fixture("test.paths", 0);
    let dir = temp.path("plugins");
    fs::create_dir_all(dir.join("nested")).unwrap();
    fs::write(dir.join("nested/mod.wasm"), &good).unwrap();
    let path = temp.path("mod.wasm");
    let fallback = temp.path("other.wasm");
    project.plugin_sources[0].path = "mod.wasm".into();
    project.plugin_sources[0].fallback = fallback.to_str().unwrap().into();
    let resolve =
        |p: &Project| Resolution::new(WasmEngine::Wasmi, p, temp.0.clone(), Some(&dir), vec![]);
    assert!(resolve(&project).missing());
    let candidate = dir.join("mod.wasm");
    fs::write(&candidate, wrong).unwrap();
    let bad = resolve(&project);
    let error = bad.rows[0].error.as_ref().unwrap();
    assert!(error.contains(candidate.to_str().unwrap()) && error.contains("identity"));
    assert_eq!(bad.sources(&project.plugin_sources), project.plugin_sources);

    fs::write(&candidate, &good).unwrap();
    let mut resolved = resolve(&project);
    assert!(resolved.rows[0].from_directory);
    assert_eq!(output(&resolved, &project), 1);
    resolved.rows[0].location = "bad.wasm".into();
    resolved.check(0);
    assert!(resolved.missing() && !resolved.rows[0].from_directory);
    fs::write(&fallback, &zero).unwrap();
    let resolved = resolve(&project);
    assert!(!resolved.rows[0].from_directory);
    assert_eq!(
        resolved.rows[0].found,
        Some(fallback.canonicalize().unwrap())
    );
    fs::write(&path, &good).unwrap();
    assert_eq!(
        resolve(&project).rows[0].found,
        Some(path.canonicalize().unwrap())
    );
    fs::remove_file(fallback).unwrap();
    project.plugin_sources[0].path.clear();
    project.plugin_sources[0].fallback = "absent/mod.wasm".into();
    assert!(resolve(&project).rows[0].from_directory);
    project.plugin_sources[0].path = "unusable-name".into();
    assert!(resolve(&project).rows[0].from_directory);
    project.plugin_sources[0].path.clear();
    project.plugin_sources[0].fallback.clear();
    assert!(resolve(&project).missing());
    project.plugin_sources[0].fallback = "missing/not-a-module.txt".into();
    fs::write(dir.join("not-a-module.txt"), good).unwrap();
    assert!(resolve(&project).missing());
}

#[test]
fn inaccessible_directory_candidate_does_not_block_other_sources() {
    let temp = Temp::new();
    let (mut project, wasm) = fixture("test.a", 1);
    let (other, _) = fixture("test.b", 0);
    project.plugin_sources[0].path = "working.wasm".into();
    project.plugin_sources.extend(other.plugin_sources);
    fs::write(temp.path("working.wasm"), wasm).unwrap();
    let regular_file = temp.path("not-a-directory");
    fs::write(&regular_file, []).unwrap();
    for dir in [temp.path("missing-directory"), regular_file] {
        let resolution = Resolution::new(
            WasmEngine::Wasmi,
            &project,
            temp.0.clone(),
            Some(&dir),
            vec![],
        );
        assert!(resolution.rows[0].loaded.is_some());
        assert!(resolution.rows[1].loaded.is_none());
        assert!(
            resolution.rows[1]
                .error
                .as_ref()
                .unwrap()
                .contains(dir.to_str().unwrap())
        );
    }
}

#[cfg(unix)]
#[test]
fn unreadable_same_name_module_reports_its_path() {
    use std::os::unix::fs::PermissionsExt;
    let temp = Temp::new();
    let (project, wasm) = fixture("test.permissions", 1);
    let dir = temp.path("modules");
    fs::create_dir(&dir).unwrap();
    let candidate = dir.join("missing.wasm");
    fs::write(&candidate, wasm).unwrap();
    fs::set_permissions(&candidate, fs::Permissions::from_mode(0o000)).unwrap();
    // Privileged test runners can still read files with mode 000.
    if fs::File::open(&candidate).is_err_and(|e| e.kind() == std::io::ErrorKind::PermissionDenied) {
        let result = Resolution::new(
            WasmEngine::Wasmi,
            &project,
            temp.0.clone(),
            Some(&dir),
            vec![],
        );
        assert!(result.missing());
        assert!(
            result.rows[0]
                .error
                .as_ref()
                .unwrap()
                .contains(candidate.to_str().unwrap())
        );
        assert_eq!(
            result.sources(&project.plugin_sources),
            project.plugin_sources
        );
    }
}
