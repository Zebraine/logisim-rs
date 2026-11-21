use super::*;
use crate::module_cache::{Outcome, load};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
};

struct Private(PathBuf);
impl Private {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("logisim-cache-test-{}-{name}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for Private {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn record(root: &Path) -> PathBuf {
    let files: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1);
    files.into_iter().next().unwrap()
}

#[test]
fn validated_module_is_reused_and_corruption_is_rebuilt_from_wasm() {
    let tmp = Private::new("restore");
    let root = tmp.0.join("cache");
    let bytes = wasm(
        &Response::Manifest(execution_manifest(AllowedEngines::Both)),
        4,
        "",
    );
    assert_eq!(load(&bytes, &root).unwrap().1, Outcome::Miss);
    let (plugin, status) = load(&bytes, &root).unwrap();
    assert_eq!(status, Outcome::Hit);
    assert!(plugin.check_dependency("wrong", "1.0", []).is_err());
    let entry = record(&root);
    let mut data = fs::read(&entry).unwrap();
    let last = data.len() - 1;
    data[last] ^= 1;
    fs::write(&entry, &data).unwrap();
    assert!(matches!(
        load(&bytes, &root).unwrap().1,
        Outcome::Rebuilt(_)
    ));
    fs::write(&entry, b"truncated").unwrap();
    assert!(matches!(
        load(&bytes, &root).unwrap().1,
        Outcome::Rebuilt(_)
    ));
    assert_eq!(load(&bytes, &root).unwrap().1, Outcome::Hit);
    let rejected = wasm(&Response::Manifest(manifest()), 4, "");
    let other = tmp.0.join("rejected");
    assert!(load(&rejected, &other).is_err());
    assert_eq!(fs::read_dir(other).unwrap().count(), 0);
}

#[test]
fn untrusted_paths_use_uncached_wasmtime_and_preserve_the_target() {
    let tmp = Private::new("paths");
    let bytes = wasm(
        &Response::Manifest(execution_manifest(AllowedEngines::Both)),
        4,
        "",
    );
    let outside = tmp.0.join("other");
    fs::create_dir(&outside).unwrap();
    let link = tmp.0.join("link");
    symlink(&outside, &link).unwrap();
    let root = tmp.0.join("cache");
    for bad in [
        link.join("cache"),
        link,
        tmp.0.join("../bad"),
        PathBuf::from("relative"),
    ] {
        let (plugin, status) = load(&bytes, &bad).unwrap();
        assert_eq!(plugin.engine(), WasmEngine::Wasmtime);
        assert!(matches!(status, Outcome::Bypassed(_)));
    }
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
    assert_eq!(load(&bytes, &root).unwrap().1, Outcome::Miss);
    let entry = record(&root);
    let sentinel = outside.join("sentinel");
    fs::write(&sentinel, b"untouched").unwrap();
    fs::remove_file(&entry).unwrap();
    symlink(&sentinel, &entry).unwrap();
    assert!(matches!(
        load(&bytes, &root).unwrap().1,
        Outcome::Bypassed(_)
    ));
    assert_eq!(fs::read(&sentinel).unwrap(), b"untouched");
    assert!(entry.is_symlink());
    for permissions in [0o755, 0o500] {
        fs::set_permissions(&root, fs::Permissions::from_mode(permissions)).unwrap();
        assert!(matches!(
            load(&bytes, &root).unwrap().1,
            Outcome::Bypassed(_)
        ));
    }
    fs::set_permissions(root, fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn stale_record_key_misses_and_concurrent_writers_publish_complete_records() {
    use sha2::{Digest, Sha256};
    use std::sync::Barrier;
    let tmp = Private::new("publication");
    let bytes = wasm(
        &Response::Manifest(execution_manifest(AllowedEngines::Both)),
        4,
        "",
    );
    let root = tmp.0.join("cache");
    let barrier = Barrier::new(2);
    std::thread::scope(|scope| {
        let open = || {
            barrier.wait();
            let (p, result) = load(&bytes, &root).unwrap();
            assert_eq!(p.manifest(), &execution_manifest(AllowedEngines::Both));
            assert!(matches!(result, Outcome::Miss | Outcome::Hit));
        };
        let first = scope.spawn(open);
        open();
        first.join().unwrap();
    });
    assert_eq!(load(&bytes, &root).unwrap().1, Outcome::Hit);
    // A correctly checksummed record for another cache key is an ordinary miss.
    let entry = record(&root);
    let mut record = fs::read(&entry).unwrap();
    let (sum, body) = record.split_at_mut(32);
    body[0] ^= 1;
    sum.copy_from_slice(&Sha256::digest(body));
    fs::write(&entry, record).unwrap();
    assert_eq!(load(&bytes, &root).unwrap().1, Outcome::Miss);
    assert_eq!(load(&bytes, &root).unwrap().1, Outcome::Hit);
}

#[test]
fn obsolete_upstream_namespace_rebuilds_once() {
    use sha2::{Digest, Sha256};
    let tmp = Private::new("obsolete");
    let bytes = wasm(
        &Response::Manifest(execution_manifest(AllowedEngines::Both)),
        4,
        "",
    );
    assert_eq!(load(&bytes, &tmp.0).unwrap().1, Outcome::Miss);
    let path = record(&tmp.0);
    let mut old = fs::read(&path).unwrap();
    let (sum, body) = old.split_at_mut(32);
    // Keep the valid artifact intact and model an old Wasmtime compiler namespace.
    let start = 32 + b"wasmtime-".len();
    assert_ne!(body[start], b'z');
    body[start] = b'z';
    sum.copy_from_slice(&Sha256::digest(body));
    fs::write(path, old).unwrap();
    assert!(matches!(
        load(&bytes, &tmp.0).unwrap().1,
        Outcome::Rebuilt(_)
    ));
    assert_eq!(load(&bytes, &tmp.0).unwrap().1, Outcome::Hit);
}

#[test]
fn scratch_cleanup_is_independent_of_background_cache_statistics() {
    let tmp = Private::new("cleanup");
    let bytes = wasm(
        &Response::Manifest(execution_manifest(AllowedEngines::Both)),
        4,
        "",
    );
    assert_eq!(load(&bytes, &tmp.0).unwrap().1, Outcome::Miss);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                for _ in 0..12 {
                    assert_eq!(load(&bytes, &tmp.0).unwrap().1, Outcome::Hit);
                }
            });
        }
    });
    assert!(record(&tmp.0).is_file());
}

#[test]
fn legacy_and_disallowed_modules_never_open_the_native_cache() {
    let tmp = Private::new("selection");
    let root = tmp.0.join("cache");
    for abi in 1..=3 {
        let response = Response::Manifest(Manifest { abi, ..manifest() });
        let (plugin, outcome) = load(&wasm(&response, abi, ""), &root).unwrap();
        assert_eq!(
            (plugin.engine(), outcome),
            (WasmEngine::Wasmi, Outcome::LegacyWasmi)
        );
        assert!(!root.exists());
    }
    let bytes = wasm(
        &Response::Manifest(execution_manifest(AllowedEngines::Wasmi)),
        4,
        "",
    );
    assert!(
        load(&bytes, &root)
            .err()
            .unwrap()
            .contains("does not permit")
    );
    assert!(!root.exists());
}
