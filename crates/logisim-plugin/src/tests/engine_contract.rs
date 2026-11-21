use super::*;
use crate::study::{Loader, Reuse};

#[test]
fn compiled_entry_requires_identical_bytes_and_rechecks_saved_definitions() {
    let m = manifest();
    let a = wasm(&Response::Manifest(m.clone()), 1, "");
    let b = wasm(&Response::Manifest(m.clone()), 1, "nop");
    let mut loader = Loader::new(Reuse::Module);
    for (bytes, hits, misses) in [(&a, 0, 1), (&a, 1, 1), (&b, 1, 2), (&a, 1, 3)] {
        let plugin = loader.load(bytes).unwrap();
        let definitions = m.components();
        plugin
            .check_dependency(&m.id, &m.version, &definitions)
            .unwrap();
        assert!(
            plugin
                .check_dependency(&m.id, "different", &definitions)
                .is_err()
        );
        let mut bad = definitions[0].clone();
        bad.tool.name.push('!');
        assert!(plugin.check_dependency(&m.id, &m.version, [&bad]).is_err());
        assert_eq!((loader.hits, loader.misses), (hits, misses));
    }
}

#[test]
fn engine_admission_and_fault_messages() {
    for engine in [WasmEngine::Wasmi, WasmEngine::Wasmtime]
        .into_iter()
        .filter(|e| e.available())
    {
        check_faults(engine);
    }
}

fn check_faults(engine: WasmEngine) {
    let m = Response::Manifest(execution_manifest(AllowedEngines::Both));
    let cases = [
        ("guest_trap", wasm(&m, 4, "unreachable")),
        ("memory_grow", wasm(&m, 4, "i32.const 256 memory.grow drop")),
        (
            "start_trap",
            wat::parse_str(r#"(module (func (export "logisim_abi4_both") (result i32) i32.const 4) (func $start unreachable) (start $start))"#).unwrap(),
        ),
        (
            "forbidden_import",
            wat::parse_str(r#"(module (import "env" "a" (func)) (func (export "logisim_abi4_both") (result i32) i32.const 4))"#).unwrap(),
        ),
        ("missing_abi", wat::parse_str("(module)").unwrap()),
        (
            "invalid_export_type",
            wat::parse_str(r#"(module (func (export "logisim_abi4_both") (result i64) i64.const 1))"#)
                .unwrap(),
        ),
        ("module_bytes", vec![0; MAX_MODULE + 1]),
        (
            "json_response_bytes",
            wasm(
                &m,
                4,
                &format!("i64.const {} return", ((MAX_MESSAGE + 1) as u64) << 32),
            ),
        ),
    ];
    for (name, bytes) in cases {
        let e = Plugin::load_with_engine(&bytes, engine).err().expect(name);
        assert!(!e.is_empty());
        println!(
            "LIMIT {}",
            serde_json::json!({"engine":engine.code(),"case":name,"error":e})
        );
    }
    let vm = Plugin::load_with_engine(&wasm(&m, 4, ""), engine).unwrap();
    let c = vm.manifest.components().remove(0);
    let mut instance = vm.instantiate(&c).unwrap();
    let request = Request::Evaluate(Evaluation {
        tool: "x".repeat(MAX_MESSAGE),
        properties: c.properties,
        inputs: vec![],
        state: vec![],
        event: Event::Reset,
    });
    assert_eq!(
        instance.call(&request).unwrap_err(),
        "Plugin request exceeds message limit."
    );
}

#[cfg(all(target_os = "linux", feature = "compare-wasmtime"))]
#[test]
fn trusted_cache_recovers_from_misses_changed_compiler_and_unavailable_storage() {
    use std::{
        fs,
        os::unix::fs::{DirBuilderExt, PermissionsExt},
        path::{Path, PathBuf},
        time::SystemTime,
    };
    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("logisim-stage2-{}-{stamp}", std::process::id()));
    fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
    let _owned = Directory(path.clone());
    let bytes = wasm(
        &Response::Manifest(execution_manifest(AllowedEngines::Both)),
        4,
        "",
    );
    let reopen = |dir: &Path| {
        let mut loader = Loader::new(Reuse::Disk(dir.into()));
        assert_eq!(loader.load(&bytes).unwrap().manifest().id, manifest().id);
        (loader.hits, loader.misses, loader.cache_error().is_some())
    };
    assert_eq!(reopen(&path), (0, 1, false));
    assert_eq!(reopen(&path), (1, 0, false));

    // Change the compiler flags for identical Wasm. Keep the same directory and engine version.
    let mut options = wasmtime::CacheConfig::new();
    options.with_directory(&path);
    let cache = wasmtime::Cache::new(options).unwrap();
    let mut cfg = backend::wasmtime_config();
    cfg.cache(Some(cache.clone()))
        .cranelift_opt_level(wasmtime::OptLevel::None);
    let engine = Engine::Wasmtime(wasmtime::Engine::new(&cfg).unwrap());
    engine.compile(&bytes).unwrap();
    assert_eq!((cache.cache_hits(), cache.cache_misses()), (0, 1));
    engine.compile(&bytes).unwrap();
    assert_eq!(cache.cache_hits(), 1);

    // A broken compression envelope must cause recompilation from the original Wasm.
    let versions = fs::read_dir(path.join("modules")).unwrap();
    for version in versions {
        for file in fs::read_dir(version.unwrap().path()).unwrap() {
            let file = file.unwrap().path();
            if file.is_file() && file.extension().is_none() {
                fs::write(file, b"incomplete compressed cache").unwrap();
            }
        }
    }
    assert_eq!(reopen(&path), (0, 1, false));
    assert_eq!(reopen(&path), (1, 0, false));

    let blocked = path.join("not-a-directory");
    fs::write(&blocked, b"original").unwrap();
    assert_eq!(reopen(&blocked), (0, 1, true));
    assert_eq!(fs::read(blocked).unwrap(), b"original");
    let public = path.join("public");
    fs::create_dir(&public).unwrap();
    fs::set_permissions(&public, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(reopen(&public), (0, 1, true));
    assert_eq!(fs::read_dir(&public).unwrap().count(), 0);

    let read_only = path.join("read-only");
    fs::DirBuilder::new()
        .mode(0o500)
        .create(&read_only)
        .unwrap();
    // Privileged CI users can write through mode bits. Check actual filesystem behavior.
    if fs::write(read_only.join("probe"), b"").is_err() {
        for _ in 0..2 {
            let (hits, misses, _) = reopen(&read_only);
            assert_eq!((hits, misses), (0, 1));
        }
        assert_eq!(fs::read_dir(&read_only).unwrap().count(), 0);
    }
    fs::set_permissions(&read_only, fs::Permissions::from_mode(0o700)).unwrap();

    let racing = path.join("writers");
    fs::DirBuilder::new().mode(0o700).create(&racing).unwrap();
    std::thread::scope(|scope| {
        let a = scope.spawn(|| reopen(&racing));
        let b = scope.spawn(|| reopen(&racing));
        for worker in [a, b] {
            assert!(!worker.join().unwrap().2);
        }
    });
    assert_eq!(reopen(&racing), (1, 0, false));
}
