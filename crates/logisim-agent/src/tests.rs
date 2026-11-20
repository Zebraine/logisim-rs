use super::*;
#[test]
fn source_hash_handles_single_and_multiple_padding_blocks() {
    for (input, expected) in [
        (
            "",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            "abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        ),
    ] {
        assert_eq!(digest::sha256(input.as_bytes()), expected);
    }
}

struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        let p =
            std::env::temp_dir().join(format!("logisim-agent-{:016x}", getrandom::u64().unwrap()));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn settings_patch_validates_the_entire_transaction_before_writing() {
    use std::fs;
    let temp = Temp::new();
    let path = temp.0.join("prefs.json");
    let bytes =
        br#"{"version":16,"recent":["/local.circ"],"plugins":{"fallback_directory":"/local"}}"#;
    fs::write(&path, bytes).unwrap();
    let hash = digest::sha256(bytes);
    let edit = |key: &str, value: Value| Operation {
        key: key.into(),
        value,
    };
    let dark = || edit("appearance.theme", json!("dark"));
    let preview = settings::patch(&path, Some(&hash), vec![dark()], true).unwrap();
    assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 1);
    assert_eq!(preview["source_sha256"], hash);
    for (op, code) in [
        (edit("recent", json!([])), "invalid_arguments"),
        (
            edit("plugins.fallback_directory", json!("/elsewhere")),
            "unknown_setting",
        ),
        (edit("files.autosave", json!(1)), "invalid_settings"),
        (
            edit("editor.default_gate_inputs", json!(33)),
            "invalid_settings",
        ),
        (
            edit("shortcuts.file.save", json!("primary+o")),
            "invalid_settings",
        ),
        (dark(), "invalid_arguments"),
    ] {
        assert_eq!(
            settings::patch(&path, None, vec![dark(), op], false)
                .unwrap_err()
                .code,
            code
        );
    }
    assert_eq!(
        settings::patch(&path, Some(&"0".repeat(64)), vec![dark()], false)
            .unwrap_err()
            .code,
        "conflict"
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let applied = settings::patch(&path, Some(&hash), vec![dark()], false).unwrap();
    assert_eq!(applied["source_sha256"], preview["result_sha256"]);
    assert_eq!(
        applied["source_sha256"],
        digest::sha256(&fs::read(&path).unwrap())
    );
    fs::write(&path, b"broken").unwrap();
    assert_eq!(
        settings::patch(&path, None, vec![dark()], false)
            .unwrap_err()
            .code,
        "invalid_settings"
    );
    assert_eq!(fs::read(&path).unwrap(), b"broken");
}

#[test]
fn competing_settings_writers_cannot_both_commit_the_same_revision() {
    use std::{fs, sync::Barrier, thread};
    let temp = Temp::new();
    let path = temp.0.join("prefs.json");
    let bytes = br#"{"version":16}"#;
    fs::write(&path, bytes).unwrap();
    let hash = digest::sha256(bytes);
    let barrier = Barrier::new(2);
    thread::scope(|scope| {
        let writers: Vec<_> = ["files.autosave", "appearance.show_telemetry"]
            .map(|key| {
                scope.spawn({
                    let (barrier, path, hash) = (&barrier, &path, &hash);
                    move || {
                        barrier.wait();
                        settings::patch(
                            path,
                            Some(hash),
                            vec![Operation {
                                key: key.into(),
                                value: false.into(),
                            }],
                            false,
                        )
                    }
                })
            })
            .into();
        let outcomes: Vec<_> = writers.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(outcomes.iter().filter(|r| r.is_ok()).count(), 1);
        assert_eq!(
            outcomes.into_iter().find_map(Result::err).unwrap().code,
            "conflict"
        );
    });
}
