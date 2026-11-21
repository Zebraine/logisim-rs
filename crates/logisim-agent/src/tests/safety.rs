//! Exercise the same service used by both transports against disposable files.
use super::{Temp, open};
use crate::{Operation, Request, Service};
use serde_json::json;
use std::{collections::BTreeSet, fs, path::Path};

const CIRCUIT: &str = r#"<project version="1.0" source="logisim-rs-plugin-v1"><main name="main"/><circuit name="main"/></project>"#;
const PREFS: &[u8] = br#"{"version":16}"#;

fn patch_settings(path: &Path, dry_run: bool) -> Request {
    Request::SettingsPatch {
        config: path.to_owned(),
        dry_run,
        expected_sha256: None,
        ops: vec![Operation {
            key: "appearance.theme".into(),
            value: "dark".into(),
        }],
    }
}

#[test]
fn rooted_paths_allow_inside_reads_and_writes_and_reject_outside_before_io() {
    let temp = Temp::new();
    let root = temp.0.join("work");
    let other = temp.0.join("work-other");
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::create_dir(&other).unwrap();
    fs::write(root.join("example.circ"), CIRCUIT).unwrap();
    fs::write(root.join("prefs.json"), PREFS).unwrap();
    fs::write(other.join("prefs.json"), PREFS).unwrap();
    let mut s = Service::with_root(&root).unwrap();
    let id = open(&mut s, Path::new("sub/../example.circ"));
    s.call(Request::ProjectSave {
        session: id,
        expected_sha256: None,
        backup: true,
    })
    .unwrap();
    assert_eq!(
        fs::read(root.join("example.circ.bak")).unwrap(),
        CIRCUIT.as_bytes()
    );
    let changed = s
        .call(patch_settings(&root.join("prefs.json"), false))
        .unwrap();
    assert_eq!(changed["settings"]["appearance"]["theme"], "dark");
    for outside in [
        other.join("prefs.json"),
        other.join("absent"),
        "../work-other/prefs.json".into(),
    ] {
        for request in [
            Request::SettingsGet {
                config: outside.clone(),
            },
            patch_settings(&outside, false),
            Request::ProjectOpen {
                path: outside.clone(),
            },
        ] {
            let error = s.call(request).unwrap_err();
            assert_eq!(error.code, "path_outside_root");
            assert!(!error.message.contains("work-other"));
        }
    }
    assert_eq!(fs::read_dir(&other).unwrap().count(), 1);
    assert_eq!(fs::read(other.join("prefs.json")).unwrap(), PREFS);
    assert_eq!(
        s.call(Request::ProjectOpen {
            path: "missing.circ".into()
        })
        .unwrap_err()
        .code,
        "not_found"
    );
}

#[cfg(unix)]
#[test]
fn rooted_links_resolve_inside_and_refuse_external_or_dangling_targets_and_sidecars() {
    use std::os::unix::fs::symlink;
    let temp = Temp::new();
    let root = temp.0.join("work");
    fs::create_dir_all(root.join("nested")).unwrap();
    let source = root.join("nested/example.circ");
    fs::write(&source, CIRCUIT).unwrap();
    fs::write(root.join("nested/prefs.json"), PREFS).unwrap();
    fs::write(temp.0.join("other.circ"), CIRCUIT).unwrap();
    symlink("nested", root.join("alias")).unwrap();
    symlink(&temp.0, root.join("external")).unwrap();
    symlink(temp.0.join("other.circ"), root.join("file-link")).unwrap();
    symlink(temp.0.join("absent"), root.join("dangling")).unwrap();
    symlink(&root, temp.0.join("root-link")).unwrap();
    let mut s = Service::with_root(&temp.0.join("root-link")).unwrap();
    let id = open(&mut s, &temp.0.join("root-link/alias/example.circ"));
    assert!(
        s.call(Request::SettingsGet {
            config: "alias/prefs.json".into()
        })
        .is_ok()
    );
    for path in [
        "external/other.circ",
        "external/missing",
        "file-link",
        "dangling",
    ] {
        assert_eq!(
            s.call(Request::ProjectOpen { path: path.into() })
                .unwrap_err()
                .code,
            "path_outside_root"
        );
    }

    let save = || Request::ProjectSave {
        session: id.clone(),
        expected_sha256: None,
        backup: true,
    };
    for ext in ["lock", "bak"] {
        let link = root.join(format!("nested/example.circ.{ext}"));
        symlink(temp.0.join("absent"), &link).unwrap();
        assert_eq!(s.call(save()).unwrap_err().code, "path_outside_root");
        assert_eq!(fs::read_dir(root.join("nested")).unwrap().count(), 3);
        fs::remove_file(link).unwrap();
    }
    symlink(temp.0.join("absent"), root.join("nested/prefs.json.lock")).unwrap();
    assert_eq!(
        s.call(patch_settings(Path::new("alias/prefs.json"), false))
            .unwrap_err()
            .code,
        "path_outside_root"
    );
    assert_eq!(fs::read(root.join("nested/prefs.json")).unwrap(), PREFS);
    // Recheck the stored project path on save, even after a successful rooted open.
    fs::rename(root.join("nested"), root.join("original")).unwrap();
    symlink(&temp.0, root.join("nested")).unwrap();
    assert_eq!(s.call(save()).unwrap_err().code, "path_outside_root");
    assert_eq!(
        fs::read(root.join("original/example.circ")).unwrap(),
        CIRCUIT.as_bytes()
    );
    assert!(!temp.0.join("absent").exists());
    assert!(!temp.0.join("example.circ.lock").exists());
}

#[test]
fn sixty_four_live_sessions_per_service_and_closing_releases_one_slot() {
    let temp = Temp::new();
    let path = temp.0.join("test.circ");
    fs::write(&path, CIRCUIT).unwrap();
    let mut s = Service::default();
    let mut ids = BTreeSet::new();
    for _ in 0..64 {
        let bad = Request::ProjectOpen {
            path: temp.0.join("missing.circ"),
        };
        assert_eq!(s.call(bad).unwrap_err().code, "not_found");
        let id = open(&mut s, &path);
        assert!(id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit()));
        assert!(ids.insert(id));
    }
    for _ in 0..2 {
        assert_eq!(
            s.call(Request::ProjectOpen { path: path.clone() })
                .unwrap_err()
                .code,
            "session_limit"
        );
    }
    assert!(!ids.contains(&open(&mut Service::default(), &path)));
    s.call(Request::ProjectClose {
        session: ids.first().unwrap().clone(),
    })
    .unwrap();
    assert!(ids.insert(open(&mut s, &path)));
    assert_eq!(
        s.call(Request::ProjectOpen { path }).unwrap_err().code,
        "session_limit"
    );
}
