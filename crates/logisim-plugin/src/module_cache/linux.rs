use super::Outcome;
use crate::{Plugin, backend};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    hash::{Hash, Hasher},
    io::{self, ErrorKind, Read, Write},
    os::unix::fs::{DirBuilderExt, DirEntryExt, MetadataExt, OpenOptionsExt},
    path::{Component, Path, PathBuf},
};

// At most 32 published records. A collision only evicts a previous entry.
// Temporary directories are exclusive to one open; concurrent writers publish by rename.
const SLOTS: u8 = 32;
const MAX_RECORD: usize = 16 * 1024 * 1024;
const DOMAIN: &[u8] = b"logisim-rs/wasmtime-cache/v1/36.0.20\0";

fn invalid(reason: &str) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, reason)
}

struct Fingerprint(Sha256);

impl Hasher for Fingerprint {
    fn write(&mut self, data: &[u8]) {
        self.0.update(data);
    }
    fn finish(&self) -> u64 {
        u64::from_be_bytes(self.0.clone().finalize()[..8].try_into().unwrap())
    }
}

fn key(engine: &wasmtime::Engine, wasm: &[u8]) -> [u8; 32] {
    let mut hash = Fingerprint(Sha256::new_with_prefix(DOMAIN));
    engine.precompile_compatibility_hash().hash(&mut hash);
    hash.write(&crate::FUEL.to_le_bytes());
    hash.write(&Sha256::digest(wasm));
    hash.0.finalize().into()
}

fn private_root(path: &Path, uid: u32) -> io::Result<()> {
    if !path.is_absolute()
        || path.parent().is_none()
        || path.components().any(|c| c == Component::ParentDir)
    {
        return Err(invalid(
            "Expected an absolute private cache path without traversal",
        ));
    }
    let system_owner = fs::metadata("/")?.uid();
    let mut part = PathBuf::new();
    for component in path.components() {
        part.push(component);
        let meta = match fs::symlink_metadata(&part) {
            Err(e) if e.kind() == ErrorKind::NotFound => {
                match DirBuilder::new().mode(0o700).create(&part) {
                    Err(e) if e.kind() != ErrorKind::AlreadyExists => return Err(e),
                    _ => (),
                }
                fs::symlink_metadata(&part)?
            }
            result => result?,
        };
        // The sticky system /tmp can host a private experiment directory even inside
        // a user namespace where the mount owner has an unmapped uid.
        let system_tmp =
            part == Path::new("/tmp") && meta.uid() == system_owner && meta.mode() & 0o1000 != 0;
        let owned = meta.uid() == uid || meta.uid() == 0 || part == Path::new("/");
        if !meta.is_dir()
            || !(system_tmp || owned && meta.mode() & 0o022 == 0)
            || (part == path && (meta.uid() != uid || meta.mode() & 0o777 != 0o700))
        {
            return Err(invalid(&format!(
                "Untrusted cache directory or permissions: {}",
                part.display()
            )));
        }
    }
    Ok(())
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        // Wasmtime's maintenance thread can still be writing stats by absolute path.
        // Detach its namespace before walking the directory for deletion. The worker
        // never recreates missing directories, and this random name is never reused.
        let retired = self.0.with_extension("retired");
        if let Err(e) = fs::rename(&self.0, &retired).and_then(|()| fs::remove_dir_all(&retired)) {
            eprintln!(
                "Could not remove temporary Wasmtime cache {}: {e}",
                self.0.display()
            );
        }
    }
}

pub(super) struct Cache {
    scratch: Scratch,
    entry: PathBuf,
    key: [u8; 32],
    uid: u32,
    cache: wasmtime::Cache,
    engine: backend::Engine,
}

// Pass only a newly created private directory to Wasmtime. Its asynchronous housekeeping
// and unverified on-disk format never have access to the directory of published records.
pub(super) fn prepare(root: &Path, wasm: &[u8]) -> io::Result<Cache> {
    let uid = fs::metadata("/proc/self")?.uid();
    private_root(root, uid)?;
    let mut random = [0; 16];
    File::open("/dev/urandom")?.read_exact(&mut random)?;
    let stage = root.join(format!(".open-{:032x}", u128::from_be_bytes(random)));
    DirBuilder::new().mode(0o700).create(&stage)?;
    let scratch = Scratch(stage);
    let mut options = wasmtime::CacheConfig::new();
    options.with_directory(&scratch.0);
    let cache = wasmtime::Cache::new(options).map_err(io::Error::other)?;
    let mut config = backend::wasmtime_config();
    config.cache(Some(cache.clone()));
    let engine = wasmtime::Engine::new(&config).map_err(io::Error::other)?;
    let key = key(&engine, wasm);
    Ok(Cache {
        entry: root.join(format!("slot-{:02x}", key[0] % SLOTS)),
        scratch,
        key,
        uid,
        cache,
        engine: backend::Engine::Wasmtime(engine),
    })
}

// Only two upstream cache path components enter a record; neither can name another path.
fn artifact_names(bytes: &[u8]) -> io::Result<(&str, &str)> {
    let names = std::str::from_utf8(bytes)
        .ok()
        .and_then(|s| s.split_once('/'));
    let Some((dir, file)) = names else {
        return Err(invalid("Invalid cache artifact path"));
    };
    let valid = |c: u8| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_');
    if !dir.starts_with("wasmtime-")
        || dir.len() > 100
        || !dir.bytes().all(|c| valid(c) || c == b'.')
        || file.len() != 43
        || !file.bytes().all(valid)
    {
        return Err(invalid("Invalid Wasmtime cache artifact name"));
    }
    Ok((dir, file))
}

fn limited(file: File) -> io::Result<Vec<u8>> {
    if file.metadata()?.len() > MAX_RECORD as u64 {
        return Err(invalid("Oversized cache record"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_RECORD as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_RECORD {
        return Err(invalid("Oversized cache record"));
    }
    Ok(bytes)
}

impl Cache {
    fn restore(&self) -> io::Result<Option<File>> {
        let record = match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&self.entry)
        {
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            result => result?,
        };
        let meta = record.metadata()?;
        if !meta.is_file()
            || meta.uid() != self.uid
            || meta.nlink() != 1
            || meta.mode() & 0o777 != 0o600
        {
            return Err(io::Error::new(
                ErrorKind::PermissionDenied,
                "Untrusted cache entry",
            ));
        }
        let bytes = limited(record)?;
        let Some((digest, body)) = bytes.split_at_checked(32) else {
            return Err(invalid("Truncated cache record"));
        };
        if Sha256::digest(body)[..] != *digest {
            return Err(invalid("Cache record checksum mismatch"));
        }
        let Some((key, artifact)) = body.split_at_checked(32) else {
            return Err(invalid("Truncated cache key"));
        };
        if key != self.key {
            return Ok(None);
        }
        let split = artifact
            .iter()
            .take(150)
            .position(|&b| b == b'\n')
            .ok_or_else(|| invalid("Invalid cache record header"))?;
        let (dir, file) = artifact_names(&artifact[..split])?;
        let dir = self.scratch.0.join("modules").join(dir);
        fs::create_dir_all(&dir)?;
        let mut restored = File::create_new(dir.join(file))?;
        restored.write_all(&artifact[split + 1..])?;
        Ok(Some(restored))
    }

    fn publish(&self, restored: Option<&File>) -> io::Result<()> {
        if self.cache.cache_misses() == 0 {
            return Err(invalid("Wasmtime could not persist the compiled artifact"));
        }
        // The upstream cache may select a new compiler directory (e.g. a debug rebuild).
        // Only publish its fresh atomic replacement, never the input that just missed.
        let previous_inode = restored.map(File::metadata).transpose()?.map(|m| m.ino());
        let modules = self.scratch.0.join("modules");
        for directory in fs::read_dir(modules)? {
            let directory = directory?;
            if !directory.file_type()?.is_dir() {
                continue;
            }
            for file in fs::read_dir(directory.path())? {
                let file = file?;
                let names = format!(
                    "{}/{}",
                    directory.file_name().to_string_lossy(),
                    file.file_name().to_string_lossy()
                );
                if Some(file.ino()) == previous_inode
                    || !file.file_type()?.is_file()
                    || artifact_names(names.as_bytes()).is_err()
                {
                    continue;
                }
                let payload = limited(File::open(file.path())?)?;
                if payload.len() + 65 + names.len() > MAX_RECORD {
                    return Err(invalid("Compiled artifact exceeds slot budget"));
                }
                let mut digest = Sha256::new();
                digest.update(self.key);
                digest.update(names.as_bytes());
                digest.update(b"\n");
                digest.update(&payload);
                let tmp = self.scratch.0.join("publish");
                let mut f = OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .mode(0o600)
                    .open(&tmp)?;
                f.write_all(&digest.finalize())?;
                f.write_all(&self.key)?;
                writeln!(f, "{names}")?;
                f.write_all(&payload)?;
                return fs::rename(tmp, &self.entry);
            }
        }
        Err(invalid("Wasmtime did not produce a cacheable artifact"))
    }

    pub(super) fn load(
        self,
        wasm: &[u8],
        declaration: crate::Declaration,
    ) -> crate::Result<(Plugin, Outcome)> {
        let restored = self.restore();
        // On an I/O/permission error abandon the scratch tree, including any partial copy.
        if let Err(e) = &restored
            && e.kind() != ErrorKind::InvalidData
        {
            return Ok((
                Plugin::load_with_engine(wasm, crate::WasmEngine::Wasmtime)?,
                Outcome::Bypassed(e.to_string()),
            ));
        }
        let plugin = Plugin::from_module(self.engine.compile(wasm)?, declaration)?;
        let status = if self.cache.cache_hits() == 1 {
            Outcome::Hit
        } else if let Err(e) = self.publish(restored.as_ref().ok().and_then(Option::as_ref)) {
            Outcome::Bypassed(e.to_string())
        } else {
            match restored {
                Err(e) => Outcome::Rebuilt(e.to_string()),
                Ok(Some(_)) => Outcome::Rebuilt("Wasmtime rejected the stored artifact".into()),
                Ok(None) => Outcome::Miss,
            }
        };
        Ok((plugin, status))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn key_changes_with_source_fuel_and_compiler_options() {
        let mut config = backend::wasmtime_config();
        let initial = wasmtime::Engine::new(&config).unwrap();
        let id = key(&initial, b"module one");
        assert_ne!(id, key(&initial, b"module two"));
        assert_eq!(
            id,
            key(&wasmtime::Engine::new(&config).unwrap(), b"module one")
        );
        config.consume_fuel(false);
        assert_ne!(
            id,
            key(&wasmtime::Engine::new(&config).unwrap(), b"module one")
        );
        config
            .consume_fuel(true)
            .cranelift_opt_level(wasmtime::OptLevel::None);
        assert_ne!(
            id,
            key(&wasmtime::Engine::new(&config).unwrap(), b"module one")
        );
    }
}
