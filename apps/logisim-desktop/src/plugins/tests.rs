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
    let resolved = Resolution::new(&p, temp.path("moved"), vec![]);
    assert!(!resolved.missing());
    assert_eq!(output(&resolved, &p), 1);
    let mut normalized = p.clone();
    absolutize(&mut normalized, &temp.path("moved"));
    assert_eq!(
        output(
            &Resolution::new(&normalized, temp.path("moved"), vec![]),
            &p
        ),
        1
    );
    fs::remove_file(temp.path("moved/local.wasm")).unwrap();
    let resolved = Resolution::new(&p, temp.path("moved"), vec![]);
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
    let mut resolution = Resolution::new(&p, temp.0.clone(), vec![]);
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
        assert!(Resolution::new(&changed, temp.0.clone(), vec![]).missing());
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
        .save(&loaded, &destination, SaveMode::Selected(None))
        .unwrap();
    let disk = logisim_file::parse(&fs::read_to_string(&destination).unwrap())
        .unwrap()
        .document;
    assert_eq!(disk.plugin_sources[0].path, "../library/lib.wasm");
    assert!(!storage.dirty(&loaded));
    assert_eq!(
        output(&Resolution::new(&disk, temp.path("saved"), vec![]), &disk),
        1
    );
    let import = crate::library_import::load(&original, &Project::default()).unwrap();
    assert!(Path::new(&import.project.plugin_sources[0].path).is_absolute());
    assert!(!Resolution::new(&import.project, temp.path("saved"), vec![]).missing());
}
