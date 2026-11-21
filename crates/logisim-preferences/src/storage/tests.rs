use super::*;

#[test]
fn released_target_lock_allows_the_next_save_to_report_its_backup_error() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("logisim-save-lock-{}-{stamp}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let target = dir.join("project.circ");
    let backup = dir.join("project.circ.bak");
    fs::write(&target, b"old").unwrap();
    fs::create_dir(&backup).unwrap();
    let save = || write_project_copy(&target, b"new", Some(b"old"), true, 64);

    let writer = lock_target(&target).unwrap();
    let inherited = writer.0.try_clone().unwrap();
    assert!(matches!(
        save(),
        Err(CopyError::Destination(WriteError::Conflict))
    ));
    drop(writer);
    assert!(matches!(save(), Err(CopyError::Backup(path, _)) if path == backup));
    assert_eq!(fs::read(&target).unwrap(), b"old");
    drop(inherited);
    fs::remove_dir_all(dir).unwrap();
}
